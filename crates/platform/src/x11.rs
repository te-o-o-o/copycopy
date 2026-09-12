//! Event-driven X11 backend.
//!
//! XFixes wakes us up when another client takes ownership of the CLIPBOARD
//! selection; we then ask for TARGETS, pick the best available format and read
//! the property, INCR included for large payloads.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::xfixes::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{
    AtomEnum, ConnectionExt as _, CreateWindowAux, EventMask, Property, WindowClass,
};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::COPY_DEPTH_FROM_PARENT;

use copycopy_core::ClipEvent;

use crate::Capture;

/// Beyond this we ignore the content: a clipboard is not a file system.
const MAX_BYTES: usize = 32 * 1024 * 1024;
/// An unresponsive client must not block the next capture.
const REPLY_TIMEOUT: Duration = Duration::from_millis(1500);

struct Atoms {
    clipboard: u32,
    targets: u32,
    incr: u32,
    utf8_string: u32,
    text_plain_utf8: u32,
    image_png: u32,
    image_bmp: u32,
    image_jpeg: u32,
    image_tiff: u32,
    uri_list: u32,
    dest: u32,
    net_wm_pid: u32,
    /// Password-manager markers: such content is never captured.
    kde_password_hint: u32,
    concealed: u32,
}

impl Atoms {
    fn intern(conn: &RustConnection) -> Result<Self, String> {
        let get = |name: &str| -> Result<u32, String> {
            conn.intern_atom(false, name.as_bytes())
                .map_err(|e| e.to_string())?
                .reply()
                .map(|r| r.atom)
                .map_err(|e| e.to_string())
        };
        Ok(Self {
            clipboard: get("CLIPBOARD")?,
            targets: get("TARGETS")?,
            incr: get("INCR")?,
            utf8_string: get("UTF8_STRING")?,
            text_plain_utf8: get("text/plain;charset=utf-8")?,
            image_png: get("image/png")?,
            image_bmp: get("image/bmp")?,
            image_jpeg: get("image/jpeg")?,
            image_tiff: get("image/tiff")?,
            uri_list: get("text/uri-list")?,
            dest: get("COPYCOPY_SELECTION")?,
            net_wm_pid: get("_NET_WM_PID")?,
            kde_password_hint: get("x-kde-passwordManagerHint")?,
            concealed: get("org.nspasteboard.ConcealedType")?,
        })
    }
}

