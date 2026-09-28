use iced::{Background, Border, Color, Theme};

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
}

/// Which palette is active. Lives in the configuration, so the choice survives
/// a restart, and is switched from the header icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Dark,
    Light,
}

impl Mode {
    pub fn toggled(self) -> Self {
        match self {
            Mode::Dark => Mode::Light,
            Mode::Light => Mode::Dark,
        }
    }

    pub fn palette(self) -> Palette {
        match self {
            Mode::Dark => DARK,
            Mode::Light => LIGHT,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Mode::Dark => "dark",
            Mode::Light => "light",
        }
    }

    /// Anything else — including the purpledream, aalto and matrix themes
    /// older versions wrote — falls back to the default.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "dark" => Some(Mode::Dark),
            "light" => Some(Mode::Light),
            _ => None,
        }
    }
}

/// Every colour the interface draws with.
///
/// A value rather than constants: constants cannot change while the program
/// runs, and switching theme has to be immediate. It is `Copy`, so style
/// closures capture it without borrowing anything.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// Tells icons that change shape with the theme which one to draw.
    pub light: bool,
    pub card: Color,
    /// Separators and the window rim.
    pub border: Color,
    /// Outline of the shortcut chips: a step stronger than `border`, which
    /// would vanish around something that small.
    pub outline: Color,
    pub selected: Color,
    pub hover: Color,
    /// Body text: previews, the code frame, unselected titles.
    pub text: Color,
    /// The selected row's title and what is typed in the search.
    pub bright: Color,
    /// Placeholder, icons, inactive filter tabs.
    pub faint: Color,
    /// The window's small print: the count, badges, meta lines, captions. A
    /// step below `faint`, so it recedes behind what can be clicked.
    pub chrome: Color,
    /// Text of the outlined shortcut chips.
    pub key: Color,
    /// The one colour: selection bar, active tab, primary chip, keywords.
    pub accent: Color,
    /// Syntax colours only — `ink` in the view. The list itself stays grey.
    pub tint_text: Color,
    pub tint_url: Color,
    pub tint_code: Color,
    pub tint_image: Color,
    pub tint_files: Color,
    /// The pin marker. Drawn rather than taken from a font: a colour emoji such
    /// as 📌 carries its own palette and cannot be tinted.
    pub pin: Color,
    /// Flash shown on the row that was just copied. Green rather than the
    /// accent: the accent already marks the selection, and a confirmation has
    /// to read as different, not as more.
    pub copied: Color,
    /// Ground of the frame around a preview, one step off the card.
    pub frame: Color,
}

/// White at a given opacity: the dark theme's lines are drawn this way, so
/// they stay the same weight over the card, a selection or the code frame.
const fn veil(a: f32) -> Color {
    Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a,
    }
}

/// "Clinique", dark: blue-grey graphite, one teal accent. Values are the
/// redesign mockup's (variant G), unchanged.
pub const DARK: Palette = Palette {
    light: false,
    card: rgb(0x0F, 0x11, 0x15),
    border: veil(0.07),
    outline: veil(0.10),
    selected: rgb(0x1A, 0x1E, 0x28),
    // Not in the mockup: halfway between the card and the selection.
    hover: rgb(0x15, 0x18, 0x1F),
    text: rgb(0xC9, 0xCF, 0xD9),
    bright: rgb(0xF1, 0xF4, 0xF8),
    faint: rgb(0x6B, 0x72, 0x80),
    chrome: rgb(0x56, 0x5D, 0x69),
    key: rgb(0x9A, 0xA2, 0xAE),
    accent: rgb(0x2D, 0xD4, 0xBF),
    tint_text: rgb(0x2D, 0xD4, 0xBF),
    tint_url: rgb(0x3D, 0xB8, 0x6A),
    tint_code: rgb(0xE0, 0xA5, 0x3E),
    tint_image: rgb(0xB0, 0x7C, 0xE0),
    tint_files: rgb(0x4F, 0xC1, 0xC6),
    pin: rgb(0xE0, 0xA5, 0x3E),
    copied: rgb(0x3D, 0xB8, 0x6A),
    frame: rgb(0x15, 0x18, 0x21),
};

