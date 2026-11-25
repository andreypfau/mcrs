use mcrs_minecraft_lang::Language;
use mcrs_minecraft_text::{Color, ResolvedStyle, Text, TextTypes};

#[must_use]
pub fn legacy_text<I: TextTypes>(language: &Language, text: &Text<I>) -> String {
    let mut out = String::new();
    let mut written = ResolvedStyle::default();
    crate::flatten(language, text, |run, style| {
        write_change(&mut out, written, style);
        written = style;
        out.push_str(run);
    });
    out
}

fn write_change<I: TextTypes>(out: &mut String, from: ResolvedStyle<I>, to: ResolvedStyle<I>) {
    let flags = |style: ResolvedStyle<I>| {
        [
            (style.obfuscated, 'k'),
            (style.bold, 'l'),
            (style.strikethrough, 'm'),
            (style.underlined, 'n'),
            (style.italic, 'o'),
        ]
    };
    let lost = flags(from)
        .iter()
        .zip(flags(to))
        .any(|(old, new)| old.0 && !new.0)
        || (from.color.is_some() && to.color.is_none());
    if lost {
        out.push_str("§r");
    }
    let cleared = lost || from.color != to.color;
    if cleared && let Some(color) = to.color {
        let named = match color {
            Color::Rgb(rgb) => rgb.to_named_lossy(),
            Color::Named(named) => named,
        };
        out.push('§');
        out.push(named.hex_digit());
    }
    for ((old, _), (new, code)) in flags(from).into_iter().zip(flags(to)) {
        if new && (cleared || !old) {
            out.push('§');
            out.push(code);
        }
    }
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_text::{IntoText, ItemHover};

    use super::*;

    #[test]
    fn codes_follow_the_inherited_style_of_each_run() {
        let siblings: Text<ItemHover> = "Heavily formatted green text\n"
            .bold()
            .italic()
            .strikethrough()
            .underlined()
            .obfuscated()
            .color(Color::GREEN)
            + "Lightly formatted red text\n"
                .not_bold()
                .not_strikethrough()
                .not_obfuscated()
                .color(Color::RED)
            + "Not formatted blue text"
                .not_italic()
                .not_underlined()
                .color(Color::BLUE);
        let joined: Text<ItemHover> = Text::translate_args(
            "multiplayer.player.joined",
            vec![Text::text("Steve").color(Color::RED).bold().into()],
        )
        .italic();
        let recoloured: Text<ItemHover> =
            Text::text("").bold() + "a".color(Color::RED) + "b".color(Color::rgb(0x55, 0x55, 0xFF));
        let language = Language::from_json(br#"{"multiplayer.player.joined":"%s joined"}"#)
            .expect("valid JSON");
        let cases = [
            (
                siblings,
                "§a§k§l§m§n§oHeavily formatted green text\n§r§c§n§oLightly formatted red \
                 text\n§r§9§k§l§mNot formatted blue text",
            ),
            (joined, "§c§l§oSteve§r§o joined"),
            (recoloured, "§c§la§9§lb"),
        ];
        for (text, expected) in cases {
            assert_eq!(legacy_text(&language, &text), expected);
        }
    }
}
