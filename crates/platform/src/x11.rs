//! Backend X11 événementiel.
//!
//! XFixes nous réveille quand un autre client prend possession de la sélection
//! CLIPBOARD ; on demande alors TARGETS, on choisit le meilleur format
//! disponible, et on lit la propriété (INCR compris, pour les gros contenus).

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

/// Au-delà, on ignore : un presse-papier n'est pas un système de fichiers.
const MAX_BYTES: usize = 32 * 1024 * 1024;
/// Un client qui ne répond pas ne doit pas bloquer la capture suivante.
const REPLY_TIMEOUT: Duration = Duration::from_millis(1500);

struct Atoms {
    clipboard: u32,
    targets: u32,
    incr: u32,
    utf8_string: u32,
    text_plain_utf8: u32,
    image_png: u32,
    uri_list: u32,
    dest: u32,
    net_wm_pid: u32,
    /// Marqueurs des gestionnaires de mots de passe : on ne capture pas.
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
            uri_list: get("text/uri-list")?,
            dest: get("COPYCOPY_SELECTION")?,
            net_wm_pid: get("_NET_WM_PID")?,
            kde_password_hint: get("x-kde-passwordManagerHint")?,
            concealed: get("org.nspasteboard.ConcealedType")?,
        })
    }
}

pub fn spawn(tx: Sender<Capture>) -> Result<(), String> {
    // On se connecte ici pour échouer immédiatement si X11 n'est pas là,
    // et laisser `start()` basculer sur le sondage.
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
    // Les changements de sélection survenus pendant qu'on lisait : les traiter
    // en différé plutôt que les perdre (copies en rafale).
    let mut queue: VecDeque<u32> = VecDeque::new();
    // Certains environnements (le pont presse-papier de WSLg, les gestionnaires
    // de presse-papier tiers) reprennent la sélection juste après l'application
    // source : on reçoit alors plusieurs notifications pour une seule copie. On
    // n'en renvoie une deuxième que si elle apporte l'attribution qui manquait.
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

        // L'attribution se fait AVANT la lecture : une appli qui se ferme juste
        // après la copie ferait disparaître sa fenêtre entre-temps.
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
    /// Propriétaires signalés pendant une lecture, à traiter juste après.
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

    // Un gestionnaire de mots de passe annonce que le contenu est secret :
    // on n'y touche pas. C'est la règle la plus importante du projet.
    if targets.contains(&self.atoms.kde_password_hint) || targets.contains(&self.atoms.concealed) {
        return Ok(None);
    }

    if targets.contains(&self.atoms.image_png) {
        let (_, png) = self.convert_and_read(self.atoms.image_png)?;
        if png.is_empty() {
            return Ok(None);
        }
        let size = crate::png_size(&png);
        return Ok(Some(ClipEvent::Image { png, size }));
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
        // Certains clients n'annoncent pas TARGETS correctement : on tente
        // UTF8_STRING quand même plutôt que de ne rien capturer.
        .unwrap_or(self.atoms.utf8_string);

    let (_, raw) = self.convert_and_read(text_target)?;
    let text = String::from_utf8_lossy(&raw).into_owned();
    if text.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(ClipEvent::Text(text)))
}

/// Demande une cible, attend le `SelectionNotify`, puis lit la propriété.
fn convert_and_read(&mut self, target: u32) -> Result<(u32, Vec<u8>), String> {
    let (conn, window, atoms) = (&self.conn, self.window, &self.atoms);
    // On efface d'abord : une propriété restée d'un échec précédent ferait
    // croire à une réponse.
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
        // Le propriétaire ne sait pas produire ce format.
        return Ok((x11rb::NONE, Vec::new()));
    }

    let (type_, data) = Self::read_property(conn, window, atoms.dest)?;
    if type_ != atoms.incr {
        return Ok((type_, data));
    }

    // Transfert incrémental : le contenu arrive en tranches, chaque tranche
    // signalée par un PropertyNotify, et une tranche vide termine.
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

/// Lit une propriété en entier puis la supprime (nécessaire pour INCR).
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

/// Attend un événement qui satisfait `pick`, en laissant filer les autres.
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
            // Une copie est survenue pendant qu'on lisait : surtout ne pas la
            // jeter, c'est un élément d'historique qu'on perdrait.
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

/// Nom de l'application source : WM_CLASS, sinon le nom du process via
/// `_NET_WM_PID`. La fenêtre propriétaire est souvent une fenêtre technique
/// non mappée, donc on remonte l'arbre.
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
