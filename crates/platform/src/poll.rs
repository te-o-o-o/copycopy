//! Universal fallback: poll the clipboard and compare hashes. Inelegant, but
//! it is exactly what macOS does anyway, since `NSPasteboard` has no change
//! event.

use std::sync::mpsc::Sender;
use std::time::Duration;

use copycopy_core::{hash_event, ClipEvent};

use crate::Capture;

const INTERVAL: Duration = Duration::from_millis(200);

pub fn spawn(tx: Sender<Capture>) -> Result<(), String> {
    // Access is validated here so we fail immediately rather than silently
    // inside the thread.
    arboard::Clipboard::new().map_err(|e| e.to_string())?;

    std::thread::Builder::new()
        .name("copycopy-poll".into())
        .spawn(move || run(tx))
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn run(tx: Sender<Capture>) {
    let mut clip = match arboard::Clipboard::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("sondage arrêté : {e}");
            return;
        }
    };
    let mut last: Option<u64> = None;

    loop {
        std::thread::sleep(INTERVAL);

        let event = clip
            .get_text()
            .ok()
            .filter(|t| !t.trim().is_empty())
            .map(ClipEvent::Text)
            .or_else(|| {
                let img = clip.get_image().ok()?;
                let (w, h) = (img.width as u32, img.height as u32);
                let buf: image::RgbaImage =
                    image::ImageBuffer::from_raw(w, h, img.bytes.into_owned())?;
                let mut png = Vec::new();
                image::DynamicImage::ImageRgba8(buf)
                    .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
                    .ok()?;
                Some(ClipEvent::Image {
                    png,
                    size: Some((w, h)),
                })
            });

        let Some(event) = event else { continue };
        let hash = hash_event(&event);
        if last == Some(hash) {
            continue;
        }
        last = Some(hash);

        if tx
            .send(Capture {
                event,
                // Polling cannot tell where the content came from.
                source: String::new(),
            })
            .is_err()
        {
            return; // L'UI est partie.
        }
    }
}
