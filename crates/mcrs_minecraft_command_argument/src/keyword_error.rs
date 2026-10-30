use core::fmt;
use mcrs_command::Span;
use mcrs_minecraft_command::{ErrorText, span_text};
use mcrs_minecraft_text::Text;

/// The errors of the arguments that read one word of a fixed set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeywordError {
    /// The word at the span names no game mode.
    GameModeInvalid(Span),
    /// The word at the span names no entity anchor.
    AnchorInvalid(Span),
    /// The word at the span names no value of the set.
    EnumInvalid(Span),
}

impl fmt::Display for KeywordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            KeywordError::GameModeInvalid(_) => "unknown game mode",
            KeywordError::AnchorInvalid(_) => "unknown entity anchor",
            KeywordError::EnumInvalid(_) => "unknown value",
        })
    }
}

impl core::error::Error for KeywordError {}

impl ErrorText for KeywordError {
    fn text(&self, input: &str) -> Text {
        match *self {
            KeywordError::GameModeInvalid(word) => {
                Text::translate("argument.gamemode.invalid", vec![span_text(input, word)])
            }
            KeywordError::AnchorInvalid(word) => {
                Text::translate("argument.anchor.invalid", vec![span_text(input, word)])
            }
            KeywordError::EnumInvalid(word) => {
                Text::translate("argument.enum.invalid", vec![span_text(input, word)])
            }
        }
    }
}
