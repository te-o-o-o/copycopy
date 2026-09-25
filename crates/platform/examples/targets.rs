//! Says what the current clipboard owner offers, and what our reader actually
//! gets out of each format.
//!
//! Written for the case where a copy produces no history entry at all: the
//! backend is silent about a format it advertised but could not read, so there
//! is otherwise nothing to look at.
//!
//!   copy something, then:
//!   cargo run --release -p copycopy-platform --example targets
//!
//! Linux only. The tool drives an X11 or Wayland server, so it has nothing to
//! talk to elsewhere — but it still has to *build* everywhere, or
//! `--all-targets` breaks the Windows and macOS runs of the CI.

#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use x11rb::connection::Connection;
#[cfg(target_os = "linux")]
use x11rb::protocol::xproto::{
    AtomEnum, ConnectionExt as _, CreateWindowAux, EventMask, Property, WindowClass,
};
#[cfg(target_os = "linux")]
use x11rb::protocol::Event;
#[cfg(target_os = "linux")]
use x11rb::rust_connection::RustConnection;
#[cfg(target_os = "linux")]
use x11rb::COPY_DEPTH_FROM_PARENT;

/// The same budget the backend gives an owner, so a timeout here means a
/// timeout there.
#[cfg(target_os = "linux")]
const REPLY_TIMEOUT: Duration = Duration::from_millis(1500);

#[cfg(target_os = "linux")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (conn, screen_num) = x11rb::connect(None)?;
    let screen = &conn.setup().roots[screen_num];
    let window = conn.generate_id()?;
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
    )?;

    let atom = |name: &str| -> Result<u32, Box<dyn std::error::Error>> {
        Ok(conn.intern_atom(false, name.as_bytes())?.reply()?.atom)
    };
    let clipboard = atom("CLIPBOARD")?;
    let targets_atom = atom("TARGETS")?;
    let dest = atom("COPYCOPY_PROBE")?;
    let incr = atom("INCR")?;

    let owner = conn.get_selection_owner(clipboard)?.reply()?.owner;
    if owner == x11rb::NONE {
        println!("personne ne possède CLIPBOARD — rien n'a été copié");
        return Ok(());
    }
    println!("propriétaire : fenêtre 0x{owner:x}\n");

    let raw = read(&conn, window, clipboard, dest, incr, targets_atom)?;
    let (_, bytes) = match raw {
        Some(v) => v,
        None => {
            println!("le propriétaire ne répond pas à TARGETS");
            return Ok(());
        }
    };
    let targets: Vec<u32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_ne_bytes(*c))
        .collect();

    println!("{} format(s) annoncé(s) :\n", targets.len());
    for target in targets {
        let name = conn
            .get_atom_name(target)
            .ok()
            .and_then(|c| c.reply().ok())
            .map(|r| String::from_utf8_lossy(&r.name).into_owned())
            .unwrap_or_else(|| format!("atome {target}"));

        // TARGETS itself, and the pure metadata formats, are not content.
        if matches!(
            name.as_str(),
            "TARGETS" | "TIMESTAMP" | "MULTIPLE" | "SAVE_TARGETS"
        ) {
            println!("  {name:<28} —");
            continue;
        }

        let started = Instant::now();
        let verdict = match read(&conn, window, clipboard, dest, incr, target) {
            Ok(Some((_, data))) if data.is_empty() => "vide".to_string(),
            Ok(Some((_, data))) => {
                let decoded = if name.starts_with("image/") {
                    match image::load_from_memory(&data) {
                        Ok(img) => format!(", décodée {}×{}", img.width(), img.height()),
                        Err(e) => format!(", NON DÉCODABLE ({e})"),
                    }
                } else {
                    String::new()
                };
                format!("{} octets{}", data.len(), decoded)
            }
            Ok(None) => "REFUSÉ par le propriétaire".to_string(),
            Err(e) => format!("ÉCHEC : {e}"),
        };
        println!(
            "  {name:<28} {verdict}  [{} ms]",
            started.elapsed().as_millis()
        );
    }

    println!(
        "\nRappel : le backend essaie image/png, image/bmp, image/jpeg, image/tiff\n\
         dans cet ordre, puis text/uri-list, puis le texte. Si un format image est\n\
         annoncé mais qu'aucun ne se lit, la copie entière est abandonnée."
    );
    Ok(())
}

/// What one target yielded: its property type and the bytes, or `None` when
/// the owner refused the format.
#[cfg(target_os = "linux")]
type Offer = Option<(u32, Vec<u8>)>;

/// Requests one target and reads the reply, INCR included — the same sequence
/// the backend follows.
#[cfg(target_os = "linux")]
fn read(
    conn: &RustConnection,
    window: u32,
    clipboard: u32,
    dest: u32,
    incr: u32,
    target: u32,
) -> Result<Offer, Box<dyn std::error::Error>> {
    let _ = conn.delete_property(window, dest);
    conn.convert_selection(window, clipboard, target, dest, x11rb::CURRENT_TIME)?;
    conn.flush()?;

    let deadline = Instant::now() + REPLY_TIMEOUT;
    let property = loop {
        if let Some(Event::SelectionNotify(n)) = conn.poll_for_event()? {
            break n.property;
        }
        if Instant::now() >= deadline {
            return Err("pas de réponse du propriétaire".into());
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    if property == x11rb::NONE {
        return Ok(None);
    }

    let (type_, data) = read_property(conn, window, dest)?;
    if type_ != incr {
        return Ok(Some((type_, data)));
    }

    let mut out = Vec::new();
    loop {
        let deadline = Instant::now() + REPLY_TIMEOUT;
        loop {
            match conn.poll_for_event()? {
                Some(Event::PropertyNotify(p))
                    if p.atom == dest && p.state == Property::NEW_VALUE =>
                {
                    break
                }
                Some(_) => continue,
                None => {
                    if Instant::now() >= deadline {
                        return Err("transfert INCR interrompu".into());
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
        let (_, chunk) = read_property(conn, window, dest)?;
        if chunk.is_empty() {
            break;
        }
        out.extend_from_slice(&chunk);
    }
    Ok(Some((type_, out)))
}

#[cfg(target_os = "linux")]
fn read_property(
    conn: &RustConnection,
    window: u32,
    prop: u32,
) -> Result<(u32, Vec<u8>), Box<dyn std::error::Error>> {
    let mut out = Vec::new();
    let mut offset = 0u32;
    let mut type_ = None;
    loop {
        let reply = conn
            .get_property(false, window, prop, AtomEnum::ANY, offset, 4096)?
            .reply()?;
        type_.get_or_insert(reply.type_);
        let more = reply.bytes_after > 0;
        offset += (reply.value.len() / 4) as u32;
        out.extend_from_slice(&reply.value);
        if !more || reply.value.is_empty() {
            break;
        }
    }
    let _ = conn.delete_property(window, prop);
    let _ = conn.flush();
    Ok((type_.unwrap_or(x11rb::NONE), out))
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("targets : lecteur du presse-papier X11, sans objet sur ce système");
}
