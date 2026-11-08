use std::collections::HashMap;
use std::fmt::Write;

use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_text::{Text, TextContent, TextTypes, TranslateArg};

/// The path of the English language file in the client jar.
pub(crate) const EN_US: &str = "assets/minecraft/lang/en_us.json";

/// The translations of one language, by translation key.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Language(HashMap<String, String>);

impl Language {
    /// Reads a language file: one JSON object from key to pattern.
    pub fn from_json(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes).map(Self)
    }

    /// The pattern of `key`, or the key itself when the language has none.
    fn pattern<'a>(&'a self, key: &'a str) -> &'a str {
        self.0.get(key).map_or(key, String::as_str)
    }

    /// The plain text of `text`, with each translation filled in from this language.
    #[must_use]
    pub fn render<I: TextTypes>(&self, text: &Text<I>) -> String {
        let mut out = String::new();
        self.render_into(text, &mut out);
        out
    }

    fn render_into<I: TextTypes>(&self, text: &Text<I>, out: &mut String) {
        match &text.content {
            TextContent::Text { text } => out.push_str(text),
            TextContent::Translate {
                translate,
                fallback,
                with,
            } => {
                let pattern = match (self.0.get(translate.as_ref()), fallback) {
                    (Some(pattern), _) => pattern.as_str(),
                    (None, Some(fallback)) => fallback.as_ref(),
                    (None, None) => self.pattern(translate),
                };
                self.fill(pattern, with, out);
            }
            TextContent::Keybind { keybind } => out.push_str(keybind),
            TextContent::EntityNames { selector, .. } => out.push_str(selector),
            TextContent::Nbt { nbt, .. } => out.push_str(nbt),
            _ => {}
        }
        for child in &text.extra {
            self.render_into(child, out);
        }
    }

    /// Fills `%s`, `%n$s` and `%%` of `pattern` with `args`.
    fn fill<I: TextTypes>(&self, pattern: &str, args: &[TranslateArg<I>], out: &mut String) {
        let mut next = 0;
        let mut rest = pattern;
        while let Some(at) = rest.find('%') {
            out.push_str(&rest[..at]);
            rest = &rest[at + 1..];
            if let Some(after) = rest.strip_prefix('%') {
                out.push('%');
                rest = after;
                continue;
            }
            let index = if let Some(after) = rest.strip_prefix('s') {
                rest = after;
                next += 1;
                next - 1
            } else if let Some((digits, after)) = rest.split_once("$s")
                && let Ok(position) = digits.parse::<usize>()
            {
                rest = after;
                position.saturating_sub(1)
            } else {
                out.push('%');
                continue;
            };
            if let Some(arg) = args.get(index) {
                self.render_arg(arg, out);
            }
        }
        out.push_str(rest);
    }

    fn render_arg<I: TextTypes>(&self, arg: &TranslateArg<I>, out: &mut String) {
        match arg {
            TranslateArg::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
            TranslateArg::Number(number) => push_number(number, out),
            TranslateArg::Text(text) => self.render_into(text, out),
        }
    }
}

/// A number as the game writes a primitive argument: an integer in decimal, a float with at
/// least one digit after the point.
fn push_number(number: &NbtTag, out: &mut String) {
    let _ = match number {
        NbtTag::Byte(value) => write!(out, "{value}"),
        NbtTag::Short(value) => write!(out, "{value}"),
        NbtTag::Int(value) => write!(out, "{value}"),
        NbtTag::Long(value) => write!(out, "{value}"),
        NbtTag::Float(value) => write_float(f64::from(*value), out),
        NbtTag::Double(value) => write_float(*value, out),
        _ => Ok(()),
    };
}

fn write_float(value: f64, out: &mut String) -> std::fmt::Result {
    if value.is_finite() && value.fract() == 0.0 && value.abs() < 1.0e7 {
        write!(out, "{value:.1}")
    } else {
        write!(out, "{value}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_translation_fills_its_arguments_in_order_and_by_position() {
        let language = Language::from_json(
            br#"{"sprint":"Sprint completed with %s ticks per second, or %s ms per tick","swap":"%2$s then %1$s, 100%%"}"#,
        )
        .expect("valid JSON");
        let sprint = Text::<mcrs_minecraft_text::ItemHover>::translate_args(
            "sprint",
            vec![4000.into(), Text::text("0.25").into()],
        );
        assert_eq!(
            language.render(&sprint),
            "Sprint completed with 4000 ticks per second, or 0.25 ms per tick"
        );
        let swap = Text::<mcrs_minecraft_text::ItemHover>::translate_args(
            "swap",
            vec![Text::text("a").into(), Text::text("b").into()],
        );
        assert_eq!(language.render(&swap), "b then a, 100%");
        let unknown = Text::<mcrs_minecraft_text::ItemHover>::translate_args(
            "commands.time.query.gametime",
            vec![12i64.into()],
        );
        assert_eq!(
            language.render(&unknown),
            "commands.time.query.gametime",
            "a key with no pattern reads as itself"
        );
    }
}
