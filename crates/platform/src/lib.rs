//! Clipboard capture. One abstraction, several implementations.
//!
//! - `x11`: event-driven through the XFixes extension. No polling, no latency,
//!   and the source application comes from WM_CLASS.
//! - `wayland`: event-driven through a data-control protocol.
//! - `windows`, `macos`: the native mechanism of each platform.
//! - `poll`: universal fallback through `arboard`.

use std::sync::mpsc::Receiver;

use copycopy_core::ClipEvent;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod wayland;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
mod x11;

mod poll;
mod setter;

pub use setter::Setter;

/// A captured event, along with the application it came from.
#[derive(Clone, Debug)]
pub struct Capture {
    pub event: ClipEvent,
    pub source: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BackendKind {
    /// Event-driven: the system wakes the backend up.
    X11Fixes,
    WaylandExt,
    WaylandWlr,
    Windows,
    /// macOS has no change event: `NSPasteboard` only offers `changeCount`.
    /// Polling is not a workaround there, it is the API.
    MacOs,
    /// Universal fallback.
    Poll,
}

impl BackendKind {
    pub fn label(self) -> &'static str {
        match self {
            BackendKind::X11Fixes => "X11/XFixes (événementiel)",
            BackendKind::WaylandExt => "Wayland/ext-data-control (événementiel)",
            BackendKind::WaylandWlr => "Wayland/wlr-data-control (événementiel)",
            BackendKind::Windows => "Windows/ClipboardFormatListener (événementiel)",
            BackendKind::MacOs => "macOS/NSPasteboard changeCount (200 ms)",
            BackendKind::Poll => "arboard (sondage 200 ms)",
        }
    }

    /// True when the OS wakes us up instead of us polling.
    pub fn is_event_driven(self) -> bool {
        !matches!(self, BackendKind::Poll | BackendKind::MacOs)
    }
}

pub struct Watcher {
    pub rx: Receiver<Capture>,
    pub kind: BackendKind,
}

/// Starts capture on a dedicated thread.
///
/// `prefer` forces a backend (`"wayland"`, `"x11"`, `"poll"`); otherwise the
/// best one available for the current session is chosen. Polling is always the
/// last resort: better to poll than to capture nothing at all.
pub fn start(prefer: Option<&str>) -> Result<Watcher, String> {
    let (tx, rx) = std::sync::mpsc::channel();

    #[cfg(target_os = "linux")]
    {
        let forced = prefer;
        let has_wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
        let has_x11 = std::env::var_os("DISPLAY").is_some();

        // In a Wayland session, Xwayland only sees the clipboard of X11
        // applications: always try data-control first.
        if forced == Some("wayland") || (forced.is_none() && has_wayland) {
            match wayland::probe() {
                Ok(variant) => match wayland::spawn(variant, tx.clone()) {
                    Ok(()) => {
                        return Ok(Watcher {
                            rx,
                            kind: match variant {
                                wayland::Variant::Ext => BackendKind::WaylandExt,
                                wayland::Variant::Wlr => BackendKind::WaylandWlr,
                            },
                        })
                    }
                    Err(e) => eprintln!("backend Wayland indisponible ({e})"),
                },
                Err(e) => eprintln!("Wayland : {e}"),
            }
        }

        if forced == Some("x11") || (forced != Some("poll") && has_x11) {
            match x11::spawn(tx.clone()) {
                Ok(()) => {
                    return Ok(Watcher {
                        rx,
                        kind: BackendKind::X11Fixes,
                    })
                }
                Err(e) => eprintln!("backend X11 indisponible ({e})"),
            }
        }
    }

    #[cfg(target_os = "windows")]
    if prefer != Some("poll") {
        match windows::spawn(tx.clone()) {
            Ok(()) => {
                return Ok(Watcher {
                    rx,
                    kind: BackendKind::Windows,
                })
            }
            Err(e) => eprintln!("backend Windows indisponible ({e})"),
        }
    }

    #[cfg(target_os = "macos")]
    if prefer != Some("poll") {
        match macos::spawn(tx.clone()) {
            Ok(()) => {
                return Ok(Watcher {
                    rx,
                    kind: BackendKind::MacOs,
                })
            }
            Err(e) => eprintln!("backend macOS indisponible ({e})"),
        }
    }

    eprintln!("repli sur le sondage périodique");
    poll::spawn(tx)?;
    Ok(Watcher {
        rx,
        kind: BackendKind::Poll,
    })
}

/// A normalised picture: PNG bytes, plus its dimensions when they are known.
///
/// This and the two helpers below serve the Linux backends only: X11 and
/// Wayland receive MIME types and have to sort bytes out themselves, whereas
/// Windows and macOS are handed typed formats and convert in their own module.
#[cfg(target_os = "linux")]
pub(crate) type Png = (Vec<u8>, Option<(u32, u32)>);

/// Normalises whatever image bytes the clipboard offered into PNG, which is the
/// single format kept internally. PNG is passed through untouched; anything
/// else is decoded and re-encoded.
#[cfg(target_os = "linux")]
pub(crate) fn to_png(bytes: Vec<u8>) -> Option<Png> {
    if bytes.is_empty() {
        return None;
    }
    if png_size(&bytes).is_some() {
        let size = png_size(&bytes);
        return Some((bytes, size));
    }
    let decoded = image::load_from_memory(&bytes).ok()?;
    let size = (decoded.width(), decoded.height());
    let mut png = Vec::new();
    decoded
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;
    Some((png, Some(size)))
}

/// Clipboard text that is not valid UTF-8, or carries NUL bytes, is not text:
/// it is binary that reached the wrong branch. Storing it would fill the
/// history with unreadable entries.
#[cfg(target_os = "linux")]
pub(crate) fn sane_text(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8(bytes.to_vec()).ok()?;
    if text.trim().is_empty() || text.contains('\0') {
        return None;
    }
    Some(text)
}

/// Dimensions read straight from the IHDR header, without decoding the image.
pub(crate) fn png_size(png: &[u8]) -> Option<(u32, u32)> {
    if png.len() < 24 || &png[..8] != b"\x89PNG\r\n\x1a\n" || &png[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(png[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(png[20..24].try_into().ok()?);
    Some((w, h))
}

#[allow(dead_code)]
pub(crate) fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
