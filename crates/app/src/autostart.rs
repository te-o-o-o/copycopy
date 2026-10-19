//! Starting with the session.
//!
//! Off by default, and a switch in the settings — never something to discover.
//! A clipboard manager is only useful if it is already running when the
//! shortcut is pressed, but deciding that for someone is not ours to make.
//!
//! | System  | Where the entry goes                            |
//! |---------|-------------------------------------------------|
//! | Windows | `HKCU\...\CurrentVersion\Run`, no administrator |
//! | Linux   | a `.desktop` file in `~/.config/autostart`      |
//! | macOS   | a launch agent                                  |
//!
//! The entry holds an absolute path, so it goes stale as soon as the executable
//! moves — which portable mode invites. `reconcile` rewrites it at every start.

use auto_launch::{AutoLaunch, AutoLaunchBuilder};

use crate::locale::Locale;

const APP_NAME: &str = "copycopy";

/// The resident starts without a window: the shortcut is what opens it.
/// Without this, every session would begin with the palette in the face.
const ARGS: [&str; 1] = ["--hidden"];

/// Why no entry can be written, worded by `say` in the interface's language.
#[derive(Debug)]
enum Blocked {
    NoExecutable(String),
    UnreadablePath,
    /// Only reachable on Windows, where a session starts before WSL does.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    NetworkPath(String),
}

impl Blocked {
    fn say(&self, l: Locale) -> String {
        match self {
            Self::NoExecutable(e) => {
                format!(
                    "{}{e}",
                    l.tr("exécutable introuvable : ", "executable not found: ")
                )
            }
            Self::UnreadablePath => l
                .tr(
                    "chemin de l'exécutable illisible",
                    "unreadable executable path",
                )
                .to_string(),
            Self::NetworkPath(path) => format!(
                "{} ({path}) — {}",
                l.tr(
                    "l'exécutable est sur un chemin réseau",
                    "the executable is on a network path"
                ),
                l.tr(
                    "copiez-le dans un dossier Windows",
                    "copy it into a Windows folder"
                ),
            ),
        }
    }
}

/// Whether the switch can be offered at all, and why not when it cannot.
pub fn availability(l: Locale) -> Result<(), String> {
    app_path().map(|_| ()).map_err(|why| why.say(l))
}

pub fn set(on: bool) -> Result<(), String> {
    let entry = entry()?;
    if on { entry.enable() } else { entry.disable() }.map_err(|e| e.to_string())
}

/// Brings the stored preference and the system into agreement at startup, and
/// returns what the preference should now be.
///
/// The two can disagree in both directions, and each is answered differently:
///
/// - the entry is there — rewrite it, so a moved executable stays reachable;
/// - the entry is gone while the preference says yes — someone removed it
///   outside the application, most likely from the Task Manager. Follow them.
///   Putting it back would make the switch impossible to turn off from there.
///
/// Where the entry cannot be read at all, the preference is left untouched
/// rather than quietly cleared.
pub fn reconcile(wanted: bool) -> bool {
    if !wanted {
        return false;
    }
    let Ok(entry) = entry() else { return true };
    match entry.is_enabled() {
        Ok(true) => {
            if let Err(e) = entry.enable() {
                eprintln!("autostart: could not refresh the entry: {e}");
            }
            true
        }
        Ok(false) => false,
        Err(e) => {
            eprintln!("autostart: could not read the entry: {e}");
            true
        }
    }
}

/// Errors go to the log and to a flash after the localised prefix, so they
/// stay in English, like the rest of what is not the interface's own text.
fn entry() -> Result<AutoLaunch, String> {
    let path = app_path().map_err(|why| why.say(Locale::En))?;
    let mut builder = AutoLaunchBuilder::new();
    builder
        .set_app_name(APP_NAME)
        .set_app_path(&path)
        .set_args(&ARGS);
    #[cfg(target_os = "windows")]
    // Per user, under `HKCU`. The default mode tries the machine-wide key
    // first, which asks for administrator rights we have no reason to want.
    builder.set_windows_enable_mode(auto_launch::WindowsEnableMode::CurrentUser);
    builder.build().map_err(|e| e.to_string())
}

/// The executable path as the entry must spell it.
fn app_path() -> Result<String, Blocked> {
    let exe = std::env::current_exe().map_err(|e| Blocked::NoExecutable(e.to_string()))?;
    let path = exe.to_str().ok_or(Blocked::UnreadablePath)?;

    #[cfg(target_os = "windows")]
    {
        // A session starts before WSL does, so an entry pointing inside it
        // fails silently at every login. Better to say so than to write one.
        if path.starts_with(r"\\") {
            return Err(Blocked::NetworkPath(path.to_string()));
        }
        // The registry value is one string, path and arguments separated by a
        // space, and `auto-launch` does not quote it. Without these quotes any
        // path holding a space — `C:\Users\Jean Dupont\...` — starts nothing.
        //
        // No `return`: once the other arm is stripped on Windows, this block is
        // the function's tail, and clippy rejects the keyword there.
        Ok(format!("\"{path}\""))
    }
    #[cfg(not(target_os = "windows"))]
    Ok(path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_entry_points_at_this_executable() {
        // Under a test runner `current_exe` is the harness binary, which is
        // enough: what matters is that a path is produced and carries the
        // quoting its platform needs.
        let path = app_path().expect("a path");
        assert!(!path.is_empty());
        if cfg!(target_os = "windows") {
            assert!(path.starts_with('"') && path.ends_with('"'));
        } else {
            assert!(path.starts_with('/'));
        }
    }
}
