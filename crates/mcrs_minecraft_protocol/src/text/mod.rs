use std::borrow::Cow;
use std::io::Write;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;
use std::{fmt, ops};

use mcrs_minecraft_core::codec::int_value;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::{from_tag, nbt_flag};
use serde::de::{IntoDeserializer, Visitor};
use serde::{Deserialize, Deserializer, Serialize, de};
use uuid::Uuid;

use crate::item::Template;
use crate::item::component::Profile;
use crate::item::component::common::{ArgbInt, DialogReg, EntityTypeReg, IntArray, lenient};
use crate::item::ctx::{decode_nbt_wire, encode_nbt_wire};
use crate::{Decode, Encode};

pub mod color;
mod into_text;
#[cfg(test)]
mod tests;

pub use color::Color;
pub use into_text::IntoText;

/// Represents formatted text in Minecraft's JSON text format.
///
/// Text is used in various places such as chat, window titles,
/// disconnect messages, written books, signs, and more.
///
/// For more information, see the relevant [Minecraft Wiki article].
///
/// [Minecraft Wiki article]: https://minecraft.fandom.com/wiki/Raw_JSON_text_format
///
/// # Examples
///
/// With [`IntoText`] in scope, you can write the following:
/// ```
/// use mcrs_minecraft_protocol::text::{Color, IntoText, Text};
///
/// let txt = "The text is ".into_text()
///     + "Red".color(Color::RED)
///     + ", "
///     + "Green".color(Color::GREEN)
///     + ", and also "
///     + "Blue".color(Color::BLUE)
///     + "! And maybe even "
///     + "Italic".italic()
///     + ".";
///
/// assert_eq!(
///     txt.to_string(),
///     r#"{"text":"The text is ","extra":[{"text":"Red","color":"red"},", ",{"text":"Green","color":"green"},", and also ",{"text":"Blue","color":"blue"},"! And maybe even ",{"text":"Italic","italic":true},"."]}"#
/// );
/// ```
#[derive(Clone, PartialEq, Default)]
pub struct Text(Box<TextInner>);

/// A plain literal with no style and no siblings is written as a bare string.
impl Serialize for Text {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self.0.collapse_to_string() {
            Some(text) => s.serialize_str(text),
            None => self.0.serialize(s),
        }
    }
}

/// NBT stores a boolean as a byte, and serde's buffered `untagged` and
/// `flatten` paths lose the deserializer's own coercion, so accept both.
pub fn optional_flag<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<bool>, D::Error> {
    struct FlagVisitor;

    impl<'de> Visitor<'de> for FlagVisitor {
        type Value = Option<bool>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            write!(formatter, "a boolean or the byte NBT stores one as")
        }

        fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
            Ok(Some(v))
        }

        fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
            Ok(Some(v != 0))
        }

        fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
            Ok(Some(v != 0))
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D: Deserializer<'de>>(
            self,
            deserializer: D,
        ) -> Result<Self::Value, D::Error> {
            deserializer.deserialize_any(FlagVisitor)
        }
    }

    deserializer.deserialize_any(FlagVisitor)
}

/// Text data and formatting.
#[derive(Clone, PartialEq, Default, Debug, Serialize)]
pub struct TextInner {
    #[serde(flatten)]
    pub content: TextContent,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<Text>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow_color: Option<ArgbInt>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub underlined: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub strikethrough: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub obfuscated: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub click_event: Option<ClickEvent>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub hover_event: Option<HoverEvent>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub insertion: Option<Cow<'static, str>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<ResourceLocation>,
}

/// Style keys are read as they arrive; every other key is kept as the tag it
/// came as and handed to the content codec once the map ends. Serde's own
/// `flatten` buffer would do the same through `Content`, which has no array
/// tags, so an int array below `with` or a hover item would come back a list.
/// `with` is read directly too: a JSON `true` argument is a boolean, which
/// an NBT buffer would turn into a byte.
impl<'de> Deserialize<'de> for TextInner {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Flag(Option<bool>);

