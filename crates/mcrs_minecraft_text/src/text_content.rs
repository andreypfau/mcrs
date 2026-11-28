use std::borrow::Cow;

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::from_tag;
use mcrs_minecraft_nbt::tag::NbtTag;
use serde::{Deserialize, Deserializer, Serialize, de};

use crate::text::map_text;
use crate::{
    DataSource, ObjectInfo, ScoreboardValueContent, Text, TextMap, TextSerde, TextTypes,
    TranslateArg,
};
use mcrs_minecraft_core::codec::lenient;

#[derive(Clone, PartialEq, Debug, Serialize)]
#[serde(untagged, bound(serialize = "I: TextSerde"))]
pub enum TextContent<I: TextTypes> {
    Text {
        text: Cow<'static, str>,
    },
    Translate {
        translate: Cow<'static, str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        fallback: Option<Cow<'static, str>>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        with: Vec<TranslateArg<I>>,
    },
    Keybind {
        keybind: Cow<'static, str>,
    },
    ScoreboardValue {
        score: ScoreboardValueContent,
    },
    EntityNames {
        selector: Cow<'static, str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        separator: Option<Text<I>>,
    },
    Nbt {
        nbt: Cow<'static, str>,
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        interpret: bool,
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        plain: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        separator: Option<Text<I>>,
        #[serde(flatten)]
        source: DataSource,
    },
    Object {
        #[serde(flatten)]
        object: ObjectInfo,
        #[serde(skip_serializing_if = "Option::is_none")]
        fallback: Option<Text<I>>,
    },
}

impl<'de, I: TextSerde> Deserialize<'de> for TextContent<I> {
    #[expect(
        clippy::too_many_lines,
        reason = "the representation structs of each content shape sit beside the untagged dispatch they feed"
    )]
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct TextRepr {
            text: String,
        }

        #[derive(Deserialize)]
        #[serde(bound = "I: TextSerde")]
        struct TranslateRepr<I: TextTypes> {
            translate: String,
            #[serde(default, deserialize_with = "lenient")]
            fallback: Option<String>,
            #[serde(default)]
            with: Vec<TranslateArg<I>>,
        }

        #[derive(Deserialize)]
        struct KeybindRepr {
            keybind: String,
        }

        #[derive(Deserialize)]
        struct ScoreRepr {
            score: ScoreboardValueContent,
        }

        #[derive(Deserialize)]
        #[serde(bound = "I: TextSerde")]
        struct SelectorRepr<I: TextTypes> {
            selector: String,
            #[serde(default)]
            separator: Option<Text<I>>,
        }

        #[derive(Deserialize)]
        #[serde(bound = "I: TextSerde")]
        struct NbtRepr<I: TextTypes> {
            nbt: String,
            #[serde(default, deserialize_with = "lenient")]
            interpret: bool,
            #[serde(default, deserialize_with = "lenient")]
            plain: bool,
            #[serde(default, deserialize_with = "lenient")]
            separator: Option<Text<I>>,
            #[serde(flatten)]
            source: DataSource,
        }

        #[derive(Deserialize)]
        #[serde(bound = "I: TextSerde")]
        struct ObjectRepr<I: TextTypes> {
            #[serde(flatten)]
            object: ObjectInfo,
            #[serde(default)]
            fallback: Option<Text<I>>,
        }

        const TYPES: [&str; 7] = [
            "text",
            "translatable",
            "keybind",
            "score",
            "selector",
            "nbt",
            "object",
        ];

        fn read<I: TextSerde, E: de::Error>(
            name: &str,
            compound: &NbtCompound,
        ) -> Result<TextContent<I>, E> {
            let tag = || NbtTag::Compound(compound.clone());
            let content = match name {
                "text" => {
                    let TextRepr { text } = from_tag(tag()).map_err(E::custom)?;
                    TextContent::Text { text: text.into() }
                }
                "translatable" => {
                    let TranslateRepr::<I> {
                        translate,
                        fallback,
                        with,
                    } = from_tag(tag()).map_err(E::custom)?;
                    TextContent::Translate {
                        translate: translate.into(),
                        fallback: fallback.map(Cow::Owned),
                        with,
                    }
                }
                "keybind" => {
                    let KeybindRepr { keybind } = from_tag(tag()).map_err(E::custom)?;
                    TextContent::Keybind {
                        keybind: keybind.into(),
                    }
                }
                "score" => {
                    let ScoreRepr { score } = from_tag(tag()).map_err(E::custom)?;
                    TextContent::ScoreboardValue { score }
                }
                "selector" => {
                    let SelectorRepr::<I> {
                        selector,
                        separator,
                    } = from_tag(tag()).map_err(E::custom)?;
                    TextContent::EntityNames {
                        selector: selector.into(),
                        separator,
                    }
                }
                "nbt" => {
                    let NbtRepr::<I> {
                        nbt,
                        interpret,
                        plain,
                        separator,
                        source,
                    } = from_tag(tag()).map_err(E::custom)?;
                    if interpret && plain {
                        return Err(E::custom("'interpret' and 'plain' flags can't be both on"));
                    }
                    TextContent::Nbt {
                        nbt: nbt.into(),
                        interpret,
                        plain,
                        separator,
                        source,
                    }
                }
                "object" => {
                    let ObjectRepr::<I> { object, fallback } =
                        from_tag(tag()).map_err(E::custom)?;
                    TextContent::Object { object, fallback }
                }
                other => {
                    return Err(E::custom(format_args!("Unknown element id: {other}")));
                }
            };
            Ok(content)
        }

        let compound = NbtCompound::deserialize(d)?;
        match compound.get("type") {
            Some(NbtTag::String(name)) => read(name, &compound),
            Some(_) => Err(de::Error::custom("'type' is not a string")),
            None => TYPES
                .iter()
                .find_map(|name| read::<I, D::Error>(name, &compound).ok())
                .ok_or_else(|| {
                    de::Error::custom("data did not match any variant of untagged enum TextContent")
                }),
        }
    }
}

