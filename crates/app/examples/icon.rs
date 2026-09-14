//! Writes the application mark to a PNG, at any size.
//!
//! The icon is drawn in code (`app/src/icon.rs`), so the only way to look at it
//! is to render it — and the installers will need exactly these files.
//!
//!   cargo run --release -p copycopy --example icon -- icon.png 256

#[path = "../src/icon.rs"]
// The module is shared with the application, which uses more of it than this
// example does.
#[allow(dead_code)]
mod icon;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap_or_else(|| "icon.png".to_string());
    let size: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(256);

    let pixels = icon::rgba(size);
    let Some(image) = image::RgbaImage::from_raw(size, size, pixels) else {
        eprintln!("taille invalide : {size}");
        return;
    };
    match image.save(&path) {
        Ok(()) => println!("écrit : {path} ({size}×{size})"),
        Err(e) => eprintln!("échec de l'écriture : {e}"),
    }
}
