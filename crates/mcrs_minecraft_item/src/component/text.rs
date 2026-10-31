use mcrs_minecraft_registries::{Named, Refs, Resolved, registry_value};
use mcrs_minecraft_resource::{ResourceLocationBuf, rl};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};

use crate::component::book::size_limited;
use crate::component::common::lenient;
use crate::component::enums::DyeColor;
use crate::harness::Sample;
use mcrs_minecraft_core::Bounded;
use mcrs_minecraft_text::IntoText;

use crate::Text;
use mcrs_minecraft_registries::Wire;
use mcrs_minecraft_wire::{Decode, Encode, Result};
use std::io::Write;

macro_rules! text_newtype {
    ($($ty:ident),* $(,)?) => {$(
        #[registry_value]
#[derive(mcrs_minecraft_registries::FromWire)]
        #[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
        #[derive(mcrs_minecraft_wire::Encode, mcrs_minecraft_wire::Decode)]
        #[serde(transparent)]
        pub struct $ty<M: Refs = Resolved>(pub Text<M::Nbt>);

        impl Sample for $ty {
            fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
                use mcrs_minecraft_nbt::{BYTE_ID, COMPOUND_ID, STRING_ID};
                match &self.0.content {
                    mcrs_minecraft_text::TextContent::Text { .. } if self.0.style.italic.is_none() => {
                        vec![("", STRING_ID)]
                    }
                    mcrs_minecraft_text::TextContent::Text { .. } => vec![
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
                    $ty("styled".italic().color(mcrs_minecraft_text::Color::RED)),
                    $ty(Text::translate("item.minecraft.stone", Vec::new())),
                ]
            }
        }
    )*};
}

text_newtype!(CustomName, ItemName);
pub const MAX_LORE_LINES: usize = 256;

#[registry_value]
#[derive(
    mcrs_minecraft_registries::FromWire, Clone, Debug, Default, PartialEq, Serialize, Encode, Decode,
)]
#[serde(transparent)]
pub struct Lore<M: Refs = Resolved> {
    pub lines: Bounded<Vec<Text<M::Nbt>>, MAX_LORE_LINES>,
}

impl<M: Refs> Lore<M> {
    pub const fn new(lines: Vec<Text<M::Nbt>>) -> Self {
        Self {
            lines: Bounded(lines),
        }
    }
}

impl<'de> Deserialize<'de> for Lore<Named> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        size_limited(d).map(|lines| Self { lines })
    }
}

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

macro_rules! location_newtype {
    ($($ty:ident),* $(,)?) => {$(
        #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[derive(mcrs_minecraft_wire::Encode, mcrs_minecraft_wire::Decode)]
        #[serde(transparent)]
        pub struct $ty(pub ResourceLocationBuf);

        impl Sample for $ty {
            fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
                vec![("", mcrs_minecraft_nbt::STRING_ID)]
            }

            fn samples() -> Vec<Self> {
                vec![
                    $ty(rl!("minecraft:stone").to_buf()),
                    $ty(rl!("custom:style/dir").to_buf()),
                ]
            }
        }
    )*};
}

location_newtype!(ItemModel, TooltipStyle, NoteBlockSound);
fn black() -> DyeColor {
    DyeColor::Black
}

/// `filtered_messages` is absent whenever it equals `messages`, on read and
/// on write alike, so two texts that render the same compare equal.
#[registry_value]
#[derive(mcrs_minecraft_registries::FromWire, Clone, Debug, PartialEq, Serialize)]
pub struct SignText<M: Refs = Resolved> {
    pub messages: [Text<M::Nbt>; 4],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filtered_messages: Option<[Text<M::Nbt>; 4]>,
    pub color: DyeColor,
    pub has_glowing_text: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, bound = "Text<M::Nbt>: DeserializeOwned")]
struct SignTextRepr<M: Refs> {
    messages: [Text<M::Nbt>; 4],
    #[serde(default, deserialize_with = "lenient")]
    filtered_messages: Option<[Text<M::Nbt>; 4]>,
    #[serde(default = "black")]
    color: DyeColor,
    #[serde(default)]
    has_glowing_text: bool,
}

impl<'de> Deserialize<'de> for SignText<Named> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        SignTextRepr::deserialize(d).map(SignText::from)
    }
}

impl<M: Refs> From<SignTextRepr<M>> for SignText<M> {
    fn from(repr: SignTextRepr<M>) -> Self {
        SignText::new(
            repr.messages,
            repr.filtered_messages,
            repr.color,
            repr.has_glowing_text,
        )
    }
}

impl<M: Refs> SignText<M> {
    pub fn new(
        messages: [Text<M::Nbt>; 4],
        filtered_messages: Option<[Text<M::Nbt>; 4]>,
        color: DyeColor,
        has_glowing_text: bool,
    ) -> Self {
        SignText {
            filtered_messages: filtered_messages.filter(|filtered| *filtered != messages),
            messages,
            color,
            has_glowing_text,
        }
    }
}

impl<M: Refs> Default for SignText<M> {
    fn default() -> Self {
        SignText::new(Default::default(), None, DyeColor::Black, false)
    }
}

impl Sample for SignText {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        use mcrs_minecraft_nbt::{BYTE_ID, COMPOUND_ID, LIST_ID, STRING_ID};
        let mut tags = vec![
            ("", COMPOUND_ID),
            ("messages", LIST_ID),
            ("color", STRING_ID),
            ("has_glowing_text", BYTE_ID),
        ];
        if self.filtered_messages.is_some() {
            tags.push(("filtered_messages", LIST_ID));
        }
        tags
    }

    fn samples() -> Vec<Self> {
        let lines = |b: Text| [Text::text("a"), b, Text::text("c"), Text::text("d")];
        vec![
            SignText::default(),
            SignText::new(lines(Text::text("b")), None, DyeColor::Black, false),
            SignText::new(
                lines("b".bold()),
                Some(lines(Text::text("x"))),
                DyeColor::Red,
                true,
            ),
        ]
    }
}

macro_rules! sign_newtype {
    ($($ty:ident),* $(,)?) => {$(
        #[registry_value]
#[derive(mcrs_minecraft_registries::FromWire)]
        #[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
        #[derive(mcrs_minecraft_wire::Encode, mcrs_minecraft_wire::Decode)]
        #[serde(transparent)]
        pub struct $ty<M: Refs = Resolved>(pub SignText<M>);

        impl Sample for $ty {
            fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
                self.0.nbt_tags()
            }

            fn samples() -> Vec<Self> {
                SignText::samples().into_iter().map($ty).collect()
            }
        }
    )*};
}

sign_newtype!(SignTextFront, SignTextBack);

impl Encode for SignText<Wire> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        self.messages.encode(&mut w)?;
        self.filtered_messages.encode(&mut w)?;
        self.color.encode(&mut w)?;
        self.has_glowing_text.encode(w)
    }
}

impl Decode<'_> for SignText<Wire> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        Ok(SignText::new(
            Decode::decode(r)?,
            Option::decode(r)?,
            DyeColor::decode(r)?,
            bool::decode(r)?,
        ))
    }
}
