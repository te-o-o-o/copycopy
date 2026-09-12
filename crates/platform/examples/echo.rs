//! Checks that what we write to the clipboard is not captured back as if
//! another application had copied it.
//!
//! It runs the real watcher and the real `Setter` in one process, as the
//! application does, then reports each capture and whether `Setter::echoes`
//! recognised it. Written because the case cannot be reached from the window
//! under WSLg: the surface is Wayland, so XTEST cannot deliver the Enter that
//! triggers a copy.
//!
//!   cargo run --release -p copycopy-platform --example echo -- shot.png
//!   cargo run --release -p copycopy-platform --example echo -- "some text"

use std::time::{Duration, Instant};

use copycopy_core::{Image, Payload};

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_else(|| "coucou".into());
    let secs: u64 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(6);
    let backend = std::env::args()
        .position(|a| a == "--backend")
        .and_then(|i| std::env::args().nth(i + 1));

    let path = std::path::PathBuf::from(&arg);
    let payload = if path.is_file() {
        Payload::Image {
            data: Image::File(path),
            size: None,
        }
    } else {
        Payload::Text(arg)
    };

    let watcher = match copycopy_platform::start(backend.as_deref()) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("capture indisponible : {e}");
            return;
        }
    };
    println!("backend : {}", watcher.kind.label());

    let mut setter = copycopy_platform::Setter::new();
    // Let the backend settle before writing, or the write lands before the
    // watcher is listening and there is nothing to recognise.
    std::thread::sleep(Duration::from_millis(500));
    match setter.set(&payload) {
        Ok(()) => println!("écrit dans le presse-papier, écoute {secs} s\n"),
        Err(e) => {
            eprintln!("échec de l'écriture : {e}");
            return;
        }
    }

    let deadline = Instant::now() + Duration::from_secs(secs);
    let mut seen = 0;
    let mut missed = 0;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        let Ok(capture) = watcher.rx.recv_timeout(left) else {
            break;
        };
        seen += 1;
        let echo = setter.echoes(&capture.event);
        if !echo {
            missed += 1;
        }
        println!(
            "{} {:?}",
            if echo { "[ÉCHO ]" } else { "[NOUVEL]" },
            summarise(&capture.event)
        );
    }

    println!("\n{seen} capture(s), dont {missed} non reconnue(s) comme la nôtre");
    if seen == 0 {
        println!("aucune capture : le backend n'a pas vu notre propre écriture");
    }
}

fn summarise(event: &copycopy_core::ClipEvent) -> String {
    match event {
        copycopy_core::ClipEvent::Text(t) => {
            format!("texte {} caractères", t.chars().count())
        }
        copycopy_core::ClipEvent::Image { png, size } => {
            format!("image {size:?}, {} octets PNG", png.len())
        }
        copycopy_core::ClipEvent::Files(paths) => format!("{} fichier(s)", paths.len()),
    }
}
