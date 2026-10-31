use std::borrow::Cow;
use std::fmt;
use std::marker::PhantomData;

use mcrs_minecraft_core::codec::non_empty;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::from_tag;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_resource::ResourceLocation;
use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, de};

use crate::{
    ClickEvent, Color, HoverEvent, Style, Text, TextContent, TextSerde, TextTypes, TranslateArg,
};
use mcrs_minecraft_core::codec::{ArgbInt, optional_flag};

/// Text data and formatting.
#[derive(Clone, PartialEq, Debug, Serialize)]
#[serde(bound(serialize = "I: TextSerde"))]
pub struct TextInner<I: TextTypes> {
    #[serde(flatten)]
    pub content: TextContent<I>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<Text<I>>,

    #[serde(flatten)]
    pub style: Style<I>,
}

/// Style keys are read as they arrive; every other key is kept as the tag it
/// came as and handed to the content codec once the map ends. Serde's own
/// `flatten` buffer would do the same through `Content`, which has no array
/// tags, so an int array below `with` or a hover item would come back a list.
/// `with` is read directly too: a JSON `true` argument is a boolean, which
/// an NBT buffer would turn into a byte.
impl<I: TextTypes> Default for TextInner<I> {
    fn default() -> Self {
        TextInner {
            content: TextContent::default(),
            extra: Vec::new(),
            style: Style::default(),
        }
    }
}

impl<'de, I: TextSerde> Deserialize<'de> for TextInner<I> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Flag(Option<bool>);

        impl<'de> Deserialize<'de> for Flag {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                optional_flag(d).map(Flag)
            }
        }

        struct Siblings<I: TextTypes>(Vec<Text<I>>);

        impl<'de, I: TextSerde> Deserialize<'de> for Siblings<I> {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                non_empty(d).map(Siblings)
            }
        }

        struct InnerVisitor<I>(PhantomData<I>);

        impl<'de, I: TextSerde> Visitor<'de> for InnerVisitor<I> {
            type Value = TextInner<I>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a text component")
            }

            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut inner = TextInner::default();
                let mut content = NbtCompound::new();
                let mut with: Option<Vec<TranslateArg<I>>> = None;
                macro_rules! style {
                    ($($field:ident).+, $ty:ty, $get:expr) => {{
                        let value: $ty = map.next_value()?;
                        inner.$($field).+ = $get(value);
                    }};
                }
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "extra" => style!(extra, Siblings<I>, |v: Siblings<I>| v.0),
                        "color" => style!(style.color, Color, Some),
                        "shadow_color" => style!(style.shadow_color, ArgbInt, Some),
                        "bold" => style!(style.bold, Flag, |v: Flag| v.0),
                        "italic" => style!(style.italic, Flag, |v: Flag| v.0),
                        "underlined" => style!(style.underlined, Flag, |v: Flag| v.0),
                        "strikethrough" => {
                            style!(style.strikethrough, Flag, |v: Flag| v.0)
                        }
                        "obfuscated" => style!(style.obfuscated, Flag, |v: Flag| v.0),
                        "click_event" => style!(style.click_event, ClickEvent<I>, Some),
                        "hover_event" => style!(style.hover_event, HoverEvent<I>, Some),
                        "insertion" => {
                            style!(style.insertion, String, |v: String| Some(Cow::Owned(v)))
                        }
                        "font" => style!(style.font, ResourceLocation, Some),
                        "with" => with = Some(map.next_value()?),
                        _ => content.child_tags.push((key, map.next_value::<NbtTag>()?)),
                    }
                }
                inner.content = from_tag(NbtTag::Compound(content)).map_err(de::Error::custom)?;
                if let (TextContent::Translate { with: args, .. }, Some(with)) =
                    (&mut inner.content, with)
                {
                    *args = with;
                }
                Ok(inner)
            }
        }

        d.deserialize_map(InnerVisitor(PhantomData))
    }
}

impl<I: TextTypes> TextInner<I> {
    pub(crate) fn collapse_to_string(&self) -> Option<&str> {
        match &self.content {
            TextContent::Text { text, .. } if self.extra.is_empty() && self.style.is_empty() => {
                Some(text)
            }
            _ => None,
        }
    }
}
