//! The interface language.
//!
//! Two languages do not need a catalogue: each call site holds its French and
//! its English side by side, so a sentence and its translation are read, and
//! changed, together.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    Fr,
    En,
}

impl Locale {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "fr" => Some(Self::Fr),
            "en" => Some(Self::En),
            _ => None,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::Fr => "fr",
            Self::En => "en",
        }
    }

    /// This language's side of a pair.
    pub fn tr<'a>(self, fr: &'a str, en: &'a str) -> &'a str {
        match self {
            Self::Fr => fr,
            Self::En => en,
        }
    }
}
