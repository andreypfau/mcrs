//! Provides the [`IntoText`] trait and implementations.

use std::borrow::Cow;

use mcrs_minecraft_core::ResourceLocation;

use super::{ClickEvent, Color, HoverEvent, HoverItem, Text};

/// Trait for any data that can be converted to a [`Text`] object.
///
/// Also conveniently provides many useful methods for modifying a [`Text`]
/// object.
///
/// # Usage
///
/// ```
/// # use mcrs_minecraft_text::{IntoText, Text, color::NamedColor};
/// let mut my_text: Text = "".into_text();
/// my_text = my_text.color(NamedColor::Red).bold();
/// my_text = my_text.add_child("CRABBBBB".obfuscated());
pub trait IntoText<I: HoverItem = ()>: Sized {
    /// Converts to an owned [`Text`] object.
    fn into_text(self) -> Text<I>;

    /// Sets the color of the text.
    fn color(self, color: impl Into<Color>) -> Text<I> {
        let mut value = self.into_text();
        value.color = Some(color.into());
        value
    }

    /// Sets the font of the text.
    fn font(self, font: impl Into<ResourceLocation>) -> Text<I> {
        let mut value = self.into_text();
        value.font = Some(font.into());
        value
    }

    /// Makes the text bold.
    fn bold(self) -> Text<I> {
        let mut value = self.into_text();
        value.bold = Some(true);
        value
    }
    /// Makes the text not bold.
    fn not_bold(self) -> Text<I> {
        let mut value = self.into_text();
        value.bold = Some(false);
        value
    }

    /// Makes the text italic.
    fn italic(self) -> Text<I> {
        let mut value = self.into_text();
        value.italic = Some(true);
        value
    }
    /// Makes the text not italic.
    fn not_italic(self) -> Text<I> {
        let mut value = self.into_text();
        value.italic = Some(false);
        value
    }

    /// Makes the text underlined.
    fn underlined(self) -> Text<I> {
        let mut value = self.into_text();
        value.underlined = Some(true);
        value
    }
    /// Makes the text not underlined.
    fn not_underlined(self) -> Text<I> {
        let mut value = self.into_text();
        value.underlined = Some(false);
        value
    }

    /// Adds a strikethrough effect to the text.
    fn strikethrough(self) -> Text<I> {
        let mut value = self.into_text();
        value.strikethrough = Some(true);
        value
    }
    /// Removes the strikethrough effect from the text.
    fn not_strikethrough(self) -> Text<I> {
        let mut value = self.into_text();
        value.strikethrough = Some(false);
        value
    }

    /// Makes the text obfuscated.
    fn obfuscated(self) -> Text<I> {
        let mut value = self.into_text();
        value.obfuscated = Some(true);
        value
    }
    /// Makes the text not obfuscated.
    fn not_obfuscated(self) -> Text<I> {
        let mut value = self.into_text();
        value.obfuscated = Some(false);
        value
    }

    /// Adds an `insertion` property to the text. When shift-clicked, the given
    /// text will be inserted into chat box for the client.
    fn insertion(self, insertion: impl Into<Cow<'static, str>>) -> Text<I> {
        let mut value = self.into_text();
        value.insertion = Some(insertion.into());
        value
    }

    /// On click, copies the given text to the chat box.
    fn on_click_suggest_command(self, command: impl Into<Cow<'static, str>>) -> Text<I> {
        let mut value = self.into_text();
        value.click_event = Some(ClickEvent::SuggestCommand {
            command: command.into(),
        });
        value
    }

    /// On mouse hover, shows the given text in a tooltip.
    fn on_hover_show_text(self, text: impl IntoText<I>) -> Text<I> {
        let mut value = self.into_text();
        value.hover_event = Some(HoverEvent::<I>::ShowText {
            value: text.into_text(),
        });
        value
    }

    /// Adds a child [`Text`] object.
    fn add_child(self, text: impl IntoText<I>) -> Text<I> {
        let mut value = self.into_text();
        value.extra.push(text.into_text());
        value
    }
}

impl<I: HoverItem, T: Into<Text<I>>> IntoText<I> for T {
    fn into_text(self) -> Text<I> {
        self.into()
    }
}

impl<I: HoverItem> From<String> for Text<I> {
    fn from(value: String) -> Self {
        Text::text(value)
    }
}

impl<I: HoverItem> From<Cow<'static, str>> for Text<I> {
    fn from(value: Cow<'static, str>) -> Self {
        Text::text(value)
    }
}

impl<I: HoverItem> From<&'static str> for Text<I> {
    fn from(value: &'static str) -> Self {
        Text::text(value)
    }
}
