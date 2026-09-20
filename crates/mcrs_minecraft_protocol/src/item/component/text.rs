use std::io::Write;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registry::RegistryLookup;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

use crate::entity::DyeColor;
use crate::item::component::common::lenient;
use crate::item::ctx::{DecodeCtx, EncodeCtx, ctx_free};
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

macro_rules! location_newtype {
    ($($ty:ident),* $(,)?) => {$(
        #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Encode, Decode)]
        #[serde(transparent)]
        pub struct $ty(pub ResourceLocation);

        impl Sample for $ty {
            fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
                vec![("", mcrs_minecraft_nbt::STRING_ID)]
            }

            fn samples() -> Vec<Self> {
                vec![
                    $ty(ResourceLocation::minecraft("stone")),
                    $ty(ResourceLocation::new("custom", "style/dir")),
                ]
            }
        }
    )*};
}

location_newtype!(ItemModel, TooltipStyle, NoteBlockSound);
ctx_free!(ItemModel, TooltipStyle, NoteBlockSound);

fn black() -> DyeColor {
    DyeColor::Black
}

/// `filtered_messages` is absent whenever it equals `messages`, on read and
/// on write alike, so two texts that render the same compare equal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "SignTextRepr")]
pub struct SignText {
    pub messages: [Text; 4],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filtered_messages: Option<[Text; 4]>,
    pub color: DyeColor,
    pub has_glowing_text: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SignTextRepr {
    messages: [Text; 4],
    #[serde(default, deserialize_with = "lenient")]
    filtered_messages: Option<[Text; 4]>,
    #[serde(default = "black")]
    color: DyeColor,
    #[serde(default)]
    has_glowing_text: bool,
}

impl From<SignTextRepr> for SignText {
    fn from(repr: SignTextRepr) -> Self {
        SignText::new(
            repr.messages,
            repr.filtered_messages,
            repr.color,
            repr.has_glowing_text,
        )
    }
}

impl SignText {
    pub fn new(
        messages: [Text; 4],
        filtered_messages: Option<[Text; 4]>,
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

impl Default for SignText {
    fn default() -> Self {
        SignText::new(Default::default(), None, DyeColor::Black, false)
    }
}

impl EncodeCtx for SignText {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.messages.encode_ctx(ctx, &mut w)?;
        self.filtered_messages.encode_ctx(ctx, &mut w)?;
        self.color.encode(&mut w)?;
        self.has_glowing_text.encode(w)
    }
}

impl DecodeCtx<'_> for SignText {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(SignText::new(
            <[Text; 4]>::decode_ctx(ctx, r)?,
            Option::decode_ctx(ctx, r)?,
            DyeColor::decode(r)?,
            bool::decode(r)?,
        ))
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
        #[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $ty(pub SignText);

        impl EncodeCtx for $ty {
            fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
                self.0.encode_ctx(ctx, w)
            }
        }

        impl DecodeCtx<'_> for $ty {
            fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
                SignText::decode_ctx(ctx, r).map($ty)
            }
        }

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
