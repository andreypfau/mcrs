use std::borrow::Cow;

use mcrs_minecraft_core::ResourceLocation;
use serde::{Deserialize, Serialize};

use crate::{ClickEvent, Color, HoverEvent, TextMap, TextSerde, TextTypes};
use mcrs_minecraft_core::codec::{ArgbInt, optional_flag};

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(
    deny_unknown_fields,
    bound(serialize = "I: TextSerde", deserialize = "I: TextSerde")
)]
pub struct Style<I: TextTypes = ()> {
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
    pub font: Option<ResourceLocation>,
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