        impl<'de> Deserialize<'de> for Flag {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                optional_flag(d).map(Flag)
            }
        }

        struct Siblings(Vec<Text>);

        impl<'de> Deserialize<'de> for Siblings {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                non_empty(d).map(Siblings)
            }
        }

        struct InnerVisitor;

        impl<'de> Visitor<'de> for InnerVisitor {
            type Value = TextInner;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a text component")
            }

            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut inner = TextInner::default();
                let mut content = NbtCompound::new();
                let mut with: Option<Vec<TranslateArg>> = None;
                let mut seen: Vec<&'static str> = Vec::new();
                macro_rules! style {
                    ($key:expr, $field:ident, $ty:ty, $get:expr) => {{
                        if seen.contains(&$key) {
                            return Err(de::Error::duplicate_field($key));
                        }
                        seen.push($key);
                        let value: $ty = map.next_value()?;
                        inner.$field = $get(value);
                    }};
                }
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "extra" => style!("extra", extra, Siblings, |v: Siblings| v.0),
                        "color" => style!("color", color, Color, Some),
                        "shadow_color" => style!("shadow_color", shadow_color, ArgbInt, Some),
                        "bold" => style!("bold", bold, Flag, |v: Flag| v.0),
                        "italic" => style!("italic", italic, Flag, |v: Flag| v.0),
                        "underlined" => style!("underlined", underlined, Flag, |v: Flag| v.0),
                        "strikethrough" => {
                            style!("strikethrough", strikethrough, Flag, |v: Flag| v.0)
                        }
                        "obfuscated" => style!("obfuscated", obfuscated, Flag, |v: Flag| v.0),
                        "click_event" => style!("click_event", click_event, ClickEvent, Some),
                        "hover_event" => style!("hover_event", hover_event, HoverEvent, Some),
                        "insertion" => style!("insertion", insertion, String, |v: String| Some(
                            Cow::Owned(v)
                        )),
                        "font" => style!("font", font, ResourceLocation, Some),
                        "with" => {
                            if with.is_some() {
                                return Err(de::Error::duplicate_field("with"));
                            }
                            with = Some(map.next_value()?);
                        }
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

        d.deserialize_map(InnerVisitor)
    }
}

impl TextInner {
    fn collapse_to_string(&self) -> Option<&str> {
        match &self.content {
            TextContent::Text { text, .. }
                if self.extra.is_empty()
                    && self.color.is_none()
                    && self.shadow_color.is_none()
                    && self.bold.is_none()
                    && self.italic.is_none()
                    && self.underlined.is_none()
                    && self.strikethrough.is_none()
                    && self.obfuscated.is_none()
                    && self.click_event.is_none()
                    && self.hover_event.is_none()
                    && self.insertion.is_none()
                    && self.font.is_none() =>
            {
                Some(text)
            }
            _ => None,
        }
    }
}

/// The discriminator a variant writes under `source` / `object`: a marker
/// that rejects any other name, so the derived first-match implements
/// `StrictEither` for the untagged enums below.
macro_rules! discriminators {
    ($($name:ident = $id:literal),* $(,)?) => {$(
        fn $name<'de, D: Deserializer<'de>>(d: D) -> Result<(), D::Error> {
            let id = String::deserialize(d)?;
            if id != $id {
                return Err(de::Error::custom(format_args!("Unknown element id: {id}")));
            }
            Ok(())
        }
    )*};
}

discriminators! {
    source_entity = "entity",
    source_block = "block",
    source_storage = "storage",
    object_atlas = "atlas",
    object_player = "player",
}

/// A translation argument: a primitive travels as itself, anything else is a
/// component, and a component that is a plain string is read as the string.
#[derive(Clone, PartialEq, Debug)]
pub enum TranslateArg {
    Bool(bool),
    Number(NbtTag),
    Text(Text),
}

impl From<Text> for TranslateArg {
    fn from(text: Text) -> Self {
        TranslateArg::Text(text)
    }
}

impl Serialize for TranslateArg {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            TranslateArg::Bool(v) => s.serialize_bool(*v),
            TranslateArg::Number(tag) => Serialize::serialize(tag, s),
            TranslateArg::Text(text) => text.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for TranslateArg {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct ArgVisitor;

        fn number<'de, E: de::Error>(
            d: impl Deserializer<'de, Error = E>,
        ) -> Result<TranslateArg, E> {
            <NbtTag as Deserialize>::deserialize(d).map(TranslateArg::Number)
        }

        impl<'de> Visitor<'de> for ArgVisitor {
            type Value = TranslateArg;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a number, a boolean, a string or a text component")
            }

            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
                Ok(TranslateArg::Bool(v))
            }

            fn visit_i8<E: de::Error>(self, v: i8) -> Result<Self::Value, E> {
                Ok(TranslateArg::Number(NbtTag::Byte(v)))
            }

            fn visit_i16<E: de::Error>(self, v: i16) -> Result<Self::Value, E> {
                Ok(TranslateArg::Number(NbtTag::Short(v)))
            }

            fn visit_i32<E: de::Error>(self, v: i32) -> Result<Self::Value, E> {
                Ok(TranslateArg::Number(NbtTag::Int(v)))
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                number(v.into_deserializer())
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                number(v.into_deserializer())
            }

            fn visit_f32<E: de::Error>(self, v: f32) -> Result<Self::Value, E> {
                Ok(TranslateArg::Number(NbtTag::Float(v)))
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
                number(v.into_deserializer())
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(TranslateArg::Text(Text::text(v.to_owned())))
            }

            fn visit_seq<A: de::SeqAccess<'de>>(self, seq: A) -> Result<Self::Value, A::Error> {
                Text::deserialize(de::value::SeqAccessDeserializer::new(seq))
                    .map(TranslateArg::Text)
            }

            fn visit_map<A: de::MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                Text::deserialize(de::value::MapAccessDeserializer::new(map))
                    .map(TranslateArg::Text)
            }
        }

        d.deserialize_any(ArgVisitor)
    }
}

