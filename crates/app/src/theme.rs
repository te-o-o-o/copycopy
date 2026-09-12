//! Palette. The whole look of the popup is tuned here.

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
/// Square window: on an undecorated window, rounded corners reveal the desktop
/// behind them, which reads as a black outline rather than a clean curve.
pub const CARD_RADIUS: f32 = 0.0;
pub const ROW_RADIUS: f32 = 8.0;
/// Vertical breathing room for the highlight inside its row. The row keeps its
/// exact height, which the virtualisation depends on; only the background is
/// shrunk.
pub const ROW_GAP: f32 = 3.0;
/// Band around the rim carrying the resize handles. It is invisible because it
/// lets the card background through. Ten pixels rather than six: an
/// undecorated window gives no other affordance, and a thin band is hard to
/// aim at.
pub const EDGE: f32 = 10.0;

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

/// The card background, border included.
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

/// Application background: opaque and card-coloured, so no transparent area
/// remains at the edges.
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
