//! Persistence.
//!
//! SQLite, compiled into the binary through rusqlite's `bundled` feature: no
//! system library to install, on any OS. The database is a single file the
//! application carries with it, which is what makes a portable build possible.
//!
//! Text is searched through **FTS5** rather than the substring scan the UI used
//! while everything lived in memory. Images are written next to the database as
//! files, never as blobs: a blob would bloat every query with bytes nobody
//! asked for.

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};

use crate::{ClipItem, Image, Kind, Payload};

const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS clips (
    id       INTEGER PRIMARY KEY,
    hash     INTEGER NOT NULL UNIQUE,
    kind     TEXT    NOT NULL,
    preview  TEXT    NOT NULL,
    text     TEXT,
    files    TEXT,
    image    TEXT,
    width    INTEGER,
    height   INTEGER,
    source   TEXT    NOT NULL DEFAULT '',
    at       INTEGER NOT NULL,
    pinned   INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS clips_order ON clips(pinned DESC, at DESC, id DESC);

-- External-content table: the text lives in `clips`, FTS only keeps the index.
-- `trigram` rather than the default tokeniser: a clipboard search is looked up
-- by fragment, not by word. It also indexes languages that have no spaces, so
-- CJK content stays searchable. Its one limit is a three-character minimum,
-- which the caller handles.
CREATE VIRTUAL TABLE IF NOT EXISTS clips_fts
    USING fts5(preview, content='clips', content_rowid='id', tokenize='trigram');

CREATE TRIGGER IF NOT EXISTS clips_ai AFTER INSERT ON clips BEGIN
    INSERT INTO clips_fts(rowid, preview) VALUES (new.id, new.preview);
END;
CREATE TRIGGER IF NOT EXISTS clips_ad AFTER DELETE ON clips BEGIN
    INSERT INTO clips_fts(clips_fts, rowid, preview) VALUES('delete', old.id, old.preview);
END;
CREATE TRIGGER IF NOT EXISTS clips_au AFTER UPDATE ON clips BEGIN
    INSERT INTO clips_fts(clips_fts, rowid, preview) VALUES('delete', old.id, old.preview);
    INSERT INTO clips_fts(rowid, preview) VALUES (new.id, new.preview);
END;
"#;

pub struct Store {
    conn: Connection,
    images: PathBuf,
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Text => "text",
        Kind::Url => "url",
        Kind::Code => "code",
        Kind::Image => "image",
        Kind::Files => "files",
    }
}

fn kind_from(name: &str) -> Kind {
    match name {
        "url" => Kind::Url,
        "code" => Kind::Code,
        "image" => Kind::Image,
        "files" => Kind::Files,
        _ => Kind::Text,
    }
}