fn non_empty<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Vec<T>, D::Error> {
    let list = Vec::deserialize(d)?;
    if list.is_empty() {
        return Err(de::Error::custom("List must have contents"));
    }
    Ok(list)
}

/// The text content of a Text object.
#[derive(Clone, PartialEq, Debug, Serialize)]
#[serde(untagged)]
pub enum TextContent {
    /// Normal text
    Text {
        #[serde(skip)]
        typed: (),
        text: Cow<'static, str>,
    },
    /// A piece of text that will be translated on the client based on the
    /// client language. If no corresponding translation can be found, the
    /// identifier itself is used as the translated text.
    Translate {
        #[serde(skip)]
        typed: (),
        /// A translation identifier, corresponding to the identifiers found in
        /// loaded language files.
        translate: Cow<'static, str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        fallback: Option<Cow<'static, str>>,
        /// Optional list of text components to be inserted into slots in the
        /// translation text. Ignored if `translate` is not present.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        with: Vec<TranslateArg>,
    },
    /// Displays the name of the button that is currently bound to a certain
    /// configurable control on the client.
    Keybind {
        #[serde(skip)]
        typed: (),
        /// A [`keybind identifier`], to be displayed as the name of the button
        /// that is currently bound to that action.
        ///
        /// [`keybind identifier`]: https://minecraft.fandom.com/wiki/Controls#Configurable_controls
        keybind: Cow<'static, str>,
    },
    /// Displays a score holder's current score in an objective.
    ScoreboardValue {
        #[serde(skip)]
        typed: (),
        score: ScoreboardValueContent,
    },
    /// Displays the name of one or more entities found by a [`selector`].
    ///
    /// [`selector`]: https://minecraft.fandom.com/wiki/Target_selectors
    EntityNames {
        #[serde(skip)]
        typed: (),
        /// A string containing a [`selector`].
        ///
        /// [`selector`]: https://minecraft.fandom.com/wiki/Target_selectors
        selector: Cow<'static, str>,
        /// An optional custom separator used when the selector returns multiple
        /// entities. Defaults to the ", " text with gray color.
        #[serde(skip_serializing_if = "Option::is_none")]
        separator: Option<Text>,
    },
    /// Displays NBT values read from a block entity, an entity or command
    /// storage.
    Nbt {
        #[serde(skip)]
        typed: (),
        nbt: Cow<'static, str>,
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        interpret: bool,
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        plain: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        separator: Option<Text>,
        #[serde(flatten)]
        source: DataSource,
    },
    /// Displays a sprite in place of text.
    Object {
        #[serde(skip)]
        typed: (),
        #[serde(flatten)]
        object: ObjectInfo,
        #[serde(skip_serializing_if = "Option::is_none")]
        fallback: Option<Text>,
    },
}

/// A present `type` admits only the codec it names, otherwise the first codec
/// that accepts the map wins.
impl<'de> Deserialize<'de> for TextContent {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct TextRepr {
            text: String,
        }

        #[derive(Deserialize)]
        struct TranslateRepr {
            translate: String,
            #[serde(default, deserialize_with = "lenient")]
            fallback: Option<String>,
            #[serde(default)]
            with: Vec<TranslateArg>,
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
        struct SelectorRepr {
            selector: String,
            #[serde(default)]
            separator: Option<Text>,
        }

        #[derive(Deserialize)]
        struct NbtRepr {
            nbt: String,
            #[serde(default, deserialize_with = "lenient")]
            interpret: bool,
            #[serde(default, deserialize_with = "lenient")]
            plain: bool,
            #[serde(default, deserialize_with = "lenient")]
            separator: Option<Text>,
            #[serde(flatten)]
            source: DataSource,
        }

        #[derive(Deserialize)]
        struct ObjectRepr {
            #[serde(flatten)]
            object: ObjectInfo,
            #[serde(default)]
            fallback: Option<Text>,
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