pub fn spawn(tx: Sender<Capture>) -> Result<(), String> {
    // Connecting here fails immediately when X11 is absent, which lets
    // `start()` fall back to polling.
    let (conn, screen_num) = x11rb::connect(None).map_err(|e| e.to_string())?;
    let atoms = Atoms::intern(&conn)?;

    conn.xfixes_query_version(5, 0)
        .map_err(|e| format!("XFixes absent : {e}"))?
        .reply()
        .map_err(|e| format!("XFixes absent : {e}"))?;

    let screen = &conn.setup().roots[screen_num];
    let window = conn.generate_id().map_err(|e| e.to_string())?;
    conn.create_window(
        COPY_DEPTH_FROM_PARENT,
        window,
        screen.root,
        0,
        0,
        1,
        1,
        0,
        WindowClass::INPUT_OUTPUT,
        x11rb::COPY_FROM_PARENT,
        &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
    )
    .map_err(|e| e.to_string())?;

    conn.xfixes_select_selection_input(
        window,
        atoms.clipboard,
        xfixes::SelectionEventMask::SET_SELECTION_OWNER,
    )
    .map_err(|e| e.to_string())?;
    conn.flush().map_err(|e| e.to_string())?;

    std::thread::Builder::new()
        .name("copycopy-x11".into())
        .spawn(move || run(conn, window, atoms, tx))
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn run(conn: RustConnection, window: u32, atoms: Atoms, tx: Sender<Capture>) {
    let mut reader = Reader {
        conn,
        window,
        atoms,
        deferred: VecDeque::new(),
    };
    // Selection changes that happened while we were reading: handle them
    // afterwards rather than lose them, which matters for rapid copies.
    let mut queue: VecDeque<u32> = VecDeque::new();
    // Some environments (the WSLg clipboard bridge, third-party clipboard
    // managers) take the selection back right after the source application, so
    // several notifications arrive for a single copy. A repeat is only
    // forwarded when it brings the source attribution that was missing.
    let mut last_hash: Option<u64> = None;
    let mut last_attributed = false;

    loop {
        let owner = match queue.pop_front() {
            Some(owner) => owner,
            None => match reader.conn.wait_for_event() {
                Ok(Event::XfixesSelectionNotify(ev))
                    if ev.selection == reader.atoms.clipboard
                        && ev.owner != x11rb::NONE
                        && ev.owner != reader.window =>
                {
                    ev.owner
                }
                Ok(_) => continue,
                Err(e) => {
                    eprintln!("connexion X11 perdue : {e}");
                    return;
                }
            },
        };

        // Attribution happens BEFORE reading: an application closing right
        // after the copy would have taken its window away by then.
        let source = reader.describe_owner(owner).unwrap_or_default();

        match reader.read_clipboard() {
            Ok(Some(event)) => {
                let hash = copycopy_core::hash_event(&event);
                let attributed = !source.is_empty();
                let same_as_before = last_hash == Some(hash);
                let redundant = same_as_before && (last_attributed || !attributed);

                last_attributed = if same_as_before {
                    last_attributed || attributed
                } else {
                    attributed
                };
                last_hash = Some(hash);

                if redundant {
                    continue;
                }
                if tx.send(Capture { event, source }).is_err() {
                    return; // L'UI est partie.
                }
            }
            Ok(None) => {}
            Err(e) => eprintln!("lecture du presse-papier : {e}"),
        }
        queue.extend(reader.deferred.drain(..));
    }
}

struct Reader {
    conn: RustConnection,
    window: u32,
    atoms: Atoms,
    /// Owners announced during a read, to be handled right after it.
    deferred: VecDeque<u32>,
}

impl Reader {
fn read_clipboard(&mut self) -> Result<Option<ClipEvent>, String> {
    let atoms_targets = self.atoms.targets;
    let (_, targets_raw) = self.convert_and_read(atoms_targets)?;
    let targets: Vec<u32> = targets_raw
        .chunks_exact(4)
        .map(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
        .collect();

    // A password manager is announcing that the content is secret, so we
    // leave it alone. This is the most important rule in the project.
    if targets.contains(&self.atoms.kde_password_hint) || targets.contains(&self.atoms.concealed) {
        return Ok(None);
    }

    // Preference order, then whatever the owner offered. Anything that is not
    // PNG gets converted, so only one format ever reaches the history.
    let image_targets = [
        self.atoms.image_png,
        self.atoms.image_bmp,
        self.atoms.image_jpeg,
        self.atoms.image_tiff,
    ];
    let offers_image = image_targets.iter().any(|t| targets.contains(t));
    for target in image_targets.into_iter().filter(|t| targets.contains(t)) {
        let (_, bytes) = self.convert_and_read(target)?;
        if let Some((png, size)) = crate::to_png(bytes) {
            return Ok(Some(ClipEvent::Image { png, size }));
        }
    }
    if offers_image {
        // The owner advertises an image we could not decode. Falling through to
        // the text branch would store the raw bytes as an unreadable entry.
        return Ok(None);
    }

    if targets.contains(&self.atoms.uri_list) {
        let (_, raw) = self.convert_and_read(self.atoms.uri_list)?;
        let paths: Vec<PathBuf> = String::from_utf8_lossy(&raw)
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .filter_map(|l| l.strip_prefix("file://").or(Some(l)))
            .map(|l| PathBuf::from(crate::percent_decode(l)))
            .collect();
        if !paths.is_empty() {
            return Ok(Some(ClipEvent::Files(paths)));
        }
    }

    let text_target = [
        self.atoms.utf8_string,
        self.atoms.text_plain_utf8,
        u32::from(AtomEnum::STRING),
    ]
        .into_iter()
        .find(|t| targets.contains(t))
        // Some clients do not advertise TARGETS properly, so UTF8_STRING is
        // attempted anyway rather than capturing nothing.
        .unwrap_or(self.atoms.utf8_string);

    let (_, raw) = self.convert_and_read(text_target)?;
    Ok(crate::sane_text(&raw).map(ClipEvent::Text))
}

/// Requests a target, waits for the `SelectionNotify`, then reads the property.
fn convert_and_read(&mut self, target: u32) -> Result<(u32, Vec<u8>), String> {
    let (conn, window, atoms) = (&self.conn, self.window, &self.atoms);
    // Clear first: a property left over from an earlier failure would look
    // like a reply.
    let _ = conn.delete_property(window, atoms.dest);
    conn.convert_selection(window, atoms.clipboard, target, atoms.dest, x11rb::CURRENT_TIME)
        .map_err(|e| e.to_string())?;
    conn.flush().map_err(|e| e.to_string())?;

    let clipboard = atoms.clipboard;
    let self_window = window;
    let notify = Self::wait_for(conn, &mut self.deferred, clipboard, self_window, |e| match e {
        Event::SelectionNotify(n) => Some(n.property),
        _ => None,
    })?;
    let (conn, window, atoms) = (&self.conn, self.window, &self.atoms);
    if notify == x11rb::NONE {
        // The owner cannot produce this format.
        return Ok((x11rb::NONE, Vec::new()));
    }

    let (type_, data) = Self::read_property(conn, window, atoms.dest)?;
    if type_ != atoms.incr {
        return Ok((type_, data));
    }

    // Incremental transfer: the content arrives in chunks, each announced by
    // a PropertyNotify, and an empty chunk ends the sequence.
    let mut out = Vec::new();
    loop {
        let dest = atoms.dest;
        Self::wait_for(conn, &mut self.deferred, clipboard, self_window, |e| match e {
            Event::PropertyNotify(p) if p.atom == dest && p.state == Property::NEW_VALUE => Some(()),
            _ => None,
        })?;
        let (conn, window, atoms) = (&self.conn, self.window, &self.atoms);
        let (_, chunk) = Self::read_property(conn, window, atoms.dest)?;
        if chunk.is_empty() {
            break;
        }
        if out.len() + chunk.len() > MAX_BYTES {
            return Err(format!("contenu > {} Mio, ignoré", MAX_BYTES / 1048576));
        }
        out.extend_from_slice(&chunk);
    }
    Ok((type_, out))
}

/// Reads a property in full, then deletes it, which INCR requires.
fn read_property(conn: &RustConnection, window: u32, prop: u32) -> Result<(u32, Vec<u8>), String> {
    let mut out = Vec::new();
    let mut offset = 0u32;
    let mut type_;
    loop {
        let reply = conn
            .get_property(false, window, prop, AtomEnum::ANY, offset, 4096)
            .map_err(|e| e.to_string())?
            .reply()
            .map_err(|e| e.to_string())?;
        type_ = reply.type_;
        let more = reply.bytes_after > 0;
        offset += (reply.value.len() / 4) as u32;
        out.extend_from_slice(&reply.value);
        if out.len() > MAX_BYTES {
            return Err(format!("contenu > {} Mio, ignoré", MAX_BYTES / 1048576));
        }
        if !more || reply.value.is_empty() {
            break;
        }
    }
    let _ = conn.delete_property(window, prop);
    let _ = conn.flush();
    Ok((type_, out))
}

/// Waits for an event matching `pick`, letting the others through.
fn wait_for<T>(
    conn: &RustConnection,
    deferred: &mut VecDeque<u32>,
    clipboard: u32,
    self_window: u32,
    mut pick: impl FnMut(&Event) -> Option<T>,
) -> Result<T, String> {
    let deadline = Instant::now() + REPLY_TIMEOUT;
    loop {
        while let Some(event) = conn.poll_for_event().map_err(|e| e.to_string())? {
            if let Some(v) = pick(&event) {
                return Ok(v);
            }
            // A copy happened while we were reading. Dropping it would lose a
            // history entry, so it is queued instead.
            if let Event::XfixesSelectionNotify(ev) = &event {
                if ev.selection == clipboard && ev.owner != x11rb::NONE && ev.owner != self_window {
                    deferred.push_back(ev.owner);
                }
            }
        }
        if Instant::now() >= deadline {
            return Err("pas de réponse du propriétaire de la sélection".into());
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Name of the source application: WM_CLASS, falling back to the process name
/// through `_NET_WM_PID`. The owning window is often an unmapped utility
/// window, so we walk up the tree.
fn describe_owner(&self, owner: u32) -> Option<String> {
    let (conn, atoms) = (&self.conn, &self.atoms);
    let mut win = owner;
    for _ in 0..8 {
        if let Ok(Ok(r)) = conn
            .get_property(false, win, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 256)
            .map(|c| c.reply())
        {
            if !r.value.is_empty() {
                let parts: Vec<&[u8]> = r.value.split(|b| *b == 0).filter(|s| !s.is_empty()).collect();
                if let Some(class) = parts.last() {
                    return Some(String::from_utf8_lossy(class).into_owned());
                }
            }
        }
        if let Ok(Ok(r)) = conn
            .get_property(false, win, atoms.net_wm_pid, AtomEnum::CARDINAL, 0, 1)
            .map(|c| c.reply())
        {
            if let Some(pid) = r.value32().and_then(|mut v| v.next()) {
                if let Ok(comm) = std::fs::read_to_string(format!("/proc/{pid}/comm")) {
                    return Some(comm.trim().to_string());
                }
            }
        }
        let tree = conn.query_tree(win).ok()?.reply().ok()?;
        if tree.parent == x11rb::NONE || tree.parent == tree.root {
            break;
        }
        win = tree.parent;
    }
    None
}

}
