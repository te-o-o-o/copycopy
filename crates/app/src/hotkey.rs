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

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};

pub struct Hotkeys {
    /// Kept alive: dropping it would unregister the shortcut.
    _manager: Option<GlobalHotKeyManager>,
    /// Message shown in the footer and at startup.
    pub status: String,
    pub registered: bool,
}

/// "Ctrl+Alt+V" becomes modifiers plus a key code.
pub fn parse(spec: &str) -> Option<HotKey> {
    let mut mods = Modifiers::empty();
    let mut code = None;

    for part in spec.split('+') {
        let part = part.trim();
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => mods |= Modifiers::CONTROL,
            "alt" | "option" => mods |= Modifiers::ALT,
            "shift" => mods |= Modifiers::SHIFT,
            "super" | "cmd" | "command" | "meta" | "win" => mods |= Modifiers::SUPER,
            _ => code = key_code(part),
        }
    }
    Some(HotKey::new(Some(mods), code?))
}

fn key_code(name: &str) -> Option<Code> {
    let upper = name.to_ascii_uppercase();
    let mut chars = upper.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if c.is_ascii_alphabetic() {
            return Code::from_str(&format!("Key{c}")).ok();
        }
        if c.is_ascii_digit() {
            return Code::from_str(&format!("Digit{c}")).ok();
        }
    }
    match upper.as_str() {
        "SPACE" => Some(Code::Space),
        "ENTER" | "RETURN" => Some(Code::Enter),
        "TAB" => Some(Code::Tab),
        // Remaining names follow the UI Events naming: F1, Escape and so on.
        _ => Code::from_str(name).ok(),
    }
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