        fn read<E: de::Error>(name: &str, compound: &NbtCompound) -> Result<TextContent, E> {
            let tag = || NbtTag::Compound(compound.clone());
            let content = match name {
                "text" => {
                    let TextRepr { text } = from_tag(tag()).map_err(E::custom)?;
                    TextContent::Text {
                        typed: (),
                        text: text.into(),
                    }
                }
                "translatable" => {
                    let TranslateRepr {
                        translate,
                        fallback,
                        with,
                    } = from_tag(tag()).map_err(E::custom)?;
                    TextContent::Translate {
                        typed: (),
                        translate: translate.into(),
                        fallback: fallback.map(Cow::Owned),
                        with,
                    }
                }
                "keybind" => {
                    let KeybindRepr { keybind } = from_tag(tag()).map_err(E::custom)?;
                    TextContent::Keybind {
                        typed: (),
                        keybind: keybind.into(),
                    }
                }
                "score" => {
                    let ScoreRepr { score } = from_tag(tag()).map_err(E::custom)?;
                    TextContent::ScoreboardValue { typed: (), score }
                }
                "selector" => {
                    let SelectorRepr {
                        selector,
                        separator,
                    } = from_tag(tag()).map_err(E::custom)?;
                    TextContent::EntityNames {
                        typed: (),
                        selector: selector.into(),
                        separator,
                    }
                }
                "nbt" => {
                    let NbtRepr {
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
                        typed: (),
                        nbt: nbt.into(),
                        interpret,
                        plain,
                        separator,
                        source,
                    }
                }
                "object" => {
                    let ObjectRepr { object, fallback } = from_tag(tag()).map_err(E::custom)?;
                    TextContent::Object {
                        typed: (),
                        object,
                        fallback,
                    }
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
                .find_map(|name| read::<D::Error>(name, &compound).ok())
                .ok_or_else(|| {
                    de::Error::custom("data did not match any variant of untagged enum TextContent")
                }),
        }
    }
}

/// Where an [`TextContent::Nbt`] component reads its data from.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DataSource {
    Entity {
        #[serde(
            rename = "source",
            default,
            deserialize_with = "source_entity",
            skip_serializing
        )]
        typed: (),
        entity: Cow<'static, str>,
    },
    Block {
        #[serde(
            rename = "source",
            default,
            deserialize_with = "source_block",
            skip_serializing
        )]
        typed: (),
        block: Cow<'static, str>,
    },
    Storage {
        #[serde(
            rename = "source",
            default,
            deserialize_with = "source_storage",
            skip_serializing
        )]
        typed: (),
        storage: ResourceLocation,
    },
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ObjectInfo {
    Atlas {
        #[serde(
            rename = "object",
            default,
            deserialize_with = "object_atlas",
            skip_serializing
        )]
        typed: (),
        #[serde(default = "default_atlas", skip_serializing_if = "is_default_atlas")]
        atlas: ResourceLocation,
        sprite: ResourceLocation,
    },
    Player {
        #[serde(
            rename = "object",
            default,
            deserialize_with = "object_player",
            skip_serializing
        )]
        typed: (),
        player: Profile,
        #[serde(
            default = "default_hat",
            deserialize_with = "nbt_flag",
            skip_serializing_if = "Clone::clone"
        )]
        hat: bool,
    },
}

fn default_atlas() -> ResourceLocation {
    ResourceLocation::minecraft("blocks")
}

fn is_default_atlas(atlas: &ResourceLocation) -> bool {
    *atlas == default_atlas()
}

fn default_hat() -> bool {
    true
}

/// Scoreboard value.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ScoreboardValueContent {
    /// The name of the score holder whose score should be displayed. This
    /// can be a [`selector`] or an explicit name.
    ///
    /// [`selector`]: https://minecraft.fandom.com/wiki/Target_selectors
    pub name: Cow<'static, str>,
    /// The internal name of the objective to display the player's score in.
    pub objective: Cow<'static, str>,
}

/// Action to take on click of the text.
#[derive(Clone, PartialEq, Debug, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ClickEvent {
    OpenUrl {
        url: Cow<'static, str>,
    },
    /// Only the client may create one; vanilla refuses to encode or decode it.
    #[serde(skip)]
    OpenFile {
        path: Cow<'static, str>,
    },
    RunCommand {
        command: Cow<'static, str>,
    },
    SuggestCommand {
        command: Cow<'static, str>,
    },
    ShowDialog {
        dialog: DialogRef,
    },
    ChangePage {
        page: i32,
    },
    CopyToClipboard {
        value: Cow<'static, str>,
    },
    Custom {
        id: ResourceLocation,
        #[serde(skip_serializing_if = "Option::is_none")]
        payload: Option<NbtTag>,
    },
}

/// A registry id, or the dialog written inline.
// ponytail: an inline dialog is carried as its compound and not validated; give it the typed
// dialog codecs once they live below the protocol crate.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DialogRef {
    Reference(ResourceKey<DialogReg>),
    Inline(NbtCompound),
}

/// A tagged map read without serde's `Content` buffer: the compound keeps
/// every NBT tag as it came, and the variant is picked from `action`.
fn tagged_compound<'de, D: Deserializer<'de>>(
    d: D,
    tag: &str,
) -> Result<(String, NbtCompound), D::Error> {
    let mut compound = NbtCompound::deserialize(d)?;
    let Some(index) = compound.child_tags.iter().position(|(key, _)| key == tag) else {
        return Err(de::Error::missing_field("action"));
    };
    match compound.child_tags.remove(index).1 {
        NbtTag::String(name) => Ok((name, compound)),
        _ => Err(de::Error::custom(format_args!("'{tag}' is not a string"))),
    }
}

