use core::error::Error;
use core::fmt;

use mcrs_minecraft_font::{FontError, LanguageError};

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum GuiAtlasError {
    Font(FontError),
    Textures(String),
    Translations(LanguageError),
}

impl fmt::Display for GuiAtlasError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Font(error) => write!(formatter, "the font: {error}"),
            Self::Textures(reason) => write!(formatter, "the GUI textures: {reason}"),
            Self::Translations(error) => write!(formatter, "the translations: {error}"),
        }
    }
}

impl Error for GuiAtlasError {}
