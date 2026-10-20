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
/// a restart. Picked in the settings; the header icon switches between the
/// dark and light halves of the same pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Standard, the default pair.
    Dark,
    Light,
    Minuit,
    Lavande,
    CliniqueDark,
    CliniqueLight,
}

impl Mode {
    pub fn toggled(self) -> Self {
        match self {
            Mode::Dark => Mode::Light,
            Mode::Light => Mode::Dark,
            Mode::Minuit => Mode::Lavande,
            Mode::Lavande => Mode::Minuit,
            Mode::CliniqueDark => Mode::CliniqueLight,
            Mode::CliniqueLight => Mode::CliniqueDark,
        }
    }

    pub fn palette(self) -> Palette {
        match self {
            Mode::Dark => DARK,
            Mode::Light => LIGHT,
            Mode::Minuit => MINUIT,
            Mode::Lavande => LAVANDE,
            Mode::CliniqueDark => CLINIQUE_DARK,
            Mode::CliniqueLight => CLINIQUE_LIGHT,
        }
    }

    /// The default pair holds the plain `dark` and `light`, so a configuration
    /// that never chose a named theme follows whatever the default is.
    pub fn name(self) -> &'static str {
        match self {
            Mode::Dark => "dark",
            Mode::Light => "light",
            Mode::Minuit => "minuit",
            Mode::Lavande => "lavande",
            Mode::CliniqueDark => "clinique-dark",
            Mode::CliniqueLight => "clinique-light",
        }
    }

    /// Anything else — including the purpledream, aalto and matrix themes
    /// older versions wrote — falls back to the default.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "dark" => Some(Mode::Dark),
            "light" => Some(Mode::Light),
            "minuit" => Some(Mode::Minuit),
            "lavande" => Some(Mode::Lavande),
            "clinique-dark" => Some(Mode::CliniqueDark),
            "clinique-light" => Some(Mode::CliniqueLight),
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
    /// The pin marker, and the bar on a pinned row's left edge. Drawn rather
    /// than taken from a font: a colour emoji such as 📌 carries its own palette
    /// and cannot be tinted.
    pub pin: Color,
    /// Flash shown on the row that was just copied. Green rather than the
    /// accent: the accent already marks the selection, and a confirmation has
    /// to read as different, not as more.
    pub copied: Color,
    /// Ground of the frame around a preview, one step off the card.
    pub frame: Color,
}

/// Lilac at a given opacity: the dark theme's lines are drawn this way, so
/// they stay the same weight over the card, a selection or the code frame, and
/// keep the indigo's cast instead of greying it.
const fn veil(a: f32) -> Color {
    Color {
        r: 200.0 / 255.0,
        g: 190.0 / 255.0,
        b: 1.0,
        a,
    }
}

/// White at a given opacity, for Standard's and Clinique's lines: the same
/// idea as `veil`, on a ground that has no cast to keep.
const fn white(a: f32) -> Color {
    Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a,
    }
}

/// "Standard", dark: plain greys and one muted blue-green accent, so what the
/// search matched and the selection still stand out; the syntax keeps only a
/// trace of colour.
pub const DARK: Palette = Palette {
    light: false,
    card: rgb(0x1A, 0x1A, 0x1A),
    border: white(0.07),
    outline: white(0.11),
    selected: rgb(0x33, 0x33, 0x33),
    hover: rgb(0x24, 0x24, 0x24),
    text: rgb(0xCC, 0xCC, 0xCC),
    bright: rgb(0xFF, 0xFF, 0xFF),
    faint: rgb(0x85, 0x85, 0x85),
    chrome: rgb(0x66, 0x66, 0x66),
    key: rgb(0xA0, 0xA0, 0xA0),
    accent: rgb(0x7D, 0xB3, 0xB8),
    tint_text: rgb(0x7D, 0xB3, 0xB8),
    tint_url: rgb(0x9F, 0xB4, 0xC8),
    tint_code: rgb(0xC8, 0xB8, 0x9A),
    tint_image: rgb(0xB5, 0xA8, 0xC8),
    tint_files: rgb(0xA3, 0xBF, 0xA9),
    // A muted amber, the theme's only warm tone: a pin is found at a glance
    // without the list taking on colour.
    pin: rgb(0xC9, 0xA8, 0x6A),
    copied: rgb(0xA3, 0xBF, 0xA9),
    frame: rgb(0x20, 0x20, 0x20),
};

/// "Standard", light: the same on an off-white, the accent darkened to hold
/// contrast on it.
pub const LIGHT: Palette = Palette {
    light: true,
    card: rgb(0xFA, 0xFA, 0xFA),
    border: rgb(0xE3, 0xE3, 0xE3),
    outline: rgb(0xD6, 0xD6, 0xD6),
    selected: rgb(0xE2, 0xE2, 0xE2),
    hover: rgb(0xF1, 0xF1, 0xF1),
    text: rgb(0x2A, 0x2A, 0x2A),
    bright: rgb(0x00, 0x00, 0x00),
    faint: rgb(0x6B, 0x6B, 0x6B),
    chrome: rgb(0x99, 0x99, 0x99),
    key: rgb(0x4F, 0x4F, 0x4F),
    accent: rgb(0x2F, 0x7A, 0x80),
    tint_text: rgb(0x2F, 0x7A, 0x80),
    tint_url: rgb(0x4A, 0x66, 0x80),
    tint_code: rgb(0x7A, 0x64, 0x40),
    tint_image: rgb(0x64, 0x5A, 0x80),
    tint_files: rgb(0x4F, 0x6E, 0x57),
    pin: rgb(0xA0, 0x7A, 0x2C),
    copied: rgb(0x4F, 0x6E, 0x57),
    frame: rgb(0xF2, 0xF2, 0xF2),
};

