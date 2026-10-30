use core::fmt;

use mcrs_minecraft_item::Template;
use mcrs_minecraft_registries::Named;
use mcrs_minecraft_text::{Text, TextMap};

/// A text names an item, an entity or a dialog, which a text without game data cannot carry
/// onto the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlainTextError;

impl fmt::Display for PlainTextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a plain text cannot name an item, an entity or a dialog")
    }
}

impl core::error::Error for PlainTextError {}

struct Plain;

impl TextMap<(), Template<Named>> for Plain {
    type Error = PlainTextError;

    fn item(&self, _: &()) -> Result<Template<Named>, PlainTextError> {
        Err(PlainTextError)
    }

    fn entity(
        &self,
        _: &<() as mcrs_minecraft_text::TextTypes>::EntityKey,
    ) -> Result<<Template<Named> as mcrs_minecraft_text::TextTypes>::EntityKey, PlainTextError>
    {
        Err(PlainTextError)
    }

    fn dialog(
        &self,
        _: &<() as mcrs_minecraft_text::TextTypes>::Dialog,
    ) -> Result<<Template<Named> as mcrs_minecraft_text::TextTypes>::Dialog, PlainTextError> {
        Err(PlainTextError)
    }

    fn at(error: PlainTextError, _: &'static str) -> PlainTextError {
        error
    }

    fn at_index(error: PlainTextError, _: usize) -> PlainTextError {
        error
    }
}

/// Converts a text without game data to the named form that a packet carries.
pub fn named(text: &Text<()>) -> Result<crate::Text, PlainTextError> {
    text.map(&Plain)
}
