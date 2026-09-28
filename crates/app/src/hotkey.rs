//! The global shortcut that opens the window.
//!
//! `global-hotkey` covers Windows (`RegisterHotKey`), macOS
//! (`RegisterEventHotKey`) and X11 (`XGrabKey`). **Wayland has no client-side
//! global shortcut**: it takes either the
//! `org.freedesktop.portal.GlobalShortcuts` portal, or — and this is the
//! universal fallback implemented here — a shortcut defined in the compositor
//! that runs `copycopy --show`, which goes through the IPC.
//!
//! Thread constraint: on macOS the manager must be created on the main thread,
//! on Windows on the thread that owns the event loop. It is therefore created
//! in `boot()`, which runs on iced's main thread, and kept alive in the state.

use std::str::FromStr;

use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};

pub struct Hotkeys {
    /// Kept alive: dropping it would unregister the shortcut.
    _manager: Option<GlobalHotKeyManager>,
    /// Message printed at startup.
    pub status: String,
    pub registered: bool,
}

/// "Ctrl+Alt+V" becomes modifiers plus a key code. `global-hotkey` parses
/// that itself, aliases included: Ctrl/Control, Alt/Option, Cmd/Super.
pub fn parse(spec: &str) -> Option<HotKey> {
    HotKey::from_str(spec).ok()
}

/// Under Wayland, `XGrabKey` only sees X11 applications, so the shortcut will
/// not fire from a native Wayland application.
fn wayland_session() -> bool {
    std::env::var("XDG_SESSION_TYPE")
        .map(|v| v.eq_ignore_ascii_case("wayland"))
        .unwrap_or(false)
        || std::env::var_os("WAYLAND_DISPLAY").is_some()
}

pub fn register(spec: &str) -> Hotkeys {
    let Some(hotkey) = parse(spec) else {
        return Hotkeys {
            _manager: None,
            status: format!("raccourci « {spec} » illisible"),
            registered: false,
        };
    };

    let manager = match GlobalHotKeyManager::new() {
        Ok(m) => m,
        Err(e) => {
            return Hotkeys {
                _manager: None,
                status: format!("raccourci global indisponible ({e})"),
                registered: false,
            }
        }
    };

    match manager.register(hotkey) {
        Ok(()) => {
            let mut status = spec.to_string();
            if cfg!(target_os = "linux") && wayland_session() {
                status.push_str(" (X11 seulement — voir --show sous Wayland)");
            }
            Hotkeys {
                _manager: Some(manager),
                status,
                registered: true,
            }
        }
        Err(e) => Hotkeys {
            _manager: None,
            status: format!("{spec} refusé ({e})"),
            registered: false,
        },
    }
}

/// The `global-hotkey` receiver is global and readable from any thread, so it
/// is drained on a dedicated one.
pub fn spawn_bridge(on_press: impl Fn() + Send + 'static) {
    std::thread::Builder::new()
        .name("copycopy-hotkey".into())
        .spawn(move || {
            let receiver = GlobalHotKeyEvent::receiver();
            while let Ok(event) = receiver.recv() {
                // Only the press matters, not the release.
                if event.state == global_hotkey::HotKeyState::Pressed {
                    on_press();
                }
            }
        })
        .expect("thread raccourci");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_shortcuts() {
        assert!(parse("Ctrl+Alt+V").is_some());
        assert!(parse("Cmd+Shift+V").is_some());
        assert!(parse("Super+Space").is_some());
        assert!(parse("Ctrl+Alt+F1").is_some());
        assert!(parse("Ctrl+Alt").is_none(), "no key: must fail");
        assert!(parse("Ctrl+Alt+Zzz").is_none(), "unknown key");
    }
}
