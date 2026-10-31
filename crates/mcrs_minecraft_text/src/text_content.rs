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

/// The text content of a Text object.
#[derive(Clone, PartialEq, Debug, Serialize)]
#[serde(untagged, bound(serialize = "I: TextSerde"))]
pub enum TextContent<I: TextTypes> {
    /// Normal text
    Text { text: Cow<'static, str> },
    /// A piece of text that will be translated on the client based on the
    /// client language. If no corresponding translation can be found, the
    /// identifier itself is used as the translated text.
    Translate {
        /// A translation identifier, corresponding to the identifiers found in
        /// loaded language files.
        translate: Cow<'static, str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        fallback: Option<Cow<'static, str>>,
        /// Optional list of text components to be inserted into slots in the
        /// translation text. Ignored if `translate` is not present.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        with: Vec<TranslateArg<I>>,
    },
    /// Displays the name of the button that is currently bound to a certain
    /// configurable control on the client.
    Keybind {
        /// A [`keybind identifier`], to be displayed as the name of the button
        /// that is currently bound to that action.
        ///
        /// [`keybind identifier`]: https://minecraft.fandom.com/wiki/Controls#Configurable_controls
        keybind: Cow<'static, str>,
    },
    /// Displays a score holder's current score in an objective.
    ScoreboardValue { score: ScoreboardValueContent },
    /// Displays the name of one or more entities found by a [`selector`].
    ///
    /// [`selector`]: https://minecraft.fandom.com/wiki/Target_selectors
    EntityNames {
        /// A string containing a [`selector`].
        ///
        /// [`selector`]: https://minecraft.fandom.com/wiki/Target_selectors
        selector: Cow<'static, str>,
        /// An optional custom separator used when the selector returns multiple
        /// entities. Defaults to the ", " text with gray color.
        #[serde(skip_serializing_if = "Option::is_none")]
        separator: Option<Text<I>>,
    },
    /// Displays NBT values read from a block entity, an entity or command
    /// storage.
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
    /// Displays a sprite in place of text.
    Object {
        #[serde(flatten)]
        object: ObjectInfo,
        #[serde(skip_serializing_if = "Option::is_none")]
        fallback: Option<Text<I>>,
    },
}

/// A present `type` admits only the codec it names, otherwise the first codec
/// that accepts the map wins.
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