fn variant<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    compound: NbtCompound,
) -> Result<T, D::Error> {
    from_tag(NbtTag::Compound(compound)).map_err(de::Error::custom)
}

impl<'de> Deserialize<'de> for ClickEvent {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Url {
            #[serde(deserialize_with = "untrusted_uri")]
            url: String,
        }

        #[derive(Deserialize)]
        struct Command {
            #[serde(deserialize_with = "chat_string")]
            command: String,
        }

        #[derive(Deserialize)]
        struct Dialog {
            dialog: DialogRef,
        }

        #[derive(Deserialize)]
        struct Page {
            #[serde(deserialize_with = "positive_int")]
            page: i32,
        }

        #[derive(Deserialize)]
        struct Value {
            value: String,
        }

        #[derive(Deserialize)]
        struct Custom {
            id: ResourceLocation,
            #[serde(default)]
            payload: Option<NbtTag>,
        }

        let (action, compound) = tagged_compound(d, "action")?;
        Ok(match action.as_str() {
            "open_url" => ClickEvent::OpenUrl {
                url: variant::<D, Url>(compound)?.url.into(),
            },
            "run_command" => ClickEvent::RunCommand {
                command: variant::<D, Command>(compound)?.command.into(),
            },
            "suggest_command" => ClickEvent::SuggestCommand {
                command: variant::<D, Command>(compound)?.command.into(),
            },
            "show_dialog" => ClickEvent::ShowDialog {
                dialog: variant::<D, Dialog>(compound)?.dialog,
            },
            "change_page" => ClickEvent::ChangePage {
                page: variant::<D, Page>(compound)?.page,
            },
            "copy_to_clipboard" => ClickEvent::CopyToClipboard {
                value: variant::<D, Value>(compound)?.value.into(),
            },
            "custom" => {
                let Custom { id, payload } = variant::<D, Custom>(compound)?;
                ClickEvent::Custom { id, payload }
            }
            other => {
                return Err(de::Error::unknown_variant(
                    other,
                    &[
                        "open_url",
                        "run_command",
                        "suggest_command",
                        "show_dialog",
                        "change_page",
                        "copy_to_clipboard",
                        "custom",
                    ],
                ));
            }
        })
    }
}

/// Any number is accepted and truncated to an int before the bound check.
fn positive_int<'de, D: Deserializer<'de>>(d: D) -> Result<i32, D::Error> {
    let value = int_value(d)?;
    if value < 1 {
        return Err(de::Error::custom(format_args!(
            "Value must be positive: {value}"
        )));
    }
    Ok(value)
}

fn chat_string<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let value = String::deserialize(d)?;
    if let Some(c) = value.chars().find(|&c| c == '§' || c < ' ' || c == '\x7f') {
        return Err(de::Error::custom(format_args!(
            "Disallowed chat character: '{c}'"
        )));
    }
    Ok(value)
}

/// A URI whose scheme is http or https, kept as written.
fn untrusted_uri<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let value = String::deserialize(d)?;
    let scheme = java_uri::scheme(&value).map_err(de::Error::custom)?;
    let Some(scheme) = scheme else {
        return Err(de::Error::custom(format_args!(
            "Missing protocol in URI: {value}: {value}"
        )));
    };
    if !matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https") {
        return Err(de::Error::custom(format_args!(
            "Unsupported protocol in URI: {value}: {value}"
        )));
    }
    Ok(value)
}

/// The `java.net.URI` parser (RFC 2396 with the RFC 2732 brackets), as far
/// as deciding whether a string is a URI and what its scheme is.
mod java_uri {
    fn unreserved(c: char) -> bool {
        c.is_ascii_alphanumeric() || "-_.!~*'()".contains(c)
    }

    fn uric(c: char) -> bool {
        unreserved(c) || ";/?:@&=+$,[]".contains(c)
    }

    fn pchar(c: char) -> bool {
        unreserved(c) || ":@&=+$,".contains(c)
    }

    fn path(c: char) -> bool {
        pchar(c) || c == '/' || c == ';'
    }

    fn authority(c: char) -> bool {
        unreserved(c) || "$,;:@&=+[]".contains(c)
    }

    fn other(c: char) -> bool {
        (c as u32) > 128 && !c.is_whitespace() && !c.is_control()
    }

