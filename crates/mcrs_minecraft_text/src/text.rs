use std::borrow::Cow;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;
use std::{fmt, ops};

use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, de};

use crate::{
    DataSource, IntoText, ItemHover, ScoreboardValueContent, TextContent, TextInner, TextMap,
    TextSerde, TextTypes, TranslateArg,
};

#[derive(Clone, PartialEq)]
pub struct Text<I: TextTypes = ItemHover>(Box<TextInner<I>>);

impl<I: TextTypes> Default for Text<I> {
    fn default() -> Self {
        Text(Box::default())
    }
}

impl<I: TextSerde> Serialize for Text<I> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self.0.collapse_to_string() {
            Some(text) => s.serialize_str(text),
            None => self.0.serialize(s),
        }
    }
}

#[expect(
    clippy::self_named_constructors,
    reason = "Text::text is the plain-text constructor; callers write it by that name"
)]
impl<I: TextTypes> Text<I> {
    pub fn text(plain: impl Into<Cow<'static, str>>) -> Self {
        Self(Box::new(TextInner {
            content: TextContent::Text { text: plain.into() },
            ..Default::default()
        }))
    }

    pub fn translate(key: impl Into<Cow<'static, str>>, with: impl Into<Vec<Text<I>>>) -> Self {
        let with: Vec<_> = with.into().into_iter().map(TranslateArg::Text).collect();
        Self::translate_args(key, with)
    }

    pub fn translate_args(
        key: impl Into<Cow<'static, str>>,
        with: impl Into<Vec<TranslateArg<I>>>,
    ) -> Self {
        Self(Box::new(TextInner {
            content: TextContent::Translate {
                translate: key.into(),
                fallback: None,
                with: with.into(),
            },
            ..Default::default()
        }))
    }

    pub fn score(
        name: impl Into<Cow<'static, str>>,
        objective: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self(Box::new(TextInner {
            content: TextContent::ScoreboardValue {
                score: ScoreboardValueContent {
                    name: name.into(),
                    objective: objective.into(),
                },
            },
            ..Default::default()
        }))
    }

    pub fn selector(selector: impl Into<Cow<'static, str>>, separator: Option<Text<I>>) -> Self {
        Self(Box::new(TextInner {
            content: TextContent::EntityNames {
                selector: selector.into(),
                separator,
            },
            ..Default::default()
        }))
    }

    pub fn keybind(keybind: impl Into<Cow<'static, str>>) -> Self {
        Self(Box::new(TextInner {
            content: TextContent::Keybind {
                keybind: keybind.into(),
            },
            ..Default::default()
        }))
    }

    pub fn nbt(
        source: DataSource,
        nbt: impl Into<Cow<'static, str>>,
        interpret: bool,
        separator: Option<Text<I>>,
    ) -> Self {
        Self(Box::new(TextInner {
            content: TextContent::Nbt {
                nbt: nbt.into(),
                interpret,
                plain: false,
                separator,
                source,
            },
            ..Default::default()
        }))
    }
}

impl<A: TextTypes> Text<A> {
    pub fn map<B: TextTypes, T: TextMap<A, B>>(&self, map: &T) -> Result<Text<B>, T::Error> {
        let extra = self
            .0
            .extra
            .iter()
            .enumerate()
            .map(|(at, text)| {
                text.map(map)
                    .map_err(|e| T::at(T::at_index(e, at), "extra"))
            })
            .collect::<Result<_, _>>()?;
        Ok(Text(Box::new(TextInner {
            content: self.0.content.map(map)?,
            extra,
            style: self.0.style.map(map)?,
        })))
    }
}

pub(crate) fn map_text<A: TextTypes, B: TextTypes, T: TextMap<A, B>>(
    text: &Option<Text<A>>,
    map: &T,
    field: &'static str,
) -> Result<Option<Text<B>>, T::Error> {
    text.as_ref()
        .map(|text| text.map(map).map_err(|e| T::at(e, field)))
        .transpose()
}

impl<I: TextTypes> Deref for Text<I> {
    type Target = TextInner<I>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<I: TextTypes> DerefMut for Text<I> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<I: TextTypes, T: IntoText<I>> ops::Add<T> for Text<I> {
    type Output = Self;

    fn add(self, rhs: T) -> Self::Output {
        self.add_child(rhs)
    }
}

impl<I: TextTypes, T: IntoText<I>> ops::AddAssign<T> for Text<I> {
    fn add_assign(&mut self, rhs: T) {
        self.extra.push(rhs.into_text());
    }
}

impl<'a, I: TextTypes> From<Text<I>> for Cow<'a, Text<I>> {
    fn from(value: Text<I>) -> Self {
        Cow::Owned(value)
    }
}

impl<'a, I: TextTypes> From<&'a Text<I>> for Cow<'a, Text<I>> {
    fn from(value: &'a Text<I>) -> Self {
        Cow::Borrowed(value)
    }
}

impl<I: TextSerde> FromStr for Text<I> {
    type Err = serde_json::error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            Ok(Text::default())
        } else {
            serde_json::from_str(s)
        }
    }
}

impl<I: TextSerde> From<Text<I>> for String {
    fn from(value: Text<I>) -> Self {
        format!("{value}")
    }
}

impl<I: TextTypes> fmt::Debug for Text<I> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl<I: TextSerde> fmt::Display for Text<I> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let string = if f.alternate() {
            serde_json::to_string_pretty(self)
        } else {
            serde_json::to_string(self)
        }
        .map_err(|_| fmt::Error)?;

        f.write_str(&string)
    }
}

impl<'de, I: TextSerde> Deserialize<'de> for Text<I> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct TextVisitor<I>(PhantomData<I>);

        impl<'de, I: TextSerde> Visitor<'de> for TextVisitor<I> {
            type Value = Text<I>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "a text component data type")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(Text::text(v.to_string()))
            }

            fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
                Ok(Text::text(v))
            }

            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let Some(mut res) = seq.next_element()? else {
                    return Err(de::Error::custom("List must have contents"));
                };

                while let Some(child) = seq.next_element::<Text<I>>()? {
                    res += child;
                }

                Ok(res)
            }

            fn visit_map<A: de::MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                use de::value::MapAccessDeserializer;

                Ok(Text(Box::new(TextInner::deserialize(
                    MapAccessDeserializer::new(map),
                )?)))
            }
        }

        deserializer.deserialize_any(TextVisitor(PhantomData))
    }
}
