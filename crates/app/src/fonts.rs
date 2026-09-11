//! Fonts.
//!
//! Unlike egui, cosmic-text discovers system fonts on its own through fontdb,
//! so there is no fallback chain left to write by hand. This module only serves
//! systems that lack CJK or emoji fonts — typically a minimal WSL, where we go
//! and fetch the Windows ones.

/// Paths tried on top of what the system already exposes. None is required.
const EXTRA: &[&str] = &[
    // WSL: the Windows fonts are mounted and cover everything.
    "/mnt/c/Windows/Fonts/segoeui.ttf",
    "/mnt/c/Windows/Fonts/seguiemj.ttf",
    "/mnt/c/Windows/Fonts/YuGothR.ttc",
    "/mnt/c/Windows/Fonts/msyh.ttc",
    "/mnt/c/Windows/Fonts/malgun.ttf",
    // Linux, when present.
    "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
];

pub struct Loaded {
    pub bytes: Vec<Vec<u8>>,
    pub paths: Vec<&'static str>,
}

pub fn extra() -> Loaded {
    let mut bytes = Vec::new();
    let mut paths = Vec::new();
    for path in EXTRA {
        if let Ok(data) = std::fs::read(path) {
            bytes.push(data);
            paths.push(*path);
        }
    }
    Loaded { bytes, paths }
}
