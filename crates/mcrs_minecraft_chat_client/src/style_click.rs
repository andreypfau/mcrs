use mcrs_minecraft_protocol::text::{ClickEvent, TextTypes};

/// The parts of a text style that a click acts on.
#[derive(Debug, PartialEq)]
pub struct StyleClick<'a, I: TextTypes = ()> {
    pub insertion: Option<&'a str>,
    pub event: Option<&'a ClickEvent<I>>,
}

impl<I: TextTypes> Default for StyleClick<'_, I> {
    fn default() -> Self {
        Self {
            insertion: None,
            event: None,
        }
    }
}

impl<I: TextTypes> Clone for StyleClick<'_, I> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<I: TextTypes> Copy for StyleClick<'_, I> {}
