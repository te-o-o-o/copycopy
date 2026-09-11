//! Écriture dans le presse-papier (le « coller » depuis l'historique).
//!
//! On s'appuie sur `arboard` : sur X11 il faut rester propriétaire de la
//! sélection tant que personne d'autre ne la prend, et arboard gère déjà ce
//! thread de service. Pas la peine de le réécrire.

use copycopy_core::Payload;

pub struct Setter {
    inner: Option<arboard::Clipboard>,
    /// Hash de ce qu'on vient d'écrire, pour que le watcher ne le recapture pas
    /// comme s'il venait d'une autre application.
    pub last_written: Option<u64>,
}

impl Setter {
    pub fn new() -> Self {
        let inner = match arboard::Clipboard::new() {
            Ok(c) => Some(c),
            Err(e) => {
                eprintln!("presse-papier inaccessible en écriture : {e}");
                None
            }
        };
        Self {
            inner,
            last_written: None,
        }
    }

    pub fn set(&mut self, payload: &Payload) -> Result<(), String> {
        let clip = self.inner.as_mut().ok_or("presse-papier indisponible")?;
        match payload {
            Payload::Text(t) => clip.set_text(t.clone()).map_err(|e| e.to_string()),
            Payload::Files(paths) => {
                let joined = paths
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join("\n");
                clip.set_text(joined).map_err(|e| e.to_string())
            }
            Payload::Image { png, .. } => {
                let decoded = image::load_from_memory(png).map_err(|e| e.to_string())?;
                let rgba = decoded.to_rgba8();
                let (w, h) = rgba.dimensions();
                clip.set_image(arboard::ImageData {
                    width: w as usize,
                    height: h as usize,
                    bytes: rgba.into_raw().into(),
                })
                .map_err(|e| e.to_string())
            }
        }
    }
}

impl Default for Setter {
    fn default() -> Self {
        Self::new()
    }
}
