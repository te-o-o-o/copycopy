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
    /// Green on black. Reached from the footer rather than the header icon,
    /// which keeps alternating between the two everyday themes.
    Matrix,
}

impl Mode {
    pub fn toggled(self) -> Self {
        match self {
            Mode::Dark => Mode::Light,
            Mode::Light | Mode::Matrix => Mode::Dark,
        }
    }

    /// In and out of Matrix. Leaving it lands on the dark theme, whichever one
    /// was active before: the closest to where Matrix left you.
    pub fn matrix_toggled(self) -> Self {
        match self {
            Mode::Matrix => Mode::Dark,
            Mode::Dark | Mode::Light => Mode::Matrix,
        }
    }

    pub fn palette(self) -> Palette {
        match self {
            Mode::Dark => DARK,
            Mode::Light => LIGHT,
            Mode::Matrix => MATRIX,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Mode::Dark => "dark",
            Mode::Light => "light",
            Mode::Matrix => "matrix",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "dark" => Some(Mode::Dark),
            "light" => Some(Mode::Light),
            "matrix" => Some(Mode::Matrix),
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
    /// Tells the footer switch which way it goes.
    pub matrix: bool,
    pub card: Color,
    pub border: Color,
    pub selected: Color,
    pub hover: Color,
    pub text: Color,
    pub faint: Color,
    /// The window's own small print: the entry count in the footer and the
    /// shortcut legend in the header. Its own role so a theme can colour it
    /// without touching the placeholder and icons that share `faint`.
    pub chrome: Color,
    pub accent: Color,
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
    /// Ground of the Carbon-style window around a text preview. A step away
    /// from the card, so the window reads as an object set on the panel.
    pub frame: Color,
    /// Its drop shadow. Near-black on the dark theme, a warm brown on the light
    /// one: a grey shadow on cream reads as dirt, not depth.
    pub shadow: Color,
}

/// Base16 Purpledream, by malet — values from the tinted-theming scheme, mapped
/// onto the interface roles. The grounds walk the scheme's purple-black ramp
/// (base00 card, base01 selection). Three values are invented: the hover, which
/// sits between base00 and base01 because the scheme has no step there, and a
/// pale pink for the window's lines and small print, chosen to answer the
/// magenta accent. The lines take a deeper pink than the text: a one-pixel line
/// in the text's pink reads as white. The badge tints use the scheme's accent
/// colours untouched.
pub const DARK: Palette = Palette {
    light: false,
    matrix: false,
    card: rgb(0x10, 0x05, 0x10),     // base00
    border: rgb(0xC9, 0x8F, 0xB4),
    selected: rgb(0x30, 0x20, 0x30), // base01
    hover: rgb(0x1E, 0x10, 0x1E),
    text: rgb(0xDD, 0xD0, 0xDD),       // base05
    faint: rgb(0x60, 0x50, 0x60),      // base03
    chrome: rgb(0xE8, 0xB4, 0xD0),
    accent: rgb(0xF0, 0x00, 0xA0),     // base0A
    tint_text: rgb(0x00, 0xA0, 0xF0),  // base0D
    tint_url: rgb(0x14, 0xCC, 0x64),   // base0B
    tint_code: rgb(0xCC, 0xAE, 0x14),  // base09
    tint_image: rgb(0xB0, 0x00, 0xD0), // base0E
    tint_files: rgb(0x00, 0x75, 0xB0), // base0C
    pin: rgb(0x14, 0xCC, 0x64),        // base0B
    copied: rgb(0x14, 0xCC, 0x64),     // base0B
    frame: rgb(0x30, 0x20, 0x30),      // base01
    shadow: Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.55,
    },
};

/// Aalto Light, warmed and dimmed: its cream ground (#FFFFE0) is nearly as
/// bright as white, so it is taken a few steps down while keeping the hue.
/// Text is the theme's dark slate grey; the badge tints come from its
/// font-lock colours — forest green, dark goldenrod, purple, cadet blue.
pub const LIGHT: Palette = Palette {
    light: true,
    matrix: false,
    card: rgb(0xF0, 0xEB, 0xCF),
    border: rgb(0xD8, 0xD1, 0xB0),
    selected: rgb(0xE0, 0xD8, 0xB4),
    hover: rgb(0xE8, 0xE2, 0xC4),
    text: rgb(0x2F, 0x4F, 0x4F),
    faint: rgb(0x8A, 0x84, 0x68),
    chrome: rgb(0x8A, 0x84, 0x68),
    accent: rgb(0x8A, 0x2B, 0xC2),
    tint_text: rgb(0x3B, 0x5B, 0xA8),
    tint_url: rgb(0x22, 0x8B, 0x22),
    tint_code: rgb(0xB8, 0x86, 0x0B),
    tint_image: rgb(0x8A, 0x2B, 0xC2),
    tint_files: rgb(0x5F, 0x9E, 0xA0),
    pin: rgb(0x22, 0x8B, 0x22),
    copied: rgb(0x22, 0x8B, 0x22),
    frame: rgb(0xF9, 0xF5, 0xE3),
    shadow: Color {
        r: 0.35,
        g: 0.27,
        b: 0.08,
        a: 0.22,
    },
};

/// Green on black, after the film's falling code. Borrowed from no scheme: the
/// green is the #00FF41 usually quoted for it, and every other colour is that
/// same green at a lower intensity, so the whole window stays a single hue.
pub const MATRIX: Palette = Palette {
    light: false,
    matrix: true,
    card: rgb(0x00, 0x05, 0x00),
    border: rgb(0x0B, 0x3D, 0x16),
    selected: rgb(0x06, 0x2B, 0x10),
    hover: rgb(0x03, 0x16, 0x08),
    text: rgb(0x00, 0xFF, 0x41),
    faint: rgb(0x0E, 0x7A, 0x2C),
    chrome: rgb(0x0E, 0x7A, 0x2C),
    accent: rgb(0x00, 0xFF, 0x41),
    tint_text: rgb(0x00, 0xFF, 0x41),
    tint_url: rgb(0x5B, 0xFF, 0x8A),
    tint_code: rgb(0x9C, 0xFF, 0x57),
    tint_image: rgb(0x00, 0xC8, 0x3A),
    tint_files: rgb(0x3F, 0xD4, 0x6B),
    pin: rgb(0x00, 0xFF, 0x41),
    copied: rgb(0x00, 0xFF, 0x41),
    frame: rgb(0x01, 0x10, 0x04),
    // A green glow rather than a shadow: on black, darkness casts nothing.
    shadow: Color {
        r: 0.0,
        g: 1.0,
        b: 0.25,
        a: 0.18,
    },
};

impl Palette {
    pub fn tint(&self, kind: copycopy_core::Kind) -> Color {
        use copycopy_core::Kind;
        match kind {
            Kind::Text => self.tint_text,
            Kind::Url => self.tint_url,
            Kind::Code => self.tint_code,
            Kind::Image => self.tint_image,
            Kind::Files => self.tint_files,
        }
    }
}

pub const ROW_H: f32 = 50.0;
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
pub fn card(p: Palette) -> impl Fn(&Theme) -> iced::widget::container::Style {
    move |_theme| iced::widget::container::Style {
        background: Some(Background::Color(p.card)),
        border: Border {
            color: p.border,
            width: 1.0,
            radius: CARD_RADIUS.into(),
        },
        ..Default::default()
    }
}

/// Application background: opaque and card-coloured, so no transparent area
/// remains at the edges.
pub fn root(p: Palette) -> iced::theme::Style {
    iced::theme::Style {
        background_color: p.card,
        text_color: p.text,
    }
}

pub fn separator(p: Palette) -> impl Fn(&Theme) -> iced::widget::container::Style {
    move |_theme| iced::widget::container::Style {
        background: Some(Background::Color(p.border)),
        ..Default::default()
    }
}
