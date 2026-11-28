use mcrs_minecraft_resource::ResourceLocationBuf;
use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::{
    ClickEvent, Color, DeclaredStyle, GlyphFont, HoverEvent, ItemHover, TextMap, TextSerde,
    TextTypes,
};
use mcrs_minecraft_core::codec::{ArgbInt, optional_flag};

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(
    deny_unknown_fields,
    bound(serialize = "I: TextSerde", deserialize = "I: TextSerde")
)]
pub struct Style<I: TextTypes = ItemHover> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow_color: Option<ArgbInt>,

    #[serde(
        default,
        deserialize_with = "optional_flag",
        skip_serializing_if = "Option::is_none"
    )]
    pub bold: Option<bool>,

    #[serde(
        default,
        deserialize_with = "optional_flag",
        skip_serializing_if = "Option::is_none"
    )]
    pub italic: Option<bool>,

    #[serde(
        default,
        deserialize_with = "optional_flag",
        skip_serializing_if = "Option::is_none"
    )]
    pub underlined: Option<bool>,

    #[serde(
        default,
        deserialize_with = "optional_flag",
        skip_serializing_if = "Option::is_none"
    )]
    pub strikethrough: Option<bool>,

    #[serde(
        default,
        deserialize_with = "optional_flag",
        skip_serializing_if = "Option::is_none"
    )]
    pub obfuscated: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub click_event: Option<ClickEvent<I>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hover_event: Option<HoverEvent<I>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insertion: Option<Cow<'static, str>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<ResourceLocationBuf>,
}

impl<I: TextTypes> Default for Style<I> {
    fn default() -> Self {
        Style {
            color: None,
            shadow_color: None,
            bold: None,
            italic: None,
            underlined: None,
            strikethrough: None,
            obfuscated: None,
            click_event: None,
            hover_event: None,
            insertion: None,
            font: None,
        }
    }
}

impl<I: TextTypes> Style<I> {
    pub fn is_empty(&self) -> bool {
        *self == Style::default()
    }

    #[must_use]
    pub fn declared(&self) -> DeclaredStyle {
        DeclaredStyle {
            click_event: self.click_event.is_some(),
            hover_event: self.hover_event.is_some(),
            font: self.font.is_some(),
            strikethrough_enabled: self.strikethrough == Some(true),
            underlined_enabled: self.underlined == Some(true),
        }
    }

    #[must_use]
    pub fn formatting<B: TextTypes>(&self) -> Style<B> {
        Style {
            color: self.color,
            bold: self.bold,
            italic: self.italic,
            underlined: self.underlined,
            strikethrough: self.strikethrough,
            obfuscated: self.obfuscated,
            ..Style::default()
        }
    }

    #[must_use]
    pub fn resolve<'a>(&'a self, parent: ResolvedStyle<'a, I>) -> ResolvedStyle<'a, I> {
        ResolvedStyle {
            color: self.color.or(parent.color),
            shadow_color: self.shadow_color.or(parent.shadow_color),
            obfuscated: self.obfuscated.unwrap_or(parent.obfuscated),
            bold: self.bold.unwrap_or(parent.bold),
            strikethrough: self.strikethrough.unwrap_or(parent.strikethrough),
            underlined: self.underlined.unwrap_or(parent.underlined),
            italic: self.italic.unwrap_or(parent.italic),
            click_event: self.click_event.as_ref().or(parent.click_event),
            hover_event: self.hover_event.as_ref().or(parent.hover_event),
            insertion: self.insertion.as_deref().or(parent.insertion),
            font: self.font.as_ref().map(GlyphFont::Named).or(parent.font),
        }
    }
}

#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub struct ResolvedStyle<'a, I: TextTypes = ItemHover> {
    pub color: Option<Color>,
    pub shadow_color: Option<ArgbInt>,
    pub obfuscated: bool,
    pub bold: bool,
    pub strikethrough: bool,
    pub underlined: bool,
    pub italic: bool,
    pub click_event: Option<&'a ClickEvent<I>>,
    pub hover_event: Option<&'a HoverEvent<I>>,
    pub insertion: Option<&'a str>,
    pub font: Option<GlyphFont<'a>>,
}

impl<I: TextTypes> Clone for ResolvedStyle<'_, I> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<I: TextTypes> Copy for ResolvedStyle<'_, I> {}

impl<I: TextTypes> Default for ResolvedStyle<'_, I> {
    fn default() -> Self {
        Self {
            color: None,
            shadow_color: None,
            obfuscated: false,
            bold: false,
            strikethrough: false,
            underlined: false,
            italic: false,
            click_event: None,
            hover_event: None,
            insertion: None,
            font: None,
        }
    }
}

impl<A: TextTypes> Style<A> {
    pub fn map<B: TextTypes, T: TextMap<A, B>>(&self, map: &T) -> Result<Style<B>, T::Error> {
        let click_event = self
            .click_event
            .as_ref()
            .map(|event| event.map(map).map_err(|e| T::at(e, "click_event")))
            .transpose()?;
        let hover_event = self
            .hover_event
            .as_ref()
            .map(|event| event.map(map).map_err(|e| T::at(e, "hover_event")))
            .transpose()?;
        Ok(Style {
            color: self.color,
            shadow_color: self.shadow_color,
            bold: self.bold,
            italic: self.italic,
            underlined: self.underlined,
            strikethrough: self.strikethrough,
            obfuscated: self.obfuscated,
            click_event,
            hover_event,
            insertion: self.insertion.clone(),
            font: self.font.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn style(value: &Value) -> Result<Style, serde_json::Error> {
        serde_json::from_value(value.clone())
    }

    #[test]
    fn a_chat_style_round_trips_every_key() {
        for field in [
            json!({"color": "gray"}),
            json!({"color": "#12AB34"}),
            json!({"shadow_color": -16777216}),
            json!({"bold": true}),
            json!({"italic": false}),
            json!({"underlined": true}),
            json!({"strikethrough": true}),
            json!({"obfuscated": false}),
            json!({"click_event": {"action": "run_command", "command": "/list"}}),
            json!({"hover_event": {"action": "show_text", "value": "hello"}}),
            json!({"insertion": "text"}),
            json!({"font": "minecraft:uniform"}),
            json!({}),
        ] {
            let read = style(&field).unwrap_or_else(|e| panic!("{field}: {e}"));
            assert_eq!(serde_json::to_value(&read).unwrap(), field);
        }
    }
}
