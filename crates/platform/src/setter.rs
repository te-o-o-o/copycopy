use copycopy_core::{fnv1a, fnv1a_from, hash_event, ClipEvent, Payload};

pub struct Setter {
    inner: Option<arboard::Clipboard>,
    /// Identity of what we last wrote, so the watcher does not capture it back
    /// as if another application had copied it. For an image this is a hash of
    /// the **pixels**, not of the PNG bytes — see `echoes`.
    last_written: Option<u64>,
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
            Payload::Text(t) => {
                clip.set_text(t.clone()).map_err(|e| e.to_string())?;
                self.last_written = Some(hash_event(&ClipEvent::Text(t.clone())));
            }
            Payload::Files(paths) => {
                let joined = paths
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join("\n");
                clip.set_text(joined.clone()).map_err(|e| e.to_string())?;
                // Written as text, so text is what will come back.
                self.last_written = Some(hash_event(&ClipEvent::Text(joined)));
            }
            Payload::Image { data, .. } => {
                let bytes = data.load()?;
                let decoded = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
                let rgba = decoded.to_rgba8();
                let (w, h) = rgba.dimensions();
                self.last_written = Some(pixels_id(w, h, rgba.as_raw()));
                clip.set_image(arboard::ImageData {
                    width: w as usize,
                    height: h as usize,
                    bytes: rgba.into_raw().into(),
                })
                .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    /// True when `event` is what we just put on the clipboard ourselves.
    ///
    /// Content hashing is not enough for an image: `arboard` can only write
    /// RGBA, so the picture leaves as a **different PNG** than the one that
    /// came in — measured at 3.7 Kio in, 412 o out on the same 64×48 image.
    /// Different bytes, different hash, and every paste-back used to land in
    /// the history as a brand new entry.
    ///
    /// The identity is kept until the next write rather than expiring on a
    /// timer. Copying the very same picture again from another application
    /// then only fails to bump the entry back to the top, whereas an expiry
    /// shorter than a slow compositor's notification would let the duplicate
    /// through — which is the whole point of this.
    pub fn echoes(&self, event: &ClipEvent) -> bool {
        let Some(written) = self.last_written else {
            return false;
        };
        match event {
            ClipEvent::Image { png, .. } => match image::load_from_memory(png) {
                Ok(decoded) => {
                    let rgba = decoded.to_rgba8();
                    let (w, h) = rgba.dimensions();
                    pixels_id(w, h, rgba.as_raw()) == written
                }
                Err(_) => false,
            },
            other => hash_event(other) == written,
        }
    }
}

/// Identity of a picture, independent of how it happens to be encoded.
fn pixels_id(width: u32, height: u32, rgba: &[u8]) -> u64 {
    let id = fnv1a(&width.to_le_bytes());
    let id = fnv1a_from(id, &height.to_le_bytes());
    fnv1a_from(id, rgba)
}

impl Default for Setter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two encodings of one picture: an RGB PNG, as a screenshot tool
    /// produces, and the RGBA PNG that comes back once `arboard` has written
    /// it. Same pixels once decoded, different bytes on the wire.
    fn two_encodings() -> (image::RgbaImage, Vec<u8>, Vec<u8>) {
        let mut rgba = image::RgbaImage::new(4, 3);
        for (x, y, p) in rgba.enumerate_pixels_mut() {
            *p = image::Rgba([x as u8 * 40, y as u8 * 60, 90, 255]);
        }
        let encode = |img: image::DynamicImage| {
            let mut out = Vec::new();
            img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
                .expect("encode");
            out
        };
        let as_rgba = encode(image::DynamicImage::ImageRgba8(rgba.clone()));
        let as_rgb = encode(image::DynamicImage::ImageRgb8(
            image::DynamicImage::ImageRgba8(rgba.clone()).to_rgb8(),
        ));
        (rgba, as_rgba, as_rgb)
    }

    #[test]
    fn a_re_encoded_picture_is_recognised_as_our_own_echo() {
        let (rgba, as_rgba, as_rgb) = two_encodings();
        assert_ne!(as_rgba, as_rgb, "two encodings, or the test proves nothing");

        let setter = Setter {
            inner: None,
            last_written: Some(pixels_id(4, 3, rgba.as_raw())),
        };
        for png in [as_rgba, as_rgb] {
            assert!(setter.echoes(&ClipEvent::Image {
                png,
                size: Some((4, 3))
            }));
        }
    }

    #[test]
    fn another_picture_is_not_an_echo() {
        let (rgba, _, _) = two_encodings();
        let setter = Setter {
            inner: None,
            last_written: Some(pixels_id(4, 3, rgba.as_raw())),
        };

        let mut other = image::RgbaImage::new(4, 3);
        other.fill(200);
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(other)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("encode");

        assert!(!setter.echoes(&ClipEvent::Image {
            png,
            size: Some((4, 3))
        }));
        assert!(!setter.echoes(&ClipEvent::Text("autre chose".into())));
    }

    #[test]
    fn nothing_written_yet_echoes_nothing() {
        let setter = Setter {
            inner: None,
            last_written: None,
        };
        assert!(!setter.echoes(&ClipEvent::Text("coucou".into())));
    }
}
