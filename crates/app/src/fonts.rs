//! Polices.
//!
//! Contrairement à egui, cosmic-text découvre les polices système tout seul
//! (via fontdb) : il n'y a plus de chaîne de fallback à écrire à la main. Ce
//! module ne sert plus qu'aux systèmes dépourvus de polices CJK ou emoji —
//! typiquement un WSL minimal, où l'on va chercher celles de Windows.

/// Chemins tentés en plus de ce que le système expose déjà. Aucun n'est requis.
const EXTRA: &[&str] = &[
    // WSL : les polices Windows sont montées et couvrent tout.
    "/mnt/c/Windows/Fonts/segoeui.ttf",
    "/mnt/c/Windows/Fonts/seguiemj.ttf",
    "/mnt/c/Windows/Fonts/YuGothR.ttc",
    "/mnt/c/Windows/Fonts/msyh.ttc",
    "/mnt/c/Windows/Fonts/malgun.ttf",
    // Linux, si présentes.
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