/// "Clinique", light: the same layout on white, the teal darkened to hold
/// contrast on it.
pub const LIGHT: Palette = Palette {
    light: true,
    card: rgb(0xFF, 0xFF, 0xFF),
    border: rgb(0xE6, 0xE9, 0xEE),
    outline: rgb(0xDF, 0xE4, 0xEA),
    selected: rgb(0xEE, 0xF3, 0xF5),
    hover: rgb(0xF5, 0xF8, 0xF9),
    text: rgb(0x22, 0x26, 0x2E),
    bright: rgb(0x0A, 0x0C, 0x10),
    faint: rgb(0x73, 0x7A, 0x85),
    chrome: rgb(0x94, 0x9B, 0xA6),
    key: rgb(0x5B, 0x62, 0x6D),
    accent: rgb(0x0E, 0x8C, 0x7F),
    tint_text: rgb(0x0E, 0x8C, 0x7F),
    tint_url: rgb(0x2E, 0x9E, 0x5B),
    tint_code: rgb(0xB8, 0x86, 0x0B),
    tint_image: rgb(0x8A, 0x4F, 0xC2),
    tint_files: rgb(0x2E, 0x8F, 0x93),
    pin: rgb(0xB8, 0x86, 0x0B),
    copied: rgb(0x2E, 0x9E, 0x5B),
    frame: rgb(0xF7, 0xF9, 0xFB),
};

pub const ROW_H: f32 = 50.0;
pub const HEADER_H: f32 = 58.0;
/// The type filter band under the search.
pub const FILTERS_H: f32 = 36.0;
/// Rounded window. It only holds up on top of two other things: the window is
/// created `transparent`, and `root` clears the surface with a transparent
/// colour — otherwise iced paints the theme colour across the whole surface and
/// the curve ends up sitting on an opaque rectangle.
///
/// Twelve pixels, against `EDGE` at ten: the curve reaches 12 px in from each
/// side, the content starts at 10 px, and by that depth the curve has already
/// come back within 6 px. Nothing but the card itself is ever drawn in the
/// corner, which is what lets it round cleanly.
pub const CARD_RADIUS: f32 = 12.0;
pub const ROW_RADIUS: f32 = 6.0;
/// Vertical breathing room for the highlight inside its row. The row keeps its
/// exact height, which the virtualisation depends on; only the background is
/// shrunk.
pub const ROW_GAP: f32 = 3.0;
/// Band around the rim carrying the resize handles. It is invisible because it
/// lets the card background through. Ten pixels rather than six: an
/// undecorated window gives no other affordance, and a thin band is hard to
/// aim at.
pub const EDGE: f32 = 10.0;
/// The source application's icon on a row's meta line. Thirteen pixels, to sit
/// on an eleven-pixel line without pushing it taller — the row's height is what
/// the virtualisation counts on.
pub const SOURCE_ICON: f32 = 13.0;
/// Width of one slot in a row's gutters. A gutter is always laid out, whether
/// or not anything is drawn in it, so the preview always clips at the same x
/// instead of shifting when a row is hovered or pinned.
pub const SLOT: f32 = 16.0;
/// Breathing room between the preview and the gutter. Without it the text ends
/// flush against the icons, which reads as a collision even though it is not.
pub const GUTTER_GAP: f32 = 18.0;

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
pub fn card(p: Palette, radius: f32) -> impl Fn(&Theme) -> iced::widget::container::Style {
    move |_theme| iced::widget::container::Style {
        background: Some(Background::Color(p.card)),
        border: Border {
            color: p.border,
            width: 1.0,
            radius: radius.into(),
        },
        ..Default::default()
    }
}

/// What the surface is cleared with, under everything the card draws.
///
/// Rounded: transparent, so the desktop shows through outside the curve. The
/// card still paints `p.card` over every pixel inside it, so nothing of the
/// window itself turns see-through.
///
/// Square: the card colour. The clear colour then matches the card exactly and
/// no seam can show at the rim, which is what an opaque window wants.
pub fn root(p: Palette, rounded: bool) -> iced::theme::Style {
    iced::theme::Style {
        background_color: if rounded { Color::TRANSPARENT } else { p.card },
        text_color: p.text,
    }
}

pub fn separator(p: Palette) -> impl Fn(&Theme) -> iced::widget::container::Style {
    move |_theme| iced::widget::container::Style {
        background: Some(Background::Color(p.border)),
        ..Default::default()
    }
}
