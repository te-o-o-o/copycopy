//! Capture du presse-papier. Une seule abstraction, plusieurs implémentations.
//!
//! - `x11` : événementiel via l'extension XFixes. Zéro sondage, latence nulle,
//!   et on récupère l'application source via WM_CLASS.
//! - `poll` : repli universel via `arboard` (Wayland, Windows, macOS).
//!
//! À venir : Wayland natif (`ext-data-control-v1`), Windows
//! (`AddClipboardFormatListener`), macOS (`NSPasteboard.changeCount`).

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

/// Un événement capturé, avec l'application qui en est à l'origine.
#[derive(Clone, Debug)]
pub struct Capture {
    pub event: ClipEvent,
    pub source: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BackendKind {
    /// Événementiel : le backend est réveillé par le système.
    X11Fixes,
    WaylandExt,
    WaylandWlr,
    Windows,
    /// macOS n'a pas d'événement de changement : `NSPasteboard` ne propose que
    /// `changeCount`. Le sondage n'y est pas un pis-aller, c'est l'API.
    MacOs,
    /// Repli universel.
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

    /// Vrai si l'OS nous réveille au lieu qu'on sonde.
    pub fn is_event_driven(self) -> bool {
        !matches!(self, BackendKind::Poll | BackendKind::MacOs)
    }
}

pub struct Watcher {
    pub rx: Receiver<Capture>,
    pub kind: BackendKind,
}

/// Démarre la capture dans un thread dédié.
///
/// `prefer` force un backend (`"wayland"`, `"x11"`, `"poll"`) ; sinon on choisit
/// le meilleur disponible pour la session courante. Le repli par sondage est
/// toujours le dernier recours : mieux vaut sonder que ne rien capturer.
pub fn start(prefer: Option<&str>) -> Result<Watcher, String> {
    let (tx, rx) = std::sync::mpsc::channel();

    #[cfg(target_os = "linux")]
    {
        let forced = prefer;
        let has_wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
        let has_x11 = std::env::var_os("DISPLAY").is_some();

        // Sous une session Wayland, Xwayland ne voit que le presse-papier des
        // applications X11 : data-control d'abord, toujours.
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

/// Dimensions lues directement dans l'en-tête IHDR, sans décoder l'image.
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
