use core::error::Error;
use core::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum GuiAtlasError {
    Font(String),
    Textures(String),
    Translations(String),
}

impl fmt::Display for GuiAtlasError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Font(reason) => write!(formatter, "the font: {reason}"),
            Self::Textures(reason) => write!(formatter, "the GUI textures: {reason}"),
            Self::Translations(reason) => write!(formatter, "the translations: {reason}"),
        }
    }
}

impl Error for GuiAtlasError {}
