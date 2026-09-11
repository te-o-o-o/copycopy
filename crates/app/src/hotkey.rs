//! Raccourci global d'ouverture.
//!
//! `global-hotkey` couvre Windows (`RegisterHotKey`), macOS
//! (`RegisterEventHotKey`) et X11 (`XGrabKey`). **Wayland n'a pas de raccourci
//! global côté client** : il faut soit le portail
//! `org.freedesktop.portal.GlobalShortcuts`, soit — et c'est le repli universel
//! qu'on implémente ici — un raccourci défini dans le compositeur qui lance
//! `copycopy-iced --show`, lequel passe par l'IPC.
//!
//! Contrainte de thread : sur macOS le gestionnaire doit être créé sur le
//! thread principal, sur Windows sur le thread qui porte la boucle
//! d'événements. On le crée donc dans `boot()`, qui tourne sur le thread
//! principal de iced, et on le garde vivant dans l'état.

use std::str::FromStr;

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};

pub struct Hotkeys {
    /// Gardé vivant : le lâcher désenregistrerait le raccourci.
    _manager: Option<GlobalHotKeyManager>,
    /// Message affiché en pied de fenêtre et au démarrage.
    pub status: String,
    pub registered: bool,
}

/// « Ctrl+Alt+V » → modificateurs + code de touche.
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
        // Les noms restants suivent la nomenclature UI Events : F1, Escape…
        _ => Code::from_str(name).ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raccourcis_courants() {
        assert!(parse("Ctrl+Alt+V").is_some());
        assert!(parse("Cmd+Shift+V").is_some());
        assert!(parse("Super+Space").is_some());
        assert!(parse("Ctrl+Alt+F1").is_some());
        assert!(parse("Ctrl+Alt").is_none(), "aucune touche : doit échouer");
        assert!(parse("Ctrl+Alt+Zzz").is_none(), "touche inconnue");
    }
}

/// Sous Wayland, `XGrabKey` ne voit que les applications X11 : le raccourci ne
/// se déclenchera pas depuis une application Wayland native.
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
            let mut status = format!("{spec}");
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

/// Le récepteur de `global-hotkey` est global et lisible depuis n'importe quel
/// thread : on le draine dans un thread dédié.
pub fn spawn_bridge(on_press: impl Fn() + Send + 'static) {
    std::thread::Builder::new()
        .name("copycopy-hotkey".into())
        .spawn(move || {
            let receiver = GlobalHotKeyEvent::receiver();
            while let Ok(event) = receiver.recv() {
                // Seul l'appui nous intéresse, pas le relâchement.
                if event.state == global_hotkey::HotKeyState::Pressed {
                    on_press();
                }
            }
        })
        .expect("thread raccourci");
}
