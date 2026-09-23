use std::collections::HashMap;

use bevy::prelude::Resource;

use super::font::Span;
use crate::model::Pack;

pub const EN_US: &str = "minecraft/lang/en_us.json";

#[derive(Resource, Debug, Default)]
pub struct Language(HashMap<String, String>);

impl Language {
    pub fn load(pack: &Pack) -> Result<Self, String> {
        serde_json::from_slice(pack.read(EN_US)?)
            .map(Self)
            .map_err(|error| format!("{EN_US}: {error}"))
    }

    /// An untranslated key reads as itself, as it does in vanilla.
    pub fn get<'a>(&'a self, key: &'a str) -> &'a str {
        self.0.get(key).map_or(key, String::as_str)
    }

    /// Fills `%s` and `%n$s` with the arguments' spans; the literal text between
    /// them takes `color`.
    pub fn translate(&self, key: &str, color: u32, args: &[Vec<Span>]) -> Vec<Span> {
        let pattern = self.get(key);
        let mut spans = Vec::new();
        let mut literal = String::new();
        let mut next_arg = 0;
        let mut rest = pattern;
        while let Some(at) = rest.find('%') {
            literal.push_str(&rest[..at]);
            rest = &rest[at + 1..];
            let (index, consumed) = if let Some(after) = rest.strip_prefix('s') {
                next_arg += 1;
                (Some(next_arg - 1), rest.len() - after.len())
            } else if let Some((digits, after)) = rest.split_once("$s")
                && let Ok(n) = digits.parse::<usize>()
            {
                (n.checked_sub(1), rest.len() - after.len())
            } else if let Some(after) = rest.strip_prefix('%') {
                literal.push('%');
                rest = after;
                continue;
            } else {
                literal.push('%');
                continue;
            };
            rest = &rest[consumed..];
            if !literal.is_empty() {
                spans.push(Span::new(std::mem::take(&mut literal), color));
            }
            if let Some(arg) = index.and_then(|index| args.get(index)) {
                spans.extend(arg.iter().cloned());
            }
        }
        literal.push_str(rest);
        if !literal.is_empty() {
            spans.push(Span::new(literal, color));
        }
        spans
    }
}

#[cfg(test)]
impl Language {
    pub(crate) fn corpus() -> &'static Language {
        static LANGUAGE: std::sync::LazyLock<Language> =
            std::sync::LazyLock::new(|| Language::load(Pack::corpus()).unwrap());
        &LANGUAGE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: u32 = 0xFFFF_FFFF;
    const AQUA: u32 = 0xFF55_FFFF;

    #[test]
    fn a_placeholder_takes_the_argument_spans() {
        let key = vec![Span::new("F4", AQUA)];
        assert_eq!(
            Language::corpus().translate("debug.gamemodes.select_next", WHITE, &[key]),
            [Span::new("F4", AQUA), Span::new(" Next", WHITE)]
        );
    }

    #[test]
    fn positional_and_escaped_placeholders() {
        let mut language = Language::default();
        language
            .0
            .insert("k".into(), "%2$s then %1$s at 100%%".into());
        assert_eq!(
            language.translate(
                "k",
                WHITE,
                &[vec![Span::new("a", AQUA)], vec![Span::new("b", AQUA)]]
            ),
            [
                Span::new("b", AQUA),
                Span::new(" then ", WHITE),
                Span::new("a", AQUA),
                Span::new(" at 100%", WHITE),
            ]
        );
        assert_eq!(language.get("missing.key"), "missing.key");
    }
}
