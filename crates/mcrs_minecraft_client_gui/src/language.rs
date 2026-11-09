use bevy::prelude::Resource;

use crate::font::Span;
use mcrs_minecraft_lang::{EN_US, Language, Piece};
use mcrs_minecraft_model::resource_set::ResourceSet;

/// The language of the client: the English translations of the resource pack.
#[derive(Resource, Debug, Default)]
pub struct ClientLanguage(Language);

impl ClientLanguage {
    pub fn load(pack: &ResourceSet) -> Result<Self, String> {
        Language::from_json(pack.read(EN_US)?)
            .map(Self)
            .map_err(|error| format!("{EN_US}: {error}"))
    }

    /// Whether the language has a translation for `key`.
    pub fn contains(&self, key: &str) -> bool {
        self.0.contains(key)
    }

    /// An untranslated key reads as itself, as it does in vanilla.
    pub fn get<'a>(&'a self, key: &'a str) -> &'a str {
        self.0.get(key)
    }

    /// Fills `%s` and `%n$s` with the arguments' spans; the literal text between
    /// them takes `color`.
    pub fn translate(&self, key: &str, color: u32, args: &[Vec<Span>]) -> Vec<Span> {
        let mut spans = Vec::new();
        let mut literal = String::new();
        mcrs_minecraft_lang::fill_pattern(self.get(key), |piece| match piece {
            Piece::Literal(text) => literal.push_str(text),
            Piece::Argument(index) => {
                if !literal.is_empty() {
                    spans.push(Span::new(std::mem::take(&mut literal), color));
                }
                if let Some(arg) = args.get(index) {
                    spans.extend(arg.iter().cloned());
                }
            }
        });
        if !literal.is_empty() {
            spans.push(Span::new(literal, color));
        }
        spans
    }
}

#[cfg(test)]
impl ClientLanguage {
    pub(crate) fn corpus() -> &'static ClientLanguage {
        static LANGUAGE: std::sync::LazyLock<ClientLanguage> =
            std::sync::LazyLock::new(|| ClientLanguage::load(ResourceSet::corpus()).unwrap());
        &LANGUAGE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: u32 = 0xFFFF_FFFF;
    const AQUA: u32 = 0xFF55_FFFF;

    mod jar {
        use super::*;

        #[test]
        fn a_placeholder_takes_the_argument_spans() {
            let key = vec![Span::new("F4", AQUA)];
            assert_eq!(
                ClientLanguage::corpus().translate("debug.gamemodes.select_next", WHITE, &[key]),
                [Span::new("F4", AQUA), Span::new(" Next", WHITE)]
            );
        }
    }

    #[test]
    fn positional_and_escaped_placeholders() {
        let language = ClientLanguage(
            Language::from_json(br#"{"k":"%2$s then %1$s at 100%%"}"#).expect("valid JSON"),
        );
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
