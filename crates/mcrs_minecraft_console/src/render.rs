use std::fmt::Write;

use mcrs_minecraft_lang::{Language, Piece};
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_text::{Text, TextContent, TextTypes, TranslateArg};

/// The plain text of `text`, with each translation filled in from `language`.
#[must_use]
pub(crate) fn render<I: TextTypes>(language: &Language, text: &Text<I>) -> String {
    let mut out = String::new();
    render_into(language, text, &mut out);
    out
}

fn render_into<I: TextTypes>(language: &Language, text: &Text<I>, out: &mut String) {
    match &text.content {
        TextContent::Text { text } => out.push_str(text),
        TextContent::Translate {
            translate,
            fallback,
            with,
        } => {
            let pattern = match fallback {
                Some(fallback) if !language.contains(translate) => fallback.as_ref(),
                _ => language.get(translate),
            };
            mcrs_minecraft_lang::fill_pattern(pattern, |piece| match piece {
                Piece::Literal(literal) => out.push_str(literal),
                Piece::Argument(index) => {
                    if let Some(arg) = with.get(index) {
                        render_arg(language, arg, out);
                    }
                }
            });
        }
        TextContent::Keybind { keybind } => out.push_str(keybind),
        TextContent::EntityNames { selector, .. } => out.push_str(selector),
        TextContent::Nbt { nbt, .. } => out.push_str(nbt),
        _ => {}
    }
    for child in &text.extra {
        render_into(language, child, out);
    }
}

fn render_arg<I: TextTypes>(language: &Language, arg: &TranslateArg<I>, out: &mut String) {
    match arg {
        TranslateArg::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
        TranslateArg::Number(number) => push_number(number, out),
        TranslateArg::Text(text) => render_into(language, text, out),
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
            render(&language, &sprint),
            "Sprint completed with 4000 ticks per second, or 0.25 ms per tick"
        );
        let swap = Text::<mcrs_minecraft_text::ItemHover>::translate_args(
            "swap",
            vec![Text::text("a").into(), Text::text("b").into()],
        );
        assert_eq!(render(&language, &swap), "b then a, 100%");
        let unknown = Text::<mcrs_minecraft_text::ItemHover>::translate_args(
            "commands.time.query.gametime",
            vec![12i64.into()],
        );
        assert_eq!(
            render(&language, &unknown),
            "commands.time.query.gametime",
            "a key with no pattern reads as itself"
        );
    }
}
