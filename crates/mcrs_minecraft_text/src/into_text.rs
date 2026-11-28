use mcrs_minecraft_resource::ResourceLocationBuf;
use std::borrow::Cow;

use uuid::Uuid;

use super::{ClickEvent, Color, HoverEvent, ItemHover, Text, TextTypes};

pub trait IntoText<I: TextTypes = ItemHover>: Sized {
    fn into_text(self) -> Text<I>;

    fn color(self, color: impl Into<Color>) -> Text<I> {
        let mut value = self.into_text();
        value.style.color = Some(color.into());
        value
    }

    fn default_color(self, color: impl Into<Color>) -> Text<I> {
        let mut value = self.into_text();
        value.style.color = value.style.color.or_else(|| Some(color.into()));
        value
    }

    fn font(self, font: impl Into<ResourceLocationBuf>) -> Text<I> {
        let mut value = self.into_text();
        value.style.font = Some(font.into());
        value
    }

    fn bold(self) -> Text<I> {
        let mut value = self.into_text();
        value.style.bold = Some(true);
        value
    }
    fn not_bold(self) -> Text<I> {
        let mut value = self.into_text();
        value.style.bold = Some(false);
        value
    }

    fn italic(self) -> Text<I> {
        let mut value = self.into_text();
        value.style.italic = Some(true);
        value
    }
    fn not_italic(self) -> Text<I> {
        let mut value = self.into_text();
        value.style.italic = Some(false);
        value
    }

    fn underlined(self) -> Text<I> {
        let mut value = self.into_text();
        value.style.underlined = Some(true);
        value
    }
    fn not_underlined(self) -> Text<I> {
        let mut value = self.into_text();
        value.style.underlined = Some(false);
        value
    }

    fn strikethrough(self) -> Text<I> {
        let mut value = self.into_text();
        value.style.strikethrough = Some(true);
        value
    }
    fn not_strikethrough(self) -> Text<I> {
        let mut value = self.into_text();
        value.style.strikethrough = Some(false);
        value
    }

    fn obfuscated(self) -> Text<I> {
        let mut value = self.into_text();
        value.style.obfuscated = Some(true);
        value
    }
    fn not_obfuscated(self) -> Text<I> {
        let mut value = self.into_text();
        value.style.obfuscated = Some(false);
        value
    }

    fn insertion(self, insertion: impl Into<Cow<'static, str>>) -> Text<I> {
        let mut value = self.into_text();
        value.style.insertion = Some(insertion.into());
        value
    }

    fn on_click_suggest_command(self, command: impl Into<Cow<'static, str>>) -> Text<I> {
        let mut value = self.into_text();
        value.style.click_event = Some(ClickEvent::SuggestCommand {
            command: command.into(),
        });
        value
    }

    fn on_click_open_url(self, url: impl Into<Cow<'static, str>>) -> Text<I> {
        let mut value = self.into_text();
        value.style.click_event = Some(ClickEvent::OpenUrl { url: url.into() });
        value
    }

    fn on_hover_show_entity(self, id: I::EntityKey, uuid: Uuid, name: Option<Text<I>>) -> Text<I> {
        let mut value = self.into_text();
        value.style.hover_event = Some(HoverEvent::<I>::ShowEntity { id, uuid, name });
        value
    }

    fn on_hover_show_text(self, text: impl IntoText<I>) -> Text<I> {
        let mut value = self.into_text();
        value.style.hover_event = Some(HoverEvent::<I>::ShowText {
            value: text.into_text(),
        });
        value
    }

    fn add_child(self, text: impl IntoText<I>) -> Text<I> {
        let mut value = self.into_text();
        value.extra.push(text.into_text());
        value
    }
}

impl<I: TextTypes, T: Into<Text<I>>> IntoText<I> for T {
    fn into_text(self) -> Text<I> {
        self.into()
    }
}

impl<I: TextTypes> From<String> for Text<I> {
    fn from(value: String) -> Self {
        Text::text(value)
    }
}

impl<I: TextTypes> From<Cow<'static, str>> for Text<I> {
    fn from(value: Cow<'static, str>) -> Self {
        Text::text(value)
    }
}

impl<I: TextTypes> From<&'static str> for Text<I> {
    fn from(value: &'static str) -> Self {
        Text::text(value)
    }
}
