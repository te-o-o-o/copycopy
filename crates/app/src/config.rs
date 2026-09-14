//! Configuration: a deliberately minimal `key = value` file.
//!
//! No TOML and no serde for now — five keys do not justify the dependency nor
//! the compile time. To be replaced the day the configuration becomes
//! structured.

use std::path::{Path, PathBuf};

pub const DEFAULT_HOTKEY: &str = if cfg!(target_os = "macos") {
    "Cmd+Shift+V"
} else {
    "Ctrl+Alt+V"
};

#[derive(Debug, Clone)]
pub struct Config {
    /// Global shortcut that opens the window, e.g. "Ctrl+Alt+V".
    pub hotkey: String,
    /// Remembered size. Always reliable: every system reports it.
    pub size: Option<(f32, f32)>,
    /// Remembered position, deliberately kept apart from the size: some
    /// environments (WSLg) never report a real position and would return 0,0,
    /// which would open the window in a corner instead of centred. It is only
    /// written once an actual move has been observed.
    pub position: Option<(f32, f32)>,
    /// Light or dark palette, switched from the header icon.
    pub theme: crate::theme::Mode,
    /// Paste the copied entry into the application that had the focus.
    /// Off by default: simulating keystrokes in another application is
    /// something to ask for, never to discover.
    pub auto_paste: bool,
    /// Start with the session. Off by default: a program that installs itself
    /// into someone's login is a program that asked first.
    pub autostart: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: DEFAULT_HOTKEY.to_string(),
            size: None,
            position: None,
            theme: crate::theme::Mode::Dark,
            auto_paste: false,
            autostart: false,
        }
    }
}

const FILE: &str = "copycopy.conf";

/// Where everything lives: configuration, database, images.
///
/// **Portable mode**: if a configuration file sits next to the executable, that
/// directory is used for all of it — drop the binary on a USB stick, create an
/// empty `copycopy.conf` beside it, and nothing touches the host machine.
/// Otherwise the usual per-user directories apply.
pub fn base_dir() -> Option<PathBuf> {
    if let Some(beside) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        if beside.join(FILE).exists() {
            return Some(beside);
        }
    }
    directories::ProjectDirs::from("", "", "copycopy").map(|d| d.config_dir().to_path_buf())
}

pub fn path() -> Option<PathBuf> {
    base_dir().map(|dir| dir.join(FILE))
}

impl Config {
    /// Loads the configuration, or returns the defaults. An unknown key or an
    /// unreadable line is ignored: a damaged file must never stop the
    /// application from starting.
    pub fn load() -> Self {
        let mut config = Self::default();
        let Some(path) = path() else {
            return config;
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return config;
        };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim().trim_matches('"');
            match key.trim() {
                "hotkey" if !value.is_empty() => config.hotkey = value.to_string(),
                "size" => config.size = parse_pair(value).filter(|(w, h)| *w >= 320.0 && *h >= 200.0),
                "position" => config.position = parse_pair(value),
                "theme" => config.theme = crate::theme::Mode::parse(value).unwrap_or(config.theme),
                "auto_paste" => config.auto_paste = truthy(value),
                "autostart" => config.autostart = truthy(value),
                _ => {}
            }
        }
        config
    }

    /// Writes the file when it does not exist yet, so the user has something
    /// to edit rather than a blank page.
    pub fn write_default_if_missing(&self) -> Option<PathBuf> {
        let path = path()?;
        if path.exists() {
            return Some(path);
        }
        self.save();
        Some(path)
    }

    /// Rewrites the whole file. Losing the configuration is never a reason to
    /// fail anything, so errors are ignored.
    pub fn save(&self) {
        let Some(path) = path() else { return };
        let Some(parent) = path.parent() else { return };
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
        let mut body = String::from(
            "# Configuration copycopy\n\
             # Raccourci global d'ouverture.\n\
             # Modificateurs : Ctrl, Alt, Shift, Super (ou Cmd).\n",
        );
        body.push_str(&format!("hotkey = \"{}\"\n", self.hotkey));
        if let Some((w, h)) = self.size {
            body.push_str("# Taille retenue : largeur,hauteur\n");
            body.push_str(&format!("size = {w:.0},{h:.0}\n"));
        }
        if let Some((x, y)) = self.position {
            body.push_str("# Position retenue : x,y\n");
            body.push_str(&format!("position = {x:.0},{y:.0}\n"));
        }
        body.push_str("# Thème : dark, light, purpledream, aalto ou matrix\n");
        body.push_str(&format!("theme = {}\n", self.theme.name()));
        body.push_str("# Coller automatiquement après une copie : true ou false\n");
        body.push_str(&format!("auto_paste = {}\n", self.auto_paste));
        body.push_str("# Lancer copycopy à l'ouverture de session : true ou false\n");
        body.push_str(&format!("autostart = {}\n", self.autostart));
        let _ = std::fs::write(&path, body);
    }
}

/// A switch is on when it says so in any of the spellings someone editing the
/// file by hand would reach for. Anything else is off.
fn truthy(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "true" | "yes" | "on" | "1"
    )
}

fn parse_pair(value: &str) -> Option<(f32, f32)> {
    let nums: Vec<f32> = value
        .split(',')
        .filter_map(|n| n.trim().parse::<f32>().ok())
        .collect();
    match nums.as_slice() {
        [a, b] => Some((*a, *b)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairs() {
        assert_eq!(parse_pair("760, 520"), Some((760.0, 520.0)));
        assert_eq!(parse_pair("abc"), None);
        assert_eq!(parse_pair("1,2,3"), None);
    }

    #[test]
    fn switches_read_the_usual_spellings() {
        for on in ["true", "TRUE", "yes", "on", "1"] {
            assert!(truthy(on), "{on} should be on");
        }
        for off in ["false", "no", "off", "0", "", "peut-être"] {
            assert!(!truthy(off), "{off} should be off");
        }
    }

    #[test]
    fn absurd_size_is_rejected() {
        let mut c = Config::default();
        for line in ["size = 10,10", "size = 760,520"] {
            let (k, v) = line.split_once('=').expect("format");
            if k.trim() == "size" {
                c.size = parse_pair(v.trim()).filter(|(w, h)| *w >= 320.0 && *h >= 200.0);
            }
        }
        assert_eq!(c.size, Some((760.0, 520.0)));
    }
}
