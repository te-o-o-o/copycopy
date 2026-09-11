//! Modèle de données et historique. Aucune dépendance OS, aucune dépendance UI.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Text,
    Url,
    Code,
    Image,
    Files,
}

/// Ce qu'un backend remonte quand le presse-papier change.
#[derive(Clone, Debug)]
pub enum ClipEvent {
    Text(String),
    /// PNG brut tel que fourni par la sélection, + dimensions si connues.
    Image {
        png: Vec<u8>,
        size: Option<(u32, u32)>,
    },
    Files(Vec<PathBuf>),
}

#[derive(Clone, Debug)]
pub enum Payload {
    Text(String),
    Image { png: Vec<u8>, size: Option<(u32, u32)> },
    Files(Vec<PathBuf>),
}

#[derive(Clone, Debug)]
pub struct ClipItem {
    pub id: u64,
    pub kind: Kind,
    pub payload: Payload,
    /// Ligne unique affichée dans la liste.
    pub preview: String,
    pub source: String,
    pub at: SystemTime,
    pub hash: u64,
    pub pinned: bool,
}

impl ClipItem {
    pub fn age(&self) -> String {
        let secs = SystemTime::now()
            .duration_since(self.at)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        match secs {
            0..=4 => "à l'instant".to_string(),
            5..=59 => format!("{secs} s"),
            60..=3599 => format!("{} min", secs / 60),
            3600..=86399 => format!("{} h", secs / 3600),
            _ => format!("{} j", secs / 86400),
        }
    }
}

/// Historique borné, dédupliqué par hash de contenu.
pub struct History {
    items: VecDeque<ClipItem>,
    capacity: usize,
    next_id: u64,
}

impl History {
    pub fn new(capacity: usize) -> Self {
        Self {
            items: VecDeque::with_capacity(capacity.min(1024)),
            capacity,
            next_id: 1,
        }
    }