/// "Minuit", dark: deep indigo, a violet selection, one coral accent.
pub const MINUIT: Palette = Palette {
    light: false,
    card: rgb(0x16, 0x13, 0x31),
    border: veil(0.10),
    outline: veil(0.16),
    selected: rgb(0x27, 0x21, 0x57),
    hover: rgb(0x1D, 0x19, 0x42),
    text: rgb(0xCF, 0xCA, 0xF0),
    bright: rgb(0xF6, 0xF4, 0xFF),
    faint: rgb(0x87, 0x80, 0xBE),
    chrome: rgb(0x68, 0x61, 0x9E),
    key: rgb(0xAC, 0xA6, 0xDC),
    accent: rgb(0xFF, 0x7A, 0x66),
    tint_text: rgb(0xFF, 0x7A, 0x66),
    tint_url: rgb(0x6F, 0xD3, 0xFF),
    tint_code: rgb(0xFF, 0xD1, 0x66),
    tint_image: rgb(0xC9, 0xA2, 0xFF),
    tint_files: rgb(0x7E, 0xE0, 0xC3),
    pin: rgb(0xFF, 0xD1, 0x66),
    copied: rgb(0x7E, 0xE0, 0xC3),
    frame: rgb(0x1C, 0x18, 0x40),
};

/// "Lavande", light: Minuit by day, built from the mark's own colours. A white
/// with a trace of lilac, text in the mark's indigo, a pale lilac selection,
/// and the coral darkened a step so it still reads on white.
pub const LAVANDE: Palette = Palette {
    light: true,
    card: rgb(0xFB, 0xFA, 0xFF),
    border: rgb(0xE7, 0xE3, 0xF5),
    outline: rgb(0xD9, 0xD3, 0xEE),
    selected: rgb(0xEE, 0xE8, 0xFF),
    hover: rgb(0xF5, 0xF2, 0xFF),
    text: rgb(0x2A, 0x25, 0x50),
    bright: rgb(0x16, 0x13, 0x31),
    faint: rgb(0x6F, 0x6A, 0x96),
    chrome: rgb(0x95, 0x90, 0xB5),
    key: rgb(0x4E, 0x48, 0x78),
    accent: rgb(0xE2, 0x51, 0x3C),
    tint_text: rgb(0xE2, 0x51, 0x3C),
    tint_url: rgb(0x2F, 0x6F, 0xD1),
    tint_code: rgb(0xB4, 0x59, 0x0E),
    tint_image: rgb(0x7B, 0x4F, 0xD6),
    tint_files: rgb(0x1E, 0x8A, 0x6E),
    pin: rgb(0xE2, 0x51, 0x3C),
    copied: rgb(0x1E, 0x8A, 0x6E),
    frame: rgb(0xF4, 0xF1, 0xFD),
};

/// "Clinique", dark: blue-grey graphite, one teal accent. Values are the
/// redesign mockup's (variant G), unchanged. Kept as a choice in the settings.
pub const CLINIQUE_DARK: Palette = Palette {
    light: false,
    card: rgb(0x0F, 0x11, 0x15),
    border: white(0.07),
    outline: white(0.10),
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
pub const CLINIQUE_LIGHT: Palette = Palette {
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
/// The coloured square standing in for that icon when there is none. Smaller
/// than the icon: a flat square reads heavier than a drawing of the same size.
pub const SOURCE_SWATCH: f32 = 10.0;
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

/// The colour of the square beside the source's name, when the platform gave
/// no icon: one per application, the same from one run, and one release, to
/// the next. Chosen from the name with FNV-1a rather than `DefaultHasher`,
/// whose output Rust does not promise to keep. Mid-tones, so every one reads on
/// both the indigo and the white card.
pub fn app_colour(source: &str) -> Color {
    const COLOURS: [Color; 8] = [
        rgb(0x3B, 0x82, 0xF6),
        rgb(0xE0, 0x54, 0x8E),
        rgb(0xF5, 0x9E, 0x0B),
        rgb(0x10, 0xB9, 0x81),
        rgb(0x8B, 0x5C, 0xF6),
        rgb(0xEF, 0x6C, 0x35),
        rgb(0x06, 0xB6, 0xD4),
        rgb(0x84, 0xCC, 0x16),
    ];
    let hash = source.bytes().fold(0x811C_9DC5_u32, |h, b| {
        (h ^ u32::from(b.to_ascii_lowercase())).wrapping_mul(0x0100_0193)
    });
    COLOURS[hash as usize % COLOURS.len()]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_application_keeps_its_colour() {
        // Case does not matter, and two common sources do not collide.
        assert_eq!(app_colour("Firefox"), app_colour("firefox"));
        assert_ne!(app_colour("Firefox"), app_colour("Terminal"));
    }
}
