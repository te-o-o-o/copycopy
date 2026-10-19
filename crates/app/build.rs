//! Puts the mark inside the Windows `.exe`, as its icon resource.
//!
//! Without one, Explorer, the Start menu and desktop shortcuts, the startup
//! apps list, and the taskbar wherever the window's own icon is not used all
//! show the generic executable. `icon::set_taskbar` only covers the open
//! window. The `.ico` is rendered here from the same drawing as everything else,
//! so no image file is checked in.

#[path = "src/mark.rs"]
#[allow(dead_code)]
mod mark;

use image::codecs::ico::{IcoEncoder, IcoFrame};
use image::ExtendedColorType;

fn main() {
    println!("cargo:rerun-if-changed=src/mark.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    // The sizes Windows picks from, small icons to the large Explorer view.
    let frames: Vec<IcoFrame> = [16, 24, 32, 48, 64, 256]
        .into_iter()
        .map(|size| {
            IcoFrame::as_png(&mark::rgba(size), size, size, ExtendedColorType::Rgba8)
                .expect("encode an icon frame")
        })
        .collect();
    let ico = out.join("copycopy.ico");
    let file = std::fs::File::create(&ico).expect("create the .ico");
    IcoEncoder::new(file)
        .encode_images(&frames)
        .expect("write the .ico");

    // A string in a resource script: backslashes in the path must be doubled.
    let rc = out.join("copycopy.rc");
    let path = ico.display().to_string().replace('\\', "\\\\");
    std::fs::write(&rc, format!("1 ICON \"{path}\"\n")).expect("write the .rc");
    embed_resource::compile(&rc, embed_resource::NONE)
        .manifest_optional()
        .expect("embed the icon");
}
