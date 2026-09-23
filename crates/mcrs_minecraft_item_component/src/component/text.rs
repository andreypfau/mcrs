use mcrs_minecraft_core::ResourceLocation;
use serde::{Deserialize, Deserializer, Serialize};

use crate::component::book::size_limited;
use crate::component::common::lenient;
use crate::component::enums::DyeColor;
use crate::harness::Sample;
use mcrs_minecraft_core::Bounded;
use mcrs_minecraft_text::IntoText;

use crate::Text;

macro_rules! text_newtype {
    ($($ty:ident),* $(,)?) => {$(
        #[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $ty(pub Text);

        impl Sample for $ty {
            fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
                use mcrs_minecraft_nbt::{BYTE_ID, COMPOUND_ID, STRING_ID};
                match &self.0.content {
                    mcrs_minecraft_text::TextContent::Text { .. } if self.0.italic.is_none() => {
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

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Lore {
    pub lines: Bounded<Vec<Text>, MAX_LORE_LINES>,
}

impl Lore {
    pub const fn new(lines: Vec<Text>) -> Self {
        Self {
            lines: Bounded(lines),
        }
    }
}

impl<'de> Deserialize<'de> for Lore {
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
