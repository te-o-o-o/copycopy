//! Data model and history. No OS dependency, no UI dependency.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Text,
    Url,
    Code,
    Image,
    Files,
}

/// What a backend reports when the clipboard changes.
#[derive(Clone, Debug)]
pub enum ClipEvent {
    Text(String),
    /// Raw PNG as provided by the selection, plus dimensions when known.
    Image {
        png: Vec<u8>,
        size: Option<(u32, u32)>,
    },
    Files(Vec<PathBuf>),
}

/// Where an image's bytes are. Freshly captured entries carry them; entries
/// read back from storage carry only a path, and the file is opened when the
/// image is actually needed — never to draw a list row, which only shows text.
#[derive(Clone, Debug)]
pub enum Image {
    Bytes(Vec<u8>),
    File(PathBuf),
}

impl Image {
    pub fn load(&self) -> Result<Vec<u8>, String> {
        match self {
            Image::Bytes(bytes) => Ok(bytes.clone()),
            Image::File(path) => std::fs::read(path).map_err(|e| e.to_string()),
        }
    }

    pub fn path(&self) -> Option<&Path> {
        match self {
            Image::File(path) => Some(path),
            Image::Bytes(_) => None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Payload {
    Text(String),
    Image { data: Image, size: Option<(u32, u32)> },
    Files(Vec<PathBuf>),
}

#[derive(Clone, Debug)]
pub struct ClipItem {
    pub id: u64,
    pub kind: Kind,
    pub payload: Payload,
    /// Single line shown in the list.
    pub preview: String,
    pub source: String,
    pub at: SystemTime,
    pub hash: u64,
    pub pinned: bool,
}

impl ClipItem {
    /// How much content the row is not showing, when there is enough of it to
    /// be worth saying. Counted on the **full payload**, not on the preview,
    /// which is itself capped — the useful answer is "how big is this", not
    /// "how much did the preview keep".
    ///
    /// Returned as `None` below the threshold: a count on a short entry would
    /// be noise, and claiming truncation that did not happen would be a lie.
    pub fn overflow_hint(&self) -> Option<String> {
        /// Roughly what a row shows at the default window width. Deliberately
        /// conservative: better to stay silent on a borderline entry than to
        /// announce more where there is none.
        const VISIBLE: usize = 60;

        match &self.payload {
            Payload::Text(text) => {
                let count = text.chars().count();
                (count > VISIBLE).then(|| format!("… {} caractères", grouped(count)))
            }
            Payload::Files(paths) if paths.len() > 1 => {
                Some(format!("… {} fichiers", paths.len()))
            }
            // An image already states its dimensions and weight in the preview.
            _ => None,
        }
    }

    pub fn age(&self) -> String {
        let secs = SystemTime::now()
            .duration_since(self.at)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        match secs {
            0..=4 => "à l'instant".to_string(),
            5..=59 => format!("{secs} s"),
            60..=3599 => format!("{} min", secs / 60),
            3600..=86399 => format!("{} h", secs / 3600),
            _ => format!("{} j", secs / 86400),
        }
    }
}

/// Bounded history, deduplicated by content hash.
pub struct History {
    items: VecDeque<ClipItem>,
    capacity: usize,
    next_id: u64,
}

impl History {
    pub fn new(capacity: usize) -> Self {
        Self {
            items: VecDeque::with_capacity(capacity.min(1024)),
            capacity,
            next_id: 1,
        }
    }

    pub fn items(&self) -> &VecDeque<ClipItem> {
        &self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&ClipItem> {
        self.items.get(index)
    }

    /// Inserts an event. Returns `false` when the content was a duplicate: the
    /// existing entry then moves back to the top instead of being duplicated.
    pub fn push(&mut self, event: ClipEvent, source: String) -> bool {
        let hash = hash_event(&event);

        if let Some(pos) = self.items.iter().position(|i| i.hash == hash) {
            let mut existing = self.items.remove(pos).expect("position valide");
            existing.at = SystemTime::now();
            existing.source = source;
            self.items.push_front(existing);
            return false;
        }

        let (kind, preview, payload) = match event {
            ClipEvent::Text(t) => {
                let kind = classify(&t);
                let preview = one_line(&t);
                (kind, preview, Payload::Text(t))
            }
            ClipEvent::Image { png, size } => {
                let preview = match size {
                    Some((w, h)) => format!("Image {w}×{h} · PNG · {}", human_bytes(png.len())),
                    None => format!("Image · PNG · {}", human_bytes(png.len())),
                };
                (
                    Kind::Image,
                    preview,
                    Payload::Image {
                        data: Image::Bytes(png),
                        size,
                    },
                )
            }
            ClipEvent::Files(paths) => {
                let preview = match paths.split_first() {
                    Some((first, [])) => first.display().to_string(),
                    Some((first, rest)) => {
                        format!("{} (+{} autres)", first.display(), rest.len())
                    }
                    None => "aucun fichier".to_string(),
                };
                (Kind::Files, preview, Payload::Files(paths))
            }
        };

        let id = self.next_id;
        self.next_id += 1;
        self.items.push_front(ClipItem {
            id,
            kind,
            payload,
            preview,
            source,
            at: SystemTime::now(),
            hash,
            pinned: false,
        });

        while self.items.len() > self.capacity {
            // A pinned entry is never discarded.
            match self.items.iter().rposition(|i| !i.pinned) {
                Some(pos) => {
                    self.items.remove(pos);
                }
                None => break,
            }
        }
        true
    }

    /// Re-inserts an entry read back from storage, keeping its preview,
    /// timestamp and pinned flag as they were written.
    pub fn push_stored(&mut self, item: ClipItem) {
        let id = self.next_id;
        self.next_id += 1;
        self.items.push_front(ClipItem { id, ..item });
        while self.items.len() > self.capacity {
            match self.items.iter().rposition(|i| !i.pinned) {
                Some(pos) => {
                    self.items.remove(pos);
                }
                None => break,
            }
        }
    }

    /// Points a freshly captured image at the file the store has just written
    /// for it, and drops the bytes.
    ///
    /// Without this the whole history stays in memory: every refilter clones
    /// the entries it keeps, so a 4K screenshot would be copied again on each
    /// keystroke in the search field.
    pub fn offload_image(&mut self, index: usize, path: PathBuf) {
        if let Some(item) = self.items.get_mut(index) {
            if let Payload::Image { data, .. } = &mut item.payload {
                *data = Image::File(path);
            }
        }
    }

    /// Moves an entry back to the top. False when the id is unknown.
    ///
    /// Copying from the history is a use, and the list is ordered by recency.
    /// The caller decides *when*: doing it the instant the entry is copied
    /// makes the list slip under the cursor.
    pub fn touch(&mut self, id: u64, at: SystemTime) -> bool {
        let Some(pos) = self.items.iter().position(|i| i.id == id) else {
            return false;
        };
        let Some(mut item) = self.items.remove(pos) else {
            return false;
        };
        item.at = at;
        self.items.push_front(item);
        true
    }

    pub fn toggle_pin(&mut self, index: usize) {
        if let Some(item) = self.items.get_mut(index) {
            item.pinned = !item.pinned;
        }
    }

    pub fn remove(&mut self, index: usize) {
        self.items.remove(index);
    }

    pub fn clear_unpinned(&mut self) {
        self.items.retain(|i| i.pinned);
    }
}

/// 64-bit FNV-1a: stable across runs, unlike `DefaultHasher`, so it can be
/// reused as-is once we move to SQLite persistence.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    fnv1a_from(0xcbf2_9ce4_8422_2325, bytes)
}

/// The same, continued from an existing state, so one hash can cover several
/// pieces without concatenating them into a buffer first — which matters when
/// one of those pieces is a decoded image.
pub fn fnv1a_from(seed: u64, bytes: &[u8]) -> u64 {
    let mut h = seed;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

pub fn hash_event(event: &ClipEvent) -> u64 {
    match event {
        // Text is normalised: "Hello" and "Hello\n" are the same clip.
        ClipEvent::Text(t) => fnv1a(t.trim().as_bytes()),
        ClipEvent::Image { png, .. } => fnv1a(png),
        ClipEvent::Files(paths) => {
            let joined = paths
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join("\n");
            fnv1a(joined.as_bytes())
        }
    }
}

/// Single-line preview: newlines and tabs become spaces, and consecutive
/// whitespace is collapsed.
fn one_line(text: &str) -> String {
    let mut out = String::with_capacity(text.len().min(256));
    let mut last_space = false;
    for c in text.trim().chars().take(512) {
        let c = if c.is_whitespace() { ' ' } else { c };
        if c == ' ' {
            if !last_space {
                out.push(' ');
            }
            last_space = true;
        } else {
            out.push(c);
            last_space = false;
        }
    }
    out
}

fn classify(text: &str) -> Kind {
    let t = text.trim();
    if t.is_empty() {
        return Kind::Text;
    }
    let is_url = !t.contains(char::is_whitespace)
        && ["http://", "https://", "ftp://", "ssh://", "postgres://", "mysql://", "file://"]
            .iter()
            .any(|p| t.starts_with(p));
    if is_url {
        return Kind::Url;
    }
    // Deliberately coarse heuristic: better to miss some code than to tag
    // ordinary prose as code.
    let code_markers = [
        "fn ", "let ", "const ", "function ", "class ", "def ", "import ", "SELECT ", "#include",
        "=>", "->", "();", "{\n", ";\n",
    ];
    let hits = code_markers.iter().filter(|m| t.contains(**m)).count();
    if hits >= 2 || (hits >= 1 && t.contains('\n')) {
        return Kind::Code;
    }
    Kind::Text
}

/// Thousands separated by a non-breaking space, as French typography does.
fn grouped(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('\u{202f}');
        }
        out.push(c);
    }
    out
}

fn human_bytes(n: usize) -> String {
    const K: f64 = 1024.0;
    let n = n as f64;
    if n < K {
        format!("{n:.0} o")
    } else if n < K * K {
        format!("{:.1} Kio", n / K)
    } else {
        format!("{:.1} Mio", n / (K * K))
    }
}

pub fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedup_moves_to_top_without_duplicating() {
        let mut h = History::new(10);
        assert!(h.push(ClipEvent::Text("a".into()), "t".into()));
        assert!(h.push(ClipEvent::Text("b".into()), "t".into()));
        // Same content, whitespace aside.
        assert!(!h.push(ClipEvent::Text("a\n".into()), "t".into()));
        assert_eq!(h.len(), 2);
        assert_eq!(h.get(0).unwrap().preview, "a");
    }

    #[test]
    fn capacity_is_honoured_and_pinned_entries_survive() {
        let mut h = History::new(3);
        for i in 0..3 {
            h.push(ClipEvent::Text(format!("t{i}")), "t".into());
        }
        h.toggle_pin(2); // "t0", the oldest one
        for i in 3..8 {
            h.push(ClipEvent::Text(format!("t{i}")), "t".into());
        }
        assert_eq!(h.len(), 3);
        assert!(h.items().iter().any(|i| i.preview == "t0" && i.pinned));
    }

    #[test]
    fn overflow_hint_counts_the_payload_not_the_preview() {
        let mut h = History::new(10);
        h.push(ClipEvent::Text("court".into()), "t".into());
        assert_eq!(h.get(0).unwrap().overflow_hint(), None, "short entries stay silent");

        // Long enough to be cut, and with newlines the preview collapses: the
        // count must follow the payload, not what the preview kept.
        let long = "ligne\n".repeat(400);
        let expected = long.chars().count();
        h.push(ClipEvent::Text(long), "t".into());
        let hint = h.get(0).unwrap().overflow_hint().expect("hint");
        assert!(hint.starts_with('…'));
        assert!(
            hint.contains(&grouped(expected)),
            "counts the payload ({expected}), got {hint}"
        );
    }

    #[test]
    fn an_offloaded_image_points_at_the_file_instead_of_the_bytes() {
        let mut h = History::new(10);
        h.push(
            ClipEvent::Image {
                png: vec![1, 2, 3],
                size: Some((2, 2)),
            },
            "t".into(),
        );
        h.offload_image(0, PathBuf::from("/somewhere/cafe.png"));
        match &h.get(0).expect("item").payload {
            Payload::Image { data, .. } => {
                assert_eq!(data.path(), Some(Path::new("/somewhere/cafe.png")))
            }
            other => panic!("expected an image, got {other:?}"),
        }
    }

    #[test]
    fn touching_an_entry_moves_it_back_to_the_top() {
        let mut h = History::new(10);
        for name in ["a", "b", "c"] {
            h.push(ClipEvent::Text(name.into()), "t".into());
        }
        let oldest = h.get(2).expect("item").id;
        assert!(h.touch(oldest, SystemTime::now()));
        assert_eq!(h.get(0).expect("item").preview, "a");
        assert_eq!(h.len(), 3, "moved, not duplicated");
        assert!(!h.touch(9999, SystemTime::now()), "an unknown id changes nothing");
    }

    #[test]
    fn thousands_are_grouped() {
        assert_eq!(grouped(42), "42");
        assert_eq!(grouped(1234), "1\u{202f}234");
        assert_eq!(grouped(1234567), "1\u{202f}234\u{202f}567");
    }

    #[test]
    fn classification() {
        assert_eq!(classify("https://example.com/a"), Kind::Url);
        assert_eq!(classify("fn main() {\n    let x = 1;\n}"), Kind::Code);
        assert_eq!(classify("Bonjour tout le monde"), Kind::Text);
        assert_eq!(classify("j'ai -> une flèche"), Kind::Text);
    }

    #[test]
    fn single_line_preview() {
        assert_eq!(one_line("  a\n\n\tb  "), "a b");
    }
}
pub mod store;
