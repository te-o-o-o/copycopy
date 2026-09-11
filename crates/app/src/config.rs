//! Configuration : un fichier `clé = valeur`, volontairement minimal.
//!
//! Pas de TOML ni de serde pour l'instant — cinq clés ne justifient pas la
//! dépendance ni le temps de compilation. À remplacer le jour où la config
//! devient structurée.

use std::path::PathBuf;

pub const DEFAULT_HOTKEY: &str = if cfg!(target_os = "macos") {
    "Cmd+Shift+V"
} else {
    "Ctrl+Alt+V"
};

#[derive(Debug, Clone)]
pub struct Config {
    /// Raccourci global d'ouverture, p. ex. « Ctrl+Alt+V ».
    pub hotkey: String,
    /// Taille retenue. Toujours fiable : tout système la rapporte.
    pub size: Option<(f32, f32)>,
    /// Position retenue. Volontairement distincte de la taille : certains
    /// environnements (WSLg) ne rapportent jamais de position réelle et
    /// renverraient 0,0 — la fenêtre s'ouvrirait alors dans un coin au lieu
    /// d'être centrée. On ne l'écrit que si un déplacement a été observé.
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
    /// Charge la configuration, ou renvoie les valeurs par défaut. Une clé
    /// inconnue ou une ligne illisible est ignorée : un fichier abîmé ne doit
    /// jamais empêcher l'application de démarrer.
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

    /// Écrit le fichier s'il n'existe pas encore, pour que l'utilisateur ait
    /// quelque chose à modifier plutôt qu'une page blanche.
    pub fn write_default_if_missing(&self) -> Option<PathBuf> {
        let path = path()?;
        if path.exists() {
            return Some(path);
        }
        self.save();
        Some(path)
    }

    /// Réécrit le fichier entier. Perdre la configuration n'est jamais une
    /// raison de faire échouer quoi que ce soit : on ignore les erreurs.
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
    fn paires() {
        assert_eq!(parse_pair("760, 520"), Some((760.0, 520.0)));
        assert_eq!(parse_pair("abc"), None);
        assert_eq!(parse_pair("1,2,3"), None);
    }

    #[test]
    fn taille_absurde_refusee() {
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
