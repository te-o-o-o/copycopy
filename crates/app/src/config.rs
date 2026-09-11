//! Configuration: a deliberately minimal `key = value` file.
//!
//! No TOML and no serde for now — five keys do not justify the dependency nor
//! the compile time. To be replaced the day the configuration becomes
//! structured.

use std::path::PathBuf;

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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: DEFAULT_HOTKEY.to_string(),
            size: None,
            position: None,
        }
    }
}

pub fn path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "copycopy")
        .map(|dirs| dirs.config_dir().join("copycopy.conf"))
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
        let _ = std::fs::write(&path, body);
    }
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
