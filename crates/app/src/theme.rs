//! Palette. Tout le look du popup se règle ici.

use iced::{Background, Border, Color, Theme};

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
}

pub const CARD: Color = rgb(0x15, 0x17, 0x1D);
pub const BORDER: Color = rgb(0x2A, 0x2D, 0x36);
pub const SELECTED: Color = rgb(0x24, 0x27, 0x31);
pub const HOVER: Color = rgb(0x1B, 0x1E, 0x25);
pub const TEXT: Color = rgb(0xE7, 0xE9, 0xEF);
pub const DIM: Color = rgb(0x9A, 0xA0, 0xB0);
pub const FAINT: Color = rgb(0x5E, 0x64, 0x75);
pub const ACCENT: Color = rgb(0x7C, 0x8C, 0xFF);

pub const TINT_TEXT: Color = rgb(0x7C, 0x8C, 0xFF);
pub const TINT_URL: Color = rgb(0x4F, 0xC3, 0xA1);
pub const TINT_CODE: Color = rgb(0xE0, 0xA4, 0x58);
pub const TINT_IMAGE: Color = rgb(0xC9, 0x8B, 0xDB);
pub const TINT_FILES: Color = rgb(0x6F, 0xA8, 0xDC);

pub const ROW_H: f32 = 58.0;
pub const HEADER_H: f32 = 58.0;
pub const FOOTER_H: f32 = 30.0;
/// Fenêtre à angles droits : des coins arrondis sur une fenêtre sans
/// décorations laissent voir le fond du bureau dans les angles, ce qui ressort
/// comme un liseré noir au lieu d'un arrondi propre.
pub const CARD_RADIUS: f32 = 0.0;
pub const ROW_RADIUS: f32 = 8.0;
/// Respiration verticale de la surbrillance à l'intérieur de sa rangée. La
/// rangée garde sa hauteur exacte (la virtualisation en dépend) ; seul le fond
/// est rétréci.
pub const ROW_GAP: f32 = 3.0;
/// Bande transparente autour de la carte : elle porte les poignées de
/// redimensionnement, et laisse voir les coins arrondis.
pub const EDGE: f32 = 6.0;

pub fn tint(kind: copycopy_core::Kind) -> Color {
    use copycopy_core::Kind;
    match kind {
        Kind::Text => TINT_TEXT,
        Kind::Url => TINT_URL,
        Kind::Code => TINT_CODE,
        Kind::Image => TINT_IMAGE,
        Kind::Files => TINT_FILES,
    }
}

pub fn badge(kind: copycopy_core::Kind) -> &'static str {
    use copycopy_core::Kind;
    match kind {
        Kind::Text => "TXT",
        Kind::Url => "URL",
        Kind::Code => "{ }",
        Kind::Image => "IMG",
        Kind::Files => "DIR",
    }
}

pub fn alpha(color: Color, a: f32) -> Color {
    Color { a, ..color }
}

/// Le fond de la carte, coins arrondis et liseré compris.
pub fn card(_theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(CARD)),
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: CARD_RADIUS.into(),
        },
        ..Default::default()
    }
}

/// Fond de l'application : opaque et de la couleur de la carte, pour qu'aucune
/// zone transparente ne subsiste sur les bords.
pub fn root() -> iced::theme::Style {
    iced::theme::Style {
        background_color: CARD,
        text_color: TEXT,
    }
}

pub fn separator(_theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(BORDER)),
        ..Default::default()
    }
}
