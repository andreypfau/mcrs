use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

use crate::item::component::common::stub_component;
use crate::item::ctx::ctx_free;
use crate::item::harness::Sample;
use crate::text::{IntoText, Text};
use crate::{Bounded, Decode, Encode};

macro_rules! text_newtype {
    ($($ty:ident),* $(,)?) => {$(
        #[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize, Encode, Decode)]
        #[serde(transparent)]
        pub struct $ty(pub Text);

        impl Sample for $ty {
            fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
                use mcrs_minecraft_nbt::{BYTE_ID, COMPOUND_ID, STRING_ID};
                match &self.0.content {
                    crate::text::TextContent::Text { .. } if self.0.italic.is_none() => {
                        vec![("", STRING_ID)]
                    }
                    crate::text::TextContent::Text { .. } => vec![
                        ("", COMPOUND_ID),
                        ("text", STRING_ID),
                        ("italic", BYTE_ID),
                        ("color", STRING_ID),
                    ],
                    _ => vec![("", COMPOUND_ID), ("translate", STRING_ID)],
                }
            }

            fn samples() -> Vec<Self> {
                vec![
                    $ty(Text::text("plain")),
                    $ty("styled".italic().color(crate::text::Color::RED)),
                    $ty(Text::translate("item.minecraft.stone", Vec::new())),
                ]
            }
        }
    )*};
}

text_newtype!(CustomName, ItemName);
ctx_free!(CustomName, ItemName);

pub const MAX_LORE_LINES: usize = 256;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Encode, Decode)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
#[serde(transparent)]
pub struct Lore {
    lines: Bounded<Vec<Text>, MAX_LORE_LINES>,
}

impl Lore {
    pub const fn new(lines: Vec<Text>) -> Self {
        Self {
            lines: Bounded(lines),
        }
    }

    pub fn lines(&self) -> &Vec<Text> {
        &self.lines.0
    }
}

impl<'de> Deserialize<'de> for Lore {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let lines = Vec::<Text>::deserialize(d)?;
        if lines.len() > MAX_LORE_LINES {
            return Err(D::Error::custom(format_args!(
                "List is too long: {}, expected range [0-{MAX_LORE_LINES}]",
                lines.len()
            )));
        }
        Ok(Lore::new(lines))
    }
}

ctx_free!(Lore);

impl Sample for Lore {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        vec![("", mcrs_minecraft_nbt::LIST_ID)]
    }

    fn samples() -> Vec<Self> {
        vec![
            Lore::new(Vec::new()),
            Lore::new(vec![Text::text("one")]),
            Lore::new(vec![Text::text("plain"), "bold".bold()]),
        ]
    }
}

stub_component!(
    ItemModel,
    TooltipStyle,
    NoteBlockSound,
    SignTextFront,
    SignTextBack,
);