    struct Parser<'a> {
        input: &'a str,
        chars: Vec<char>,
    }

    impl Parser<'_> {
        fn fail(&self, reason: &str, at: usize) -> String {
            format!("{reason} at index {at}: {}", self.input)
        }

        fn until(&self, mut p: usize, stop: &str) -> usize {
            while p < self.chars.len() && !stop.contains(self.chars[p]) {
                p += 1;
            }
            p
        }

        fn check(
            &self,
            start: usize,
            end: usize,
            allowed: fn(char) -> bool,
            what: &str,
        ) -> Result<(), String> {
            let mut p = start;
            while p < end {
                let c = self.chars[p];
                if allowed(c) || other(c) {
                    p += 1;
                } else if c == '%' {
                    let hex = |i: usize| self.chars.get(i).is_some_and(char::is_ascii_hexdigit);
                    if p + 2 >= end || !hex(p + 1) || !hex(p + 2) {
                        return Err(self.fail("Malformed escape pair", p));
                    }
                    p += 3;
                } else {
                    return Err(self.fail(&format!("Illegal character in {what}"), p));
                }
            }
            Ok(())
        }

        fn hierarchical(&self, mut p: usize) -> Result<usize, String> {
            let n = self.chars.len();
            if self.chars.get(p) == Some(&'/') && self.chars.get(p + 1) == Some(&'/') {
                p += 2;
                let q = self.until(p, "/?#");
                if q > p {
                    self.check(p, q, authority, "authority")?;
                    p = q;
                } else if q >= n {
                    return Err(self.fail("Expected authority", p));
                }
            }
            let q = self.until(p, "?#");
            self.check(p, q, path, "path")?;
            p = q;
            if self.chars.get(p) == Some(&'?') {
                p += 1;
                let q = self.until(p, "#");
                self.check(p, q, uric, "query")?;
                p = q;
            }
            Ok(p)
        }
    }

    pub(super) fn scheme(input: &str) -> Result<Option<String>, String> {
        let parser = Parser {
            input,
            chars: input.chars().collect(),
        };
        let chars = &parser.chars;
        let n = chars.len();
        let colon = parser.until(0, ":/?#");
        let (scheme, mut p) = if chars.get(colon) == Some(&':') {
            if colon == 0 {
                return Err(parser.fail("Expected scheme name", 0));
            }
            if !chars[0].is_ascii_alphabetic() {
                return Err(parser.fail("Illegal character in scheme name", 0));
            }
            if let Some(bad) = (1..colon)
                .find(|&i| !(chars[i].is_ascii_alphanumeric() || "+-.".contains(chars[i])))
            {
                return Err(parser.fail("Illegal character in scheme name", bad));
            }
            let p = colon + 1;
            let end = if chars.get(p) == Some(&'/') {
                parser.hierarchical(p)?
            } else {
                let q = parser.until(p, "#");
                if q <= p {
                    return Err(parser.fail("Expected scheme-specific part", p));
                }
                parser.check(p, q, uric, "opaque part")?;
                q
            };
            (Some(chars[..colon].iter().collect()), end)
        } else {
            (None, parser.hierarchical(0)?)
        };
        if chars.get(p) == Some(&'#') {
            parser.check(p + 1, n, uric, "fragment")?;
            p = n;
        }
        if p < n {
            return Err(parser.fail("end of URI", p));
        }
        Ok(scheme)
    }
}

/// Action to take when mouse-hovering on the text.
#[derive(Clone, PartialEq, Debug, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
#[allow(clippy::enum_variant_names)]
pub enum HoverEvent {
    ShowText {
        value: Text,
    },
    ShowItem(Box<Template>),
    ShowEntity {
        id: ResourceKey<EntityTypeReg>,
        #[serde(with = "lenient_uuid")]
        uuid: Uuid,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<Text>,
    },
}

impl<'de> Deserialize<'de> for HoverEvent {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct ShowText {
            value: Text,
        }

        #[derive(Deserialize)]
        struct ShowEntity {
            id: ResourceKey<EntityTypeReg>,
            #[serde(deserialize_with = "lenient_uuid::deserialize")]
            uuid: Uuid,
            #[serde(default)]
            name: Option<Text>,
        }

        let (action, compound) = tagged_compound(d, "action")?;
        Ok(match action.as_str() {
            "show_text" => HoverEvent::ShowText {
                value: variant::<D, ShowText>(compound)?.value,
            },
            "show_item" => HoverEvent::ShowItem(Box::new(variant::<D, Template>(compound)?)),
            "show_entity" => {
                let ShowEntity { id, uuid, name } = variant::<D, ShowEntity>(compound)?;
                HoverEvent::ShowEntity { id, uuid, name }
            }
            other => {
                return Err(de::Error::unknown_variant(
                    other,
                    &["show_text", "show_item", "show_entity"],
                ));
            }
        })
    }
}

/// Four ints, or on read the hyphenated string.
mod lenient_uuid {
    use super::*;

    pub(super) fn serialize<S: serde::Serializer>(uuid: &Uuid, s: S) -> Result<S::Ok, S::Error> {
        let v = uuid.as_u128();
        IntArray([
            (v >> 96) as i32,
            (v >> 64) as i32,
            (v >> 32) as i32,
            v as i32,
        ])
        .serialize(s)
    }

