use mcrs_minecraft_text::{Text, TextTypes};

use crate::{Projection, TextHost, UnitBudget};

#[must_use]
pub fn plain_text<I: TextTypes>(host: &impl TextHost, text: &Text<I>) -> String {
    let mut out = String::new();
    let mut budget = UnitBudget::new();
    crate::flatten(host, text, Projection::Plain, |run, _| {
        out.push_str(budget.take(run));
    });
    out
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use mcrs_minecraft_lang::Language;
    use mcrs_minecraft_text::ItemHover;

    use super::*;
    use crate::MAX_UNITS;

    #[test]
    fn each_content_shows_its_text() {
        let language = Language::from_json(br#"{"left":"%s left the game"}"#).expect("valid JSON");
        let cases = [
            (
                r#"{"translate":"left","fallback":"unused","with":["Alex"]}"#,
                "Alex left the game",
            ),
            (
                r#"{"translate":"missing","fallback":"%s is gone","with":[true]}"#,
                "true is gone",
            ),
            (r#"{"keybind":"key.jump","extra":["!"]}"#, "key.jump!"),
        ];
        for (json, expected) in cases {
            let text: Text<ItemHover> = Text::from_str(json).expect(json);
            assert_eq!(plain_text(&language, &text), expected, "{json}");
        }
    }

    #[test]
    fn the_plain_string_stops_at_the_unit_bound() {
        let language = Language::from_json(br#"{"grow":"%1$s%1$s%1$s%1$s"}"#).expect("valid JSON");
        let mut text: Text<ItemHover> = Text::text("a".repeat(30_000));
        for _ in 0..3 {
            text = Text::translate_args("grow", vec![text.into()]);
        }
        assert_eq!(
            plain_text(&language, &text).encode_utf16().count(),
            MAX_UNITS
        );
    }

    #[test]
    fn a_translation_fills_its_arguments_in_order_and_by_position() {
        let language = Language::from_json(
            br#"{"sprint":"Sprint completed with %s ticks per second, or %s ms per tick","swap":"%2$s then %1$s, 100%%"}"#,
        )
        .expect("valid JSON");
        let cases: [(Text<ItemHover>, &str); 3] = [
            (
                Text::translate_args("sprint", vec![4000.into(), Text::text("0.25").into()]),
                "Sprint completed with 4000 ticks per second, or 0.25 ms per tick",
            ),
            (
                Text::translate_args("swap", vec![Text::text("a").into(), Text::text("b").into()]),
                "b then a, 100%",
            ),
            (
                Text::translate_args("commands.time.query.gametime", vec![12i64.into()]),
                "commands.time.query.gametime",
            ),
        ];
        for (text, expected) in cases {
            assert_eq!(plain_text(&language, &text), expected);
        }
    }
}