    pub fn items(&self) -> &VecDeque<ClipItem> {
        &self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&ClipItem> {
        self.items.get(index)
    }

    /// Insère un événement. Renvoie `false` si c'était un doublon de contenu :
    /// dans ce cas l'entrée existante remonte en tête au lieu d'être dupliquée.
    pub fn push(&mut self, event: ClipEvent, source: String) -> bool {
        let hash = hash_event(&event);

        if let Some(pos) = self.items.iter().position(|i| i.hash == hash) {
            let mut existing = self.items.remove(pos).expect("position valide");
            existing.at = SystemTime::now();
            existing.source = source;
            self.items.push_front(existing);
            return false;
        }

        let (kind, preview, payload) = match event {
            ClipEvent::Text(t) => {
                let kind = classify(&t);
                let preview = one_line(&t);
                (kind, preview, Payload::Text(t))
            }
            ClipEvent::Image { png, size } => {
                let preview = match size {
                    Some((w, h)) => format!("Image {w}×{h} · PNG · {}", human_bytes(png.len())),
                    None => format!("Image · PNG · {}", human_bytes(png.len())),
                };
                (Kind::Image, preview, Payload::Image { png, size })
            }
            ClipEvent::Files(paths) => {
                let preview = match paths.split_first() {
                    Some((first, [])) => first.display().to_string(),
                    Some((first, rest)) => {
                        format!("{} (+{} autres)", first.display(), rest.len())
                    }
                    None => "aucun fichier".to_string(),
                };
                (Kind::Files, preview, Payload::Files(paths))
            }
        };

        let id = self.next_id;
        self.next_id += 1;
        self.items.push_front(ClipItem {
            id,
            kind,
            payload,
            preview,
            source,
            at: SystemTime::now(),
            hash,
            pinned: false,
        });

        while self.items.len() > self.capacity {
            // On ne jette jamais une entrée épinglée.
            match self.items.iter().rposition(|i| !i.pinned) {
                Some(pos) => {
                    self.items.remove(pos);
                }
                None => break,
            }
        }
        true
    }

    pub fn toggle_pin(&mut self, index: usize) {
        if let Some(item) = self.items.get_mut(index) {
            item.pinned = !item.pinned;
        }
    }

    pub fn remove(&mut self, index: usize) {
        self.items.remove(index);
    }

    pub fn clear_unpinned(&mut self) {
        self.items.retain(|i| i.pinned);
    }
}

/// FNV-1a 64 bits : stable entre exécutions (contrairement à `DefaultHasher`),
/// donc réutilisable tel quel quand on passera à la persistance SQLite.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

pub fn hash_event(event: &ClipEvent) -> u64 {
    match event {
        // Le texte est normalisé : « Bonjour » et « Bonjour\n » sont le même clip.
        ClipEvent::Text(t) => fnv1a(t.trim().as_bytes()),
        ClipEvent::Image { png, .. } => fnv1a(png),
        ClipEvent::Files(paths) => {
            let joined = paths
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join("\n");
            fnv1a(joined.as_bytes())
        }
    }
}

/// Aperçu sur une seule ligne : les retours et tabulations deviennent des
/// espaces, les blancs consécutifs sont écrasés.
fn one_line(text: &str) -> String {
    let mut out = String::with_capacity(text.len().min(256));
    let mut last_space = false;
    for c in text.trim().chars().take(512) {
        let c = if c.is_whitespace() { ' ' } else { c };
        if c == ' ' {
            if !last_space {
                out.push(' ');
            }
            last_space = true;
        } else {
            out.push(c);
            last_space = false;
        }
    }
    out
}

fn classify(text: &str) -> Kind {
    let t = text.trim();
    if t.is_empty() {
        return Kind::Text;
    }
    let is_url = !t.contains(char::is_whitespace)
        && ["http://", "https://", "ftp://", "ssh://", "postgres://", "mysql://", "file://"]
            .iter()
            .any(|p| t.starts_with(p));
    if is_url {
        return Kind::Url;
    }
    // Heuristique volontairement grossière : on préfère rater du code que de
    // taguer du texte courant comme du code.
    let code_markers = [
        "fn ", "let ", "const ", "function ", "class ", "def ", "import ", "SELECT ", "#include",
        "=>", "->", "();", "{\n", ";\n",
    ];
    let hits = code_markers.iter().filter(|m| t.contains(**m)).count();
    if hits >= 2 || (hits >= 1 && t.contains('\n')) {
        return Kind::Code;
    }
    Kind::Text
}

fn human_bytes(n: usize) -> String {
    const K: f64 = 1024.0;
    let n = n as f64;
    if n < K {
        format!("{n:.0} o")
    } else if n < K * K {
        format!("{:.1} Kio", n / K)
    } else {
        format!("{:.1} Mio", n / (K * K))
    }
}

pub fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedup_remonte_en_tete_sans_dupliquer() {
        let mut h = History::new(10);
        assert!(h.push(ClipEvent::Text("a".into()), "t".into()));
        assert!(h.push(ClipEvent::Text("b".into()), "t".into()));
        // Même contenu, aux blancs près.
        assert!(!h.push(ClipEvent::Text("a\n".into()), "t".into()));
        assert_eq!(h.len(), 2);
        assert_eq!(h.get(0).unwrap().preview, "a");
    }

    #[test]
    fn capacite_respectee_et_epingles_preserves() {
        let mut h = History::new(3);
        for i in 0..3 {
            h.push(ClipEvent::Text(format!("t{i}")), "t".into());
        }
        h.toggle_pin(2); // « t0 », le plus ancien
        for i in 3..8 {
            h.push(ClipEvent::Text(format!("t{i}")), "t".into());
        }
        assert_eq!(h.len(), 3);
        assert!(h.items().iter().any(|i| i.preview == "t0" && i.pinned));
    }

    #[test]
    fn classification() {
        assert_eq!(classify("https://example.com/a"), Kind::Url);
        assert_eq!(classify("fn main() {\n    let x = 1;\n}"), Kind::Code);
        assert_eq!(classify("Bonjour tout le monde"), Kind::Text);
        assert_eq!(classify("j'ai -> une flèche"), Kind::Text);
    }

    #[test]
    fn apercu_sur_une_ligne() {
        assert_eq!(one_line("  a\n\n\tb  "), "a b");
    }
}