    /// Five hex groups of any length, each masked to its width.
    fn from_string(v: &str) -> Result<Uuid, String> {
        if v.len() > 36 {
            return Err("UUID string too large".to_owned());
        }
        let groups: Vec<&str> = v.split('-').collect();
        let [a, b, c, d, e] = groups[..] else {
            return Err(format!("Invalid UUID string: {v}"));
        };
        let hex = |group: &str| {
            i64::from_str_radix(group, 16).map_err(|_| format!("For input string: \"{group}\""))
        };
        let most =
            ((hex(a)? & 0xffff_ffff) << 32) | ((hex(b)? & 0xffff) << 16) | (hex(c)? & 0xffff);
        let least = ((hex(d)? & 0xffff) << 48) | (hex(e)? & 0xffff_ffff_ffff);
        Ok(Uuid::from_u64_pair(most as u64, least as u64))
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Uuid, D::Error> {
        struct UuidVisitor;

        impl<'de> Visitor<'de> for UuidVisitor {
            type Value = Uuid;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a uuid as four ints or a string")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Uuid, E> {
                from_string(v).map_err(|e| E::custom(format_args!("Invalid UUID {v}: {e}")))
            }

            fn visit_seq<A: de::SeqAccess<'de>>(self, seq: A) -> Result<Uuid, A::Error> {
                let IntArray([a, b, c, d]) =
                    IntArray::<4>::deserialize(de::value::SeqAccessDeserializer::new(seq))?;
                Ok(Uuid::from_u128(
                    (a as u32 as u128) << 96
                        | (b as u32 as u128) << 64
                        | (c as u32 as u128) << 32
                        | d as u32 as u128,
                ))
            }
        }

        d.deserialize_any(UuidVisitor)
    }
}

impl Encode for Text {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        encode_nbt_wire(self, w)
    }
}

impl Decode<'_> for Text {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        decode_nbt_wire(r)
    }
}

#[allow(clippy::self_named_constructors)]
impl Text {
    /// Constructs a new plain text object.
    pub fn text(plain: impl Into<Cow<'static, str>>) -> Self {
        Self(Box::new(TextInner {
            content: TextContent::Text {
                typed: (),
                text: plain.into(),
            },
            ..Default::default()
        }))
    }

    /// Create translated text based on the given translation key, with extra
    /// text components to be inserted into the slots of the translation text.
    pub fn translate(key: impl Into<Cow<'static, str>>, with: impl Into<Vec<Text>>) -> Self {
        Self(Box::new(TextInner {
            content: TextContent::Translate {
                typed: (),
                translate: key.into(),
                fallback: None,
                with: with.into().into_iter().map(TranslateArg::Text).collect(),
            },
            ..Default::default()
        }))
    }

    /// Create a score from the scoreboard.
    pub fn score(
        name: impl Into<Cow<'static, str>>,
        objective: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self(Box::new(TextInner {
            content: TextContent::ScoreboardValue {
                typed: (),
                score: ScoreboardValueContent {
                    name: name.into(),
                    objective: objective.into(),
                },
            },
            ..Default::default()
        }))
    }

    /// Creates a text component for selecting entity names with an optional
    /// custom separator.
    pub fn selector(selector: impl Into<Cow<'static, str>>, separator: Option<Text>) -> Self {
        Self(Box::new(TextInner {
            content: TextContent::EntityNames {
                typed: (),
                selector: selector.into(),
                separator,
            },
            ..Default::default()
        }))
    }

    /// Creates a text component for a keybind. The keybind should be a valid
    /// [`keybind identifier`].
    ///
    /// [`keybind identifier`]: https://minecraft.fandom.com/wiki/Controls#Configurable_controls
    pub fn keybind(keybind: impl Into<Cow<'static, str>>) -> Self {
        Self(Box::new(TextInner {
            content: TextContent::Keybind {
                typed: (),
                keybind: keybind.into(),
            },
            ..Default::default()
        }))
    }

    /// Creates a text component showing NBT read from `source`.
    pub fn nbt(
        source: DataSource,
        nbt: impl Into<Cow<'static, str>>,
        interpret: bool,
        separator: Option<Text>,
    ) -> Self {
        Self(Box::new(TextInner {
            content: TextContent::Nbt {
                typed: (),
                nbt: nbt.into(),
                interpret,
                plain: false,
                separator,
                source,
            },
            ..Default::default()
        }))
    }

    /// Returns `true` if the text contains no characters. Returns `false`
    /// otherwise.
    pub fn is_empty(&self) -> bool {
        for extra in &self.0.extra {
            if !extra.is_empty() {
                return false;
            }
        }

        match &self.0.content {
            TextContent::Text { text, .. } => text.is_empty(),
            TextContent::Translate { translate, .. } => translate.is_empty(),
            TextContent::ScoreboardValue { score, .. } => {
                let ScoreboardValueContent {
                    name, objective, ..
                } = score;

                name.is_empty() || objective.is_empty()
            }
            TextContent::EntityNames { selector, .. } => selector.is_empty(),
            TextContent::Keybind { keybind, .. } => keybind.is_empty(),
            TextContent::Nbt { nbt, .. } => nbt.is_empty(),
            TextContent::Object { .. } => false,
        }
    }

