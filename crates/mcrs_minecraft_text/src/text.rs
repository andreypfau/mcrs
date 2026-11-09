use std::borrow::Cow;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;
use std::{fmt, ops};

use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, de};

use crate::{
    Color, DataSource, IntoText, ItemHover, ScoreboardValueContent, TextContent, TextInner,
    TextMap, TextSerde, TextTypes, TranslateArg,
};

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
/// use mcrs_minecraft_text::{Color, IntoText, Text};
///
/// let txt: Text = "The text is ".into_text()
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
#[derive(Clone, PartialEq)]
pub struct Text<I: TextTypes = ItemHover>(Box<TextInner<I>>);

impl<I: TextTypes> Default for Text<I> {
    fn default() -> Self {
        Text(Box::default())
    }
}

/// A plain literal with no style and no siblings is written as a bare string.
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
    /// Constructs a new plain text object.
    pub fn text(plain: impl Into<Cow<'static, str>>) -> Self {
        Self(Box::new(TextInner {
            content: TextContent::Text { text: plain.into() },
            ..Default::default()
        }))
    }

    /// Create translated text based on the given translation key, with extra
    /// text components to be inserted into the slots of the translation text.
    pub fn translate(key: impl Into<Cow<'static, str>>, with: impl Into<Vec<Text<I>>>) -> Self {
        let with: Vec<_> = with.into().into_iter().map(TranslateArg::Text).collect();
        Self::translate_args(key, with)
    }

    /// Translated text whose arguments may be primitives as well as components.
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

    /// Create a score from the scoreboard.
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

    /// Creates a text component for selecting entity names with an optional
    /// custom separator.
    pub fn selector(selector: impl Into<Cow<'static, str>>, separator: Option<Text<I>>) -> Self {
        Self(Box::new(TextInner {
            content: TextContent::EntityNames {
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

        fn to_legacy_inner<I: TextTypes>(
            this: &Text<I>,
            result: &mut String,
            mods: &mut Modifiers,
        ) {
            let new_mods = Modifiers {
                obfuscated: this.0.style.obfuscated,
                bold: this.0.style.bold,
                strikethrough: this.0.style.strikethrough,
                underlined: this.0.style.underlined,
                italic: this.0.style.italic,
                color: this.0.style.color,
            };

            // If any modifiers were removed
            if [
                this.0.style.obfuscated,
                this.0.style.bold,
                this.0.style.strikethrough,
                this.0.style.underlined,
                this.0.style.italic,
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

            match &this.0.content {
                TextContent::Text { text, .. } => {
                    result.push_str(text);
                }
                TextContent::Translate {
                    translate,
                    fallback,
                    with,
                } => {
                    let template = fallback.as_deref().unwrap_or(translate);
                    format_translate(template, with, result, mods);
                }
                TextContent::Keybind { keybind } => {
                    result.push_str(keybind);
                }
                TextContent::ScoreboardValue { score } => {
                    result.push_str(&score.name);
                }
                TextContent::EntityNames { selector, .. } => {
                    result.push_str(selector);
                }
                TextContent::Nbt { nbt, .. } => {
                    result.push_str(nbt);
                }
                TextContent::Object { fallback, .. } => {
                    if let Some(fallback) = fallback {
                        to_legacy_inner(fallback, result, mods);
                    }
                }
            }

            for child in &this.0.extra {
                to_legacy_inner(child, result, mods);
            }
        }

        fn render_arg<I: TextTypes>(
            arg: &TranslateArg<I>,
            result: &mut String,
            mods: &mut Modifiers,
        ) {
            match arg {
                TranslateArg::Text(text) => to_legacy_inner(text, result, mods),
                TranslateArg::Bool(b) => result.push_str(if *b { "true" } else { "false" }),
                TranslateArg::Number(tag) => match tag {
                    mcrs_minecraft_nbt::tag::NbtTag::Byte(v) => result.push_str(&v.to_string()),
                    mcrs_minecraft_nbt::tag::NbtTag::Short(v) => result.push_str(&v.to_string()),
                    mcrs_minecraft_nbt::tag::NbtTag::Int(v) => result.push_str(&v.to_string()),
                    mcrs_minecraft_nbt::tag::NbtTag::Long(v) => result.push_str(&v.to_string()),
                    mcrs_minecraft_nbt::tag::NbtTag::Float(v) => result.push_str(&v.to_string()),
                    mcrs_minecraft_nbt::tag::NbtTag::Double(v) => result.push_str(&v.to_string()),
                    _ => result.push_str(&format!("{tag:?}")),
                },
            }
        }

        fn format_translate<I: TextTypes>(
            template: &str,
            with: &[TranslateArg<I>],
            result: &mut String,
            mods: &mut Modifiers,
        ) {
            if with.is_empty() {
                result.push_str(template);
                return;
            }

            if !template.contains('%') {
                result.push_str(template);
                for (i, arg) in with.iter().enumerate() {
                    if i == 0 {
                        result.push_str(": ");
                    } else {
                        result.push_str(", ");
                    }
                    render_arg(arg, result, mods);
                }
                return;
            }

            let mut chars = template.chars().peekable();
            let mut auto_idx = 0;
            while let Some(c) = chars.next() {
                if c != '%' {
                    result.push(c);
                    continue;
                }

                match chars.peek() {
                    Some('%') => {
                        chars.next();
                        result.push('%');
                    }
                    Some('0'..='9') => {
                        let mut num_chars = Vec::new();
                        let mut is_positional = false;
                        while let Some(&d) = chars.peek() {
                            if d.is_ascii_digit() {
                                num_chars.push(d);
                                chars.next();
                            } else if d == '$' {
                                chars.next();
                                is_positional = true;
                                break;
                            } else {
                                break;
                            }
                        }
                        if is_positional {
                            let num_str: String = num_chars.into_iter().collect();
                            if let Ok(idx) = num_str.parse::<usize>() {
                                if let Some(_spec) = chars.next() {
                                    if idx >= 1 && idx <= with.len() {
                                        render_arg(&with[idx - 1], result, mods);
                                    }
                                }
                            }
                        } else {
                            result.push('%');
                            for ch in num_chars {
                                result.push(ch);
                            }
                        }
                    }
                    Some('s') | Some('d') | Some('f') => {
                        chars.next();
                        if auto_idx < with.len() {
                            render_arg(&with[auto_idx], result, mods);
                            auto_idx += 1;
                        }
                    }
                    _ => {
                        result.push('%');
                    }
                }
            }
        }

        let mut result = String::new();
        let mut mods = Modifiers::default();
        to_legacy_inner(self, &mut result, &mut mods);

        result
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Color;

    #[test]
    fn to_legacy_lossy_plain_text() {
        let text: Text = Text::text("Hello world");
        assert_eq!(text.to_legacy_lossy(), "Hello world");
    }

    #[test]
    fn to_legacy_lossy_translate_without_args() {
        let text: Text = Text::translate("multiplayer.disconnect.invalid_player_movement", Vec::new());
        assert_eq!(
            text.to_legacy_lossy(),
            "multiplayer.disconnect.invalid_player_movement"
        );
    }

    #[test]
    fn to_legacy_lossy_translate_with_fallback_and_placeholders() {
        let text: Text = Text(Box::new(TextInner {
            content: TextContent::Translate {
                translate: "chat.type.text".into(),
                fallback: Some("<%s> %s".into()),
                with: vec![
                    TranslateArg::Text(Text::text("Steve")),
                    TranslateArg::Text(Text::text("Hello!")),
                ],
            },
            ..Default::default()
        }));
        assert_eq!(text.to_legacy_lossy(), "<Steve> Hello!");
    }

    #[test]
    fn to_legacy_lossy_translate_with_positional_placeholders() {
        let text: Text = Text(Box::new(TextInner {
            content: TextContent::Translate {
                translate: "custom.ordered".into(),
                fallback: Some("%2$s before %1$s".into()),
                with: vec![
                    TranslateArg::Text(Text::text("first")),
                    TranslateArg::Text(Text::text("second")),
                ],
            },
            ..Default::default()
        }));
        assert_eq!(text.to_legacy_lossy(), "second before first");
    }

    #[test]
    fn to_legacy_lossy_translate_with_args_no_specifiers() {
        let text: Text = Text::translate_args(
            "multiplayer.disconnect.banned.reason",
            vec![TranslateArg::Text(Text::text("Griefing"))],
        );
        assert_eq!(
            text.to_legacy_lossy(),
            "multiplayer.disconnect.banned.reason: Griefing"
        );
    }

    #[test]
    fn to_legacy_lossy_keybind() {
        let text: Text = Text::keybind("key.jump");
        assert_eq!(text.to_legacy_lossy(), "key.jump");
    }

    #[test]
    fn to_legacy_lossy_formatted_args() {
        let mut child: Text = Text::text("Player").color(Color::RED);
        child.0.style.bold = Some(true);
        let text: Text = Text(Box::new(TextInner {
            content: TextContent::Translate {
                translate: "multiplayer.player.joined".into(),
                fallback: Some("%s joined the game".into()),
                with: vec![TranslateArg::Text(child)],
            },
            ..Default::default()
        }));
        assert_eq!(text.to_legacy_lossy(), "§c§lPlayer joined the game");
    }
}