impl<A: TextTypes> TextContent<A> {
    pub(crate) fn map<B: TextTypes, T: TextMap<A, B>>(
        &self,
        map: &T,
    ) -> Result<TextContent<B>, T::Error> {
        Ok(match self {
            TextContent::Text { text } => TextContent::Text { text: text.clone() },
            TextContent::Translate {
                translate,
                fallback,
                with,
            } => TextContent::Translate {
                translate: translate.clone(),
                fallback: fallback.clone(),
                with: with
                    .iter()
                    .enumerate()
                    .map(|(at, arg)| {
                        Ok(match arg {
                            TranslateArg::Bool(v) => TranslateArg::Bool(*v),
                            TranslateArg::Number(tag) => TranslateArg::Number(tag.clone()),
                            TranslateArg::Text(text) => TranslateArg::Text(
                                text.map(map)
                                    .map_err(|e| T::at(T::at_index(e, at), "with"))?,
                            ),
                        })
                    })
                    .collect::<Result<_, _>>()?,
            },
            TextContent::Keybind { keybind } => TextContent::Keybind {
                keybind: keybind.clone(),
            },
            TextContent::ScoreboardValue { score } => TextContent::ScoreboardValue {
                score: score.clone(),
            },
            TextContent::EntityNames {
                selector,
                separator,
            } => TextContent::EntityNames {
                selector: selector.clone(),
                separator: map_text(separator, map, "separator")?,
            },
            TextContent::Nbt {
                nbt,
                interpret,
                plain,
                separator,
                source,
            } => TextContent::Nbt {
                nbt: nbt.clone(),
                interpret: *interpret,
                plain: *plain,
                separator: map_text(separator, map, "separator")?,
                source: source.clone(),
            },
            TextContent::Object { object, fallback } => TextContent::Object {
                object: object.clone(),
                fallback: map_text(fallback, map, "fallback")?,
            },
        })
    }
}

impl<I: TextTypes> Default for TextContent<I> {
    fn default() -> Self {
        Self::Text { text: "".into() }
    }
}