    /// Converts the [`Text`] object to a plain string with the [legacy formatting (`§` and format codes)](https://wiki.vg/Chat#Old_system)
    ///
    /// Removes everything that can't be represented with a `§` and a modifier.
    /// Any colors not on the [the legacy color list](https://wiki.vg/Chat#Colors) will be replaced with their closest equivalent.
    pub fn to_legacy_lossy(&self) -> String {
        // For keeping track of the currently active modifiers
        #[derive(Default, Clone)]
        struct Modifiers {
            obfuscated: Option<bool>,
            bold: Option<bool>,
            strikethrough: Option<bool>,
            underlined: Option<bool>,
            italic: Option<bool>,
            color: Option<Color>,
        }

        impl Modifiers {
            // Writes all active modifiers to a String as `§<mod>`
            fn write(&self, output: &mut String) {
                if let Some(color) = self.color {
                    let code = match color {
                        Color::Rgb(rgb) => rgb.to_named_lossy().hex_digit(),
                        Color::Named(normal) => normal.hex_digit(),
                    };

                    output.push('§');
                    output.push(code);
                }
                if let Some(true) = self.obfuscated {
                    output.push_str("§k");
                }
                if let Some(true) = self.bold {
                    output.push_str("§l");
                }
                if let Some(true) = self.strikethrough {
                    output.push_str("§m");
                }
                if let Some(true) = self.underlined {
                    output.push_str("§n");
                }
                if let Some(true) = self.italic {
                    output.push_str("§o");
                }
            }
            // Merges 2 Modifiers. The result is what you would get if you applied them both
            // sequentially.
            fn add(&self, other: &Self) -> Self {
                Self {
                    obfuscated: other.obfuscated.or(self.obfuscated),
                    bold: other.bold.or(self.bold),
                    strikethrough: other.strikethrough.or(self.strikethrough),
                    underlined: other.underlined.or(self.underlined),
                    italic: other.italic.or(self.italic),
                    color: other.color.or(self.color),
                }
            }
        }

        fn to_legacy_inner(this: &Text, result: &mut String, mods: &mut Modifiers) {
            let new_mods = Modifiers {
                obfuscated: this.0.obfuscated,
                bold: this.0.bold,
                strikethrough: this.0.strikethrough,
                underlined: this.0.underlined,
                italic: this.0.italic,
                color: this.0.color,
            };

            // If any modifiers were removed
            if [
                this.0.obfuscated,
                this.0.bold,
                this.0.strikethrough,
                this.0.underlined,
                this.0.italic,
            ]
            .contains(&Some(false))
            {
                // Reset and print sum of old and new modifiers
                result.push_str("§r");
                mods.add(&new_mods).write(result);
            } else {
                // Print only new modifiers
                new_mods.write(result);
            }

            *mods = mods.add(&new_mods);

            if let TextContent::Text { text, .. } = &this.0.content {
                result.push_str(text);
            }

            for child in &this.0.extra {
                to_legacy_inner(child, result, mods);
            }
        }

        let mut result = String::new();
        let mut mods = Modifiers::default();
        to_legacy_inner(self, &mut result, &mut mods);

        result
    }
}

impl Deref for Text {
    type Target = TextInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Text {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: IntoText<'static>> ops::Add<T> for Text {
    type Output = Self;

    fn add(self, rhs: T) -> Self::Output {
        self.add_child(rhs)
    }
}

impl<T: IntoText<'static>> ops::AddAssign<T> for Text {
    fn add_assign(&mut self, rhs: T) {
        self.extra.push(rhs.into_text());
    }
}

impl<'a> From<Text> for Cow<'a, Text> {
    fn from(value: Text) -> Self {
        Cow::Owned(value)
    }
}

impl<'a> From<&'a Text> for Cow<'a, Text> {
    fn from(value: &'a Text) -> Self {
        Cow::Borrowed(value)
    }
}

impl FromStr for Text {
    type Err = serde_json::error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            Ok(Text::default())
        } else {
            serde_json::from_str(s)
        }
    }
}

impl From<Text> for String {
    fn from(value: Text) -> Self {
        format!("{value}")
    }
}

impl fmt::Debug for Text {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for Text {
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

impl Default for TextContent {
    fn default() -> Self {
        Self::Text {
            typed: (),
            text: "".into(),
        }
    }
}

impl<'de> Deserialize<'de> for Text {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct TextVisitor;

        impl<'de> Visitor<'de> for TextVisitor {
            type Value = Text;

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

                while let Some(child) = seq.next_element::<Text>()? {
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

        deserializer.deserialize_any(TextVisitor)
    }
}

impl From<TextInner> for Text {
    fn from(inner: TextInner) -> Self {
        Self(Box::new(inner))
    }
}