fn seconds(at: SystemTime) -> i64 {
    at.duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl Store {
    /// Opens, and creates, the database inside `dir`. Images land in
    /// `dir/images`.
    pub fn open(dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let images = dir.join("images");
        std::fs::create_dir_all(&images).map_err(|e| e.to_string())?;

        let conn = Connection::open(dir.join("copycopy.db")).map_err(|e| e.to_string())?;
        conn.execute_batch(SCHEMA).map_err(|e| e.to_string())?;
        Ok(Self { conn, images })
    }

    /// Inserts an entry, or bumps the existing one when the content is already
    /// known. Returns the row id in both cases, so the caller can keep the
    /// in-memory entry and the row in step.
    pub fn insert(&self, item: &ClipItem) -> Result<u64, String> {
        let hash = item.hash as i64;

        if let Some(id) = self.id_for_hash(hash)? {
            self.conn
                .execute(
                    "UPDATE clips SET at = ?1, source = ?2 WHERE id = ?3",
                    params![seconds(item.at), item.source, id as i64],
                )
                .map_err(|e| e.to_string())?;
            return Ok(id);
        }

        let (text, files, image, width, height) = match &item.payload {
            Payload::Text(t) => (Some(t.clone()), None, None, None, None),
            Payload::Files(paths) => {
                let joined = paths
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join("\n");
                (None, Some(joined), None, None, None)
            }
            Payload::Image { data, size } => {
                // One file per content hash: the name is reproducible, so an
                // entry read back and re-inserted lands on the same file.
                let path = self.image_path(item.hash);
                if !path.exists() {
                    std::fs::write(&path, data.load()?).map_err(|e| e.to_string())?;
                }
                (
                    None,
                    None,
                    Some(path.display().to_string()),
                    size.map(|(w, _)| w as i64),
                    size.map(|(_, h)| h as i64),
                )
            }
        };

        self.conn
            .execute(
                "INSERT INTO clips (hash, kind, preview, text, files, image, width, height, source, at, pinned)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    hash,
                    kind_name(item.kind),
                    item.preview,
                    text,
                    files,
                    image,
                    width,
                    height,
                    item.source,
                    seconds(item.at),
                    item.pinned as i64,
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(self.conn.last_insert_rowid() as u64)
    }

    /// Where `insert` writes an image of that hash. Derived from the hash
    /// rather than stored, so the caller can point an in-memory entry at the
    /// file straight after inserting it, without a query.
    pub fn image_path(&self, hash: u64) -> PathBuf {
        self.images.join(format!("{hash:016x}.png"))
    }

    fn id_for_hash(&self, hash: i64) -> Result<Option<u64>, String> {
        self.conn
            .query_row("SELECT id FROM clips WHERE hash = ?1", params![hash], |r| {
                r.get::<_, i64>(0)
            })
            .optional()
            .map(|o| o.map(|id| id as u64))
            .map_err(|e| e.to_string())
    }

    /// Newest first, pinned entries ahead — the ordering the UI used to compute
    /// by hand.
    pub fn recent(&self, limit: usize) -> Result<Vec<ClipItem>, String> {
        self.collect(
            "SELECT id, hash, kind, preview, text, files, image, width, height, source, at, pinned
             FROM clips ORDER BY pinned DESC, at DESC, id DESC LIMIT ?1",
            params![limit as i64],
        )
    }

    /// Shortest query the trigram index can answer. Below it the caller filters
    /// in memory instead.
    pub const MIN_QUERY: usize = 3;

    /// Full-text search, by fragment, through the trigram index.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<ClipItem>, String> {
        let trimmed = query.trim();
        if trimmed.chars().count() < Self::MIN_QUERY {
            return self.recent(limit);
        }
        // Quoted as a phrase: the fragment is taken literally, so punctuation
        // in a URL or a snippet of code cannot be read as query syntax.
        let pattern = format!("\"{}\"", trimmed.replace('"', " "));
        self.collect(
            "SELECT c.id, c.hash, c.kind, c.preview, c.text, c.files, c.image, c.width,
                    c.height, c.source, c.at, c.pinned
             FROM clips_fts f JOIN clips c ON c.id = f.rowid
             WHERE clips_fts MATCH ?1
             ORDER BY c.pinned DESC, c.at DESC, c.id DESC LIMIT ?2",
            params![pattern, limit as i64],
        )
    }

    fn collect(&self, sql: &str, args: impl rusqlite::Params) -> Result<Vec<ClipItem>, String> {
        let mut stmt = self.conn.prepare(sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(args, |row| {
                let kind = kind_from(&row.get::<_, String>(2)?);
                let text: Option<String> = row.get(4)?;
                let files: Option<String> = row.get(5)?;
                let image: Option<String> = row.get(6)?;
                let width: Option<i64> = row.get(7)?;
                let height: Option<i64> = row.get(8)?;

                let payload = if let Some(path) = image {
                    // The path only: reading every PNG back would make each
                    // keystroke of a search hit the disk.
                    Payload::Image {
                        data: Image::File(PathBuf::from(path)),
                        size: width.zip(height).map(|(w, h)| (w as u32, h as u32)),
                    }
                } else if let Some(files) = files {
                    Payload::Files(files.lines().map(PathBuf::from).collect())
                } else {
                    Payload::Text(text.unwrap_or_default())
                };

                Ok(ClipItem {
                    id: row.get::<_, i64>(0)? as u64,
                    kind,
                    payload,
                    preview: row.get(3)?,
                    source: row.get(9)?,
                    at: UNIX_EPOCH + Duration::from_secs(row.get::<_, i64>(10)?.max(0) as u64),
                    hash: row.get::<_, i64>(1)? as u64,
                    pinned: row.get::<_, i64>(11)? != 0,
                })
            })
            .map_err(|e| e.to_string())?;

        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    /// Addressed by content hash rather than row id: the in-memory history
    /// numbers its own entries, and keeping two counters in step would be one
    /// more thing to get wrong.
    pub fn set_pinned(&self, hash: u64, pinned: bool) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE clips SET pinned = ?1 WHERE hash = ?2",
                params![pinned as i64, hash as i64],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// Marks an entry as used again, so it leads the list on the next read.
    ///
    /// Leaves `source` alone on purpose: copying from the history is not a
    /// new capture, and the attribution must stay that of the application the
    /// content actually came from.
    pub fn touch(&self, hash: u64, at: SystemTime) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE clips SET at = ?1 WHERE hash = ?2",
                params![seconds(at), hash as i64],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn delete(&self, hash: u64) -> Result<(), String> {
        let images = self.image_paths(
            "SELECT image FROM clips WHERE hash = ?1 AND image IS NOT NULL",
            params![hash as i64],
        )?;
        self.conn
            .execute("DELETE FROM clips WHERE hash = ?1", params![hash as i64])
            .map_err(|e| e.to_string())?;
        self.forget_images(&images);
        Ok(())
    }

    /// Drops the oldest unpinned entries beyond `keep`. Pinned ones are never
    /// dropped, whatever their age.
    pub fn prune(&self, keep: usize) -> Result<usize, String> {
        const DOOMED: &str = "pinned = 0 AND id NOT IN (
                 SELECT id FROM clips WHERE pinned = 0 ORDER BY at DESC LIMIT ?1
             )";
        let images = self.image_paths(
            &format!("SELECT image FROM clips WHERE {DOOMED} AND image IS NOT NULL"),
            params![keep as i64],
        )?;
        let dropped = self
            .conn
            .execute(
                &format!("DELETE FROM clips WHERE {DOOMED}"),
                params![keep as i64],
            )
            .map_err(|e| e.to_string())?;
        self.forget_images(&images);
        Ok(dropped)
    }

    /// Removes image files no row points at any more, and says how many.
    ///
    /// Two sources feed that pile: a crash between writing the file and
    /// inserting its row, and every run of this program from before deletion
    /// and pruning learnt to take the file with the row. Meant to be called
    /// once at startup, when nothing else is touching the directory.
    pub fn sweep_orphan_images(&self) -> Result<usize, String> {
        let kept: HashSet<OsString> = self
            .image_paths("SELECT image FROM clips WHERE image IS NOT NULL", [])?
            .iter()
            .filter_map(|p| Path::new(p).file_name().map(OsStr::to_os_string))
            .collect();

        let mut removed = 0;
        for entry in std::fs::read_dir(&self.images)
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
        {
            let name = entry.file_name();
            if Path::new(&name).extension().is_some_and(|e| e == "png")
                && !kept.contains(&name)
                && std::fs::remove_file(entry.path()).is_ok()
            {
                removed += 1;
            }
        }
        Ok(removed)
    }

    /// The image column of whatever rows the query selects.
    fn image_paths(&self, sql: &str, args: impl rusqlite::Params) -> Result<Vec<String>, String> {
        let mut stmt = self.conn.prepare(sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(args, |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    /// Deletes image files whose rows have just gone. Called **after** the
    /// rows, never before: an orphan file is invisible and recoverable, while
    /// a row pointing at a file that is gone is an entry that fails when the
    /// user tries to copy it.
    fn forget_images(&self, paths: &[String]) {
        for path in paths {
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => eprintln!("could not remove {path}: {e}"),
            }
        }
    }

    pub fn count(&self) -> Result<usize, String> {
        self.conn
            .query_row("SELECT COUNT(*) FROM clips", [], |r| r.get::<_, i64>(0))
            .map(|n| n as usize)
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClipEvent, History};

    /// A throwaway directory per test, removed on drop.
    struct Temp(PathBuf);

    impl Temp {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "copycopy-test-{name}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            Self(dir)
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Builds entries through `History`, so the tests exercise the same
    /// classification and hashing the application uses.
    fn entries(texts: &[&str]) -> Vec<ClipItem> {
        let mut history = History::new(100);
        for t in texts {
            history.push(ClipEvent::Text((*t).to_string()), "test".into());
        }
        history.items().iter().cloned().collect()
    }

    #[test]
    fn survives_a_reopen() {
        let dir = Temp::new("reopen");
        {
            let store = Store::open(&dir.0).expect("open");
            for item in entries(&["alpha", "beta"]) {
                store.insert(&item).expect("insert");
            }
        }
        let store = Store::open(&dir.0).expect("reopen");
        let previews: Vec<String> = store
            .recent(10)
            .expect("recent")
            .into_iter()
            .map(|i| i.preview)
            .collect();
        assert_eq!(previews.len(), 2);
        assert!(previews.contains(&"alpha".to_string()));
    }

    #[test]
    fn same_content_is_not_duplicated() {
        let dir = Temp::new("dedup");
        let store = Store::open(&dir.0).expect("open");
        let items = entries(&["hello"]);
        let first = store.insert(&items[0]).expect("insert");
        let second = store.insert(&items[0]).expect("insert again");
        assert_eq!(first, second, "same hash must reuse the row");
        assert_eq!(store.count().expect("count"), 1);
    }

    #[test]
    fn full_text_search_finds_and_filters() {
        let dir = Temp::new("fts");
        let store = Store::open(&dir.0).expect("open");
        for item in entries(&["the quick brown fox", "a lazy dog", "another fox entirely"]) {
            store.insert(&item).expect("insert");
        }
        let hits = store.search("fox", 10).expect("search");
        assert_eq!(hits.len(), 2, "both foxes");
        assert!(store.search("zzz", 10).expect("search").is_empty());
        // Fragment, not word: the point of the trigram tokeniser.
        assert_eq!(store.search("uick", 10).expect("search").len(), 1);
        assert_eq!(store.search("own fo", 10).expect("search").len(), 1);
    }

    #[test]
    fn pinned_come_first_and_survive_pruning() {
        let dir = Temp::new("prune");
        let store = Store::open(&dir.0).expect("open");
        let items = entries(&["one", "two", "three", "four"]);
        for item in &items {
            store.insert(item).expect("insert");
        }
        // `entries` returns newest first, so the last one is the oldest entry.
        let oldest = items.last().expect("item").hash;
        store.set_pinned(oldest, true).expect("pin");

        assert!(
            store.recent(10).expect("recent")[0].pinned,
            "a pinned entry leads the list"
        );

        store.prune(1).expect("prune");
        let left = store.recent(10).expect("recent");
        assert_eq!(left.len(), 2, "one unpinned kept, plus the pinned one");
        assert!(left.iter().any(|i| i.pinned), "pruning never drops a pin");
    }

    #[test]
    fn deleting_removes_the_row_and_its_index_entry() {
        let dir = Temp::new("delete");
        let store = Store::open(&dir.0).expect("open");
        let items = entries(&["keep this one", "delete that one"]);
        for item in &items {
            store.insert(item).expect("insert");
        }
        let doomed = items
            .iter()
            .find(|i| i.preview.contains("delete"))
            .expect("item");
        store.delete(doomed.hash).expect("delete");

        assert_eq!(store.count().expect("count"), 1);
        assert!(
            store.search("delete", 10).expect("search").is_empty(),
            "the full-text index must forget it too"
        );
        assert_eq!(store.search("keep", 10).expect("search").len(), 1);
    }

    /// Inserts `count` distinct pictures and returns them, newest first.
    fn images(store: &Store, count: usize) -> Vec<ClipItem> {
        let mut history = History::new(100);
        for i in 0..count {
            history.push(
                ClipEvent::Image {
                    png: format!("not really a picture, only entry {i}").into_bytes(),
                    size: Some((10 + i as u32, 20)),
                },
                "test".into(),
            );
        }
        let items: Vec<ClipItem> = history.items().iter().cloned().collect();
        for item in items.iter().rev() {
            store.insert(item).expect("insert");
        }
        items
    }

    fn image_files(dir: &Path) -> usize {
        std::fs::read_dir(dir.join("images"))
            .expect("images dir")
            .filter_map(Result::ok)
            .count()
    }

    #[test]
    fn touching_an_entry_reorders_it_without_rewriting_its_source() {
        let dir = Temp::new("touch");
        let store = Store::open(&dir.0).expect("open");
        let items = entries(&["oldest", "newest"]);
        for item in items.iter().rev() {
            store.insert(item).expect("insert");
        }
        assert_eq!(store.recent(10).expect("recent")[0].preview, "newest");

        let oldest = items.iter().find(|i| i.preview == "oldest").expect("item");
        store
            .touch(oldest.hash, SystemTime::now() + Duration::from_secs(60))
            .expect("touch");

        let back = store.recent(10).expect("recent");
        assert_eq!(back[0].preview, "oldest", "it leads the list now");
        assert_eq!(back[0].source, "test", "but it did not come from us");
    }

    #[test]
    fn deleting_an_entry_takes_its_image_file_with_it() {
        let dir = Temp::new("image-delete");
        let store = Store::open(&dir.0).expect("open");
        let items = images(&store, 3);
        assert_eq!(image_files(&dir.0), 3);

        store.delete(items[1].hash).expect("delete");
        assert_eq!(image_files(&dir.0), 2, "the file goes with the row");
        assert!(
            !store.image_path(items[1].hash).exists(),
            "and it is the right one"
        );
        assert!(store.image_path(items[0].hash).exists());
    }

    #[test]
    fn pruning_takes_the_image_files_it_drops() {
        let dir = Temp::new("image-prune");
        let store = Store::open(&dir.0).expect("open");
        let items = images(&store, 4);
        store.set_pinned(items[3].hash, true).expect("pin");

        store.prune(1).expect("prune");
        // One unpinned kept, plus the pinned one: two files, two rows.
        assert_eq!(store.count().expect("count"), 2);
        assert_eq!(image_files(&dir.0), 2);
        assert!(
            store.image_path(items[3].hash).exists(),
            "a pinned entry keeps its picture whatever its age"
        );
    }

    #[test]
    fn opening_sweeps_pictures_no_row_points_at() {
        let dir = Temp::new("image-sweep");
        let store = Store::open(&dir.0).expect("open");
        let items = images(&store, 2);

        // What every version before this one left behind: a file whose row was
        // pruned away, and one written just before a crash.
        std::fs::write(dir.0.join("images").join("deadbeefdeadbeef.png"), b"orphan")
            .expect("write");
        std::fs::write(dir.0.join("images").join("0123456789abcdef.png"), b"orphan")
            .expect("write");
        assert_eq!(image_files(&dir.0), 4);

        assert_eq!(store.sweep_orphan_images().expect("sweep"), 2);
        assert_eq!(image_files(&dir.0), 2);
        for item in &items {
            assert!(
                store.image_path(item.hash).exists(),
                "a picture with a row is never swept"
            );
        }
    }

    #[test]
    fn images_go_to_disk_not_into_the_database() {
        let dir = Temp::new("image");
        let store = Store::open(&dir.0).expect("open");
        let mut history = History::new(10);
        let png = b"\x89PNG\r\n\x1a\n-not-a-real-image".to_vec();
        history.push(
            ClipEvent::Image {
                png: png.clone(),
                size: Some((12, 34)),
            },
            "test".into(),
        );
        let item = history.get(0).expect("item").clone();
        store.insert(&item).expect("insert");

        let files: Vec<_> = std::fs::read_dir(dir.0.join("images"))
            .expect("images dir")
            .filter_map(Result::ok)
            .collect();
        assert_eq!(files.len(), 1, "one file written next to the database");

        match &store.recent(1).expect("recent")[0].payload {
            Payload::Image { data, size } => {
                assert!(data.path().is_some(), "read back as a path, not bytes");
                assert_eq!(data.load().expect("load"), png, "bytes load on demand");
                assert_eq!(*size, Some((12, 34)));
            }
            other => panic!("expected an image, got {other:?}"),
        }
    }
}
