use mcrs_minecraft_lang::{FormatError, Piece, template_pieces, write_primitive_text};
use mcrs_minecraft_text::color::NamedColor;
use mcrs_minecraft_text::{
    Color, GlyphFont, ResolvedStyle, Style, Text, TextContent, TextTypes, TranslateArg,
};

use crate::projection::Projection;
use crate::text_host::TextHost;

const MAX_PIECES: usize = 65_536;
const MAX_NODES: usize = 262_144;
const OBJECT_GLYPH: &str = "\u{FFFC}";

pub fn flatten<'a, I: TextTypes>(
    host: &impl TextHost,
    text: &'a Text<I>,
    projection: Projection,
    mut emit: impl FnMut(&str, ResolvedStyle<'a, I>),
) {
    let mut walk = Walk {
        host,
        projection,
        emit: &mut emit,
        pieces_left: MAX_PIECES,
        nodes_left: MAX_NODES,
        scratch: String::new(),
    };
    walk.text(text, ResolvedStyle::default());
}

struct Walk<'h, H, F> {
    host: &'h H,
    projection: Projection,
    emit: &'h mut F,
    pieces_left: usize,
    nodes_left: usize,
    scratch: String,
}

impl<H: TextHost, F> Walk<'_, H, F> {
    fn text<'a, I: TextTypes>(&mut self, text: &'a Text<I>, inherited: ResolvedStyle<'a, I>)
    where
        F: FnMut(&str, ResolvedStyle<'a, I>),
    {
        if self.pieces_left == 0 {
            return;
        }
        let Some(nodes_left) = self.nodes_left.checked_sub(1) else {
            return;
        };
        self.nodes_left = nodes_left;
        let style = text.style.resolve(inherited);
        match &text.content {
            TextContent::Text { text } => self.piece(text, style),
            TextContent::Translate {
                translate,
                fallback,
                with,
            } => self.translate(translate, fallback.as_deref(), with, style),
            TextContent::Keybind { keybind } => {
                let host = self.host;
                match host.keybind_label(keybind) {
                    Some(label) => self.piece(label, style),
                    None if host.translates_keybind_name() => {
                        self.translate(keybind, None, &[], style)
                    }
                    None => self.piece(keybind, style),
                }
            }
            TextContent::EntityNames { selector, .. } => self.piece(selector, style),
            TextContent::Nbt { .. } | TextContent::ScoreboardValue { .. } => {}
            TextContent::Object { object, fallback } => match self.projection {
                Projection::Drawn => {
                    let mut glyph = style;
                    glyph.font = Some(GlyphFont::Object(object));
                    self.piece(OBJECT_GLYPH, glyph);
                }
                Projection::Plain => match fallback {
                    Some(fallback) => self.text(fallback, style),
                    None => {
                        let mut name = std::mem::take(&mut self.scratch);
                        name.clear();
                        object.write_default_text(&mut name);
                        self.piece(&name, style);
                        self.scratch = name;
                    }
                },
            },
        }
        for child in &text.extra {
            self.text(child, style);
        }
    }

    fn translate<'a, I: TextTypes>(
        &mut self,
        key: &'a str,
        fallback: Option<&'a str>,
        with: &'a [TranslateArg<I>],
        style: ResolvedStyle<'a, I>,
    ) where
        F: FnMut(&str, ResolvedStyle<'a, I>),
    {
        let host = self.host;
        let template = host.translation(key).or(fallback).unwrap_or(key);
        let pieces = match template_pieces(template, with.len()) {
            Ok(pieces) => pieces,
            Err(FormatError) => {
                self.piece(template, style);
                return;
            }
        };
        for piece in pieces {
            if self.pieces_left == 0 {
                return;
            }
            match piece {
                Piece::Literal(literal) => self.piece(literal, style),
                Piece::Argument(index) => {
                    if let Some(arg) = with.get(index) {
                        self.arg(arg, style);
                    }
                }
            }
        }
    }

    fn arg<'a, I: TextTypes>(&mut self, arg: &'a TranslateArg<I>, style: ResolvedStyle<'a, I>)
    where
        F: FnMut(&str, ResolvedStyle<'a, I>),
    {
        match arg {
            TranslateArg::Bool(value) => self.piece(if *value { "true" } else { "false" }, style),
            TranslateArg::Number(number) => {
                let mut digits = std::mem::take(&mut self.scratch);
                digits.clear();
                write_primitive_text(number, &mut digits);
                self.piece(&digits, style);
                self.scratch = digits;
            }
            TranslateArg::Text(text) => match plain_string(text) {
                Some("") => {
                    self.spend_piece();
                }
                _ => self.text(text, style),
            },
        }
    }

    fn piece<'a, I: TextTypes>(&mut self, piece: &str, style: ResolvedStyle<'a, I>)
    where
        F: FnMut(&str, ResolvedStyle<'a, I>),
    {
        if piece.is_empty() || !self.spend_piece() {
            return;
        }
        match self.projection {
            Projection::Plain => (self.emit)(piece, style),
            Projection::Drawn => self.drawn(piece, style),
        }
    }

    fn spend_piece(&mut self) -> bool {
        match self.pieces_left.checked_sub(1) {
            Some(pieces_left) => {
                self.pieces_left = pieces_left;
                true
            }
            None => false,
        }
    }

    fn drawn<'a, I: TextTypes>(&mut self, piece: &str, base: ResolvedStyle<'a, I>)
    where
        F: FnMut(&str, ResolvedStyle<'a, I>),
    {
        let mut style = base;
        let mut rest = piece;
        while let Some((run, after)) = rest.split_once('§') {
            if !run.is_empty() {
                (self.emit)(run, style);
            }
            let mut chars = after.chars();
            if let Some(code) = chars.next() {
                style = format_code(code, style, base);
            }
            rest = chars.as_str();
        }
        if !rest.is_empty() {
            (self.emit)(rest, style);
        }
    }
}

fn plain_string<I: TextTypes>(text: &Text<I>) -> Option<&str> {
    match &text.content {
        TextContent::Text { text: literal }
            if text.extra.is_empty() && text.style == Style::default() =>
        {
            Some(literal)
        }
        _ => None,
    }
}

fn format_code<'a, I: TextTypes>(
    code: char,
    mut style: ResolvedStyle<'a, I>,
    base: ResolvedStyle<'a, I>,
) -> ResolvedStyle<'a, I> {
    if let Some(color) = NamedColor::from_hex_digit(code) {
        style.color = Some(Color::Named(color));
        style.obfuscated = false;
        style.bold = false;
        style.strikethrough = false;
        style.underlined = false;
        style.italic = false;
        return style;
    }
    match code.to_ascii_lowercase() {
        'k' => style.obfuscated = true,
        'l' => style.bold = true,
        'm' => style.strikethrough = true,
        'n' => style.underlined = true,
        'o' => style.italic = true,
        'r' => style = base,
        _ => {}
    }
    style
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use mcrs_minecraft_lang::Language;
    use mcrs_minecraft_text::ItemHover;

    use super::*;
    use crate::NoHost;

    fn shown(host: &impl TextHost, json: &str, projection: Projection) -> String {
        let text: Text<ItemHover> = Text::from_str(json).expect(json);
        let mut out = String::new();
        flatten(host, &text, projection, |run, _| out.push_str(run));
        out
    }

    #[test]
    fn nested_translations_stop_at_the_piece_bound() {
        let language = Language::from_json(br#"{"grow":"%1$s%1$s%1$s%1$s"}"#).expect("valid JSON");
        for (leaf, expected) in [("ab", MAX_PIECES), ("", 0)] {
            let mut text: Text<ItemHover> = Text::text(leaf);
            for _ in 0..16 {
                text = Text::translate_args("grow", vec![text.into()]);
            }
            let mut pieces = 0;
            flatten(&language, &text, Projection::Plain, |_, _| pieces += 1);
            assert_eq!(pieces, expected, "{leaf:?}");
        }
    }

    #[test]
    fn an_exhausted_piece_bound_stops_the_expansion_of_later_nodes() {
        let repeated = "%1$s".repeat(MAX_PIECES);
        let json = format!(r#"{{"inner":"{repeated}","outer":"{repeated}"}}"#);
        let language = Language::from_json(json.as_bytes()).expect("valid JSON");
        let inner: Text<ItemHover> =
            Text::from_str(r#"{"translate":"inner","with":[true]}"#).expect("valid text");
        let text: Text<ItemHover> = Text::translate_args("outer", vec![inner.into()]);
        let mut pieces = 0;
        flatten(&language, &text, Projection::Plain, |_, _| pieces += 1);
        assert_eq!(pieces, MAX_PIECES);
    }

    #[test]
    fn nested_translations_stop_at_the_node_bound() {
        let language =
            Language::from_json(br#"{"grow":"%1$s%1$s%1$s%1$s%1$s%1$s%1$s%1$s","none":""}"#)
                .expect("valid JSON");
        for (levels, expected) in [(5, "!"), (6, "")] {
            let mut text: Text<ItemHover> = Text::translate_args("none", Vec::new());
            for _ in 0..levels {
                text = Text::translate_args("grow", vec![text.into()]);
            }
            text.extra.push(Text::text("!"));
            let mut out = String::new();
            flatten(&language, &text, Projection::Plain, |run, _| {
                out.push_str(run)
            });
            assert_eq!(out, expected, "{levels} levels");
        }
    }

    #[test]
    fn an_empty_string_argument_takes_one_piece_of_the_bound() {
        let language = Language::from_json(br#"{"grow":"%1$s%1$s%1$s%1$s"}"#).expect("valid JSON");
        for (leaf, expected) in [("\"\"", ""), (r#"{"text":"","bold":true}"#, "!")] {
            let mut text: Text<ItemHover> = Text::from_str(leaf).expect(leaf);
            for _ in 0..8 {
                text = Text::translate_args("grow", vec![text.into()]);
            }
            text.extra.push(Text::text("!"));
            let mut out = String::new();
            flatten(&language, &text, Projection::Plain, |run, _| {
                out.push_str(run)
            });
            assert_eq!(out, expected, "{leaf}");
        }
    }

    struct Client(Language);

    impl TextHost for Client {
        fn translation<'a>(&'a self, key: &'a str) -> Option<&'a str> {
            self.0.translation(key)
        }

        fn translates_keybind_name(&self) -> bool {
            true
        }
    }

    #[test]
    fn only_a_client_translates_a_keybind_name_without_a_mapping() {
        let table = br#"{"key.a":"100%%","key.b":"%s","gui.yes":"Yes"}"#;
        let client = Client(Language::from_json(table).expect("valid JSON"));
        let server = Language::from_json(table).expect("valid JSON");
        for (json, on_client, on_server) in [
            (r#"{"keybind":"key.a"}"#, "100%", "key.a"),
            (r#"{"keybind":"key.b"}"#, "%s", "key.b"),
            (r#"{"keybind":"key.c"}"#, "key.c", "key.c"),
            (r#"{"keybind":"gui.yes"}"#, "Yes", "gui.yes"),
        ] {
            for projection in [Projection::Drawn, Projection::Plain] {
                assert_eq!(shown(&client, json, projection), on_client, "{json}");
                assert_eq!(shown(&server, json, projection), on_server, "{json}");
            }
        }
    }

    #[test]
    fn each_content_kind_shows_as_the_rules_state() {
        let cases = [
            (r#""§cR§zX§""#, "RX", "§cR§zX§"),
            (
                r#"{"selector":"@e[ type = cow ]"}"#,
                "@e[ type = cow ]",
                "@e[ type = cow ]",
            ),
            (
                r#"{"score":{"name":"a","objective":"b"},"extra":["!"]}"#,
                "!",
                "!",
            ),
            (
                r#"{"nbt":"a","entity":"@s","separator":"x","extra":["!"]}"#,
                "!",
                "!",
            ),
            (r#"{"keybind":"gui.yes"}"#, "gui.yes", "gui.yes"),
            (
                r#"{"translate":"<%s> %s","with":["a"]}"#,
                "<%s> %s",
                "<%s> %s",
            ),
            (
                r#"{"translate":"%2$s %1$s %s","with":["a","b"]}"#,
                "b a a",
                "b a a",
            ),
            (
                r#"{"translate":"%s|%s|%s","with":[1.0E10,2.0,true]}"#,
                "10000000000|2|true",
                "10000000000|2|true",
            ),
            (
                r#"{"translate":"no.such.key","fallback":"fb %s!","with":["X"]}"#,
                "fb X!",
                "fb X!",
            ),
            (
                r#"{"object":"atlas","sprite":"block/stone"}"#,
                "\u{FFFC}",
                "[block/stone]",
            ),
            (
                r#"{"object":"atlas","atlas":"items","sprite":"item/diamond"}"#,
                "\u{FFFC}",
                "[item/diamond@items]",
            ),
            (
                r#"{"object":"atlas","atlas":"my:atl","sprite":"foo/bar"}"#,
                "\u{FFFC}",
                "[foo/bar@my:atl]",
            ),
            (
                r#"{"object":"atlas","sprite":"block/stone","fallback":"x"}"#,
                "\u{FFFC}",
                "x",
            ),
            (
                r#"{"object":"player","player":{"name":"Alex"}}"#,
                "\u{FFFC}",
                "[Alex head]",
            ),
            (
                r#"{"object":"player","player":{}}"#,
                "\u{FFFC}",
                "[unknown player head]",
            ),
        ];
        for (json, drawn, plain) in cases {
            assert_eq!(shown(&NoHost, json, Projection::Drawn), drawn, "{json}");
            assert_eq!(shown(&NoHost, json, Projection::Plain), plain, "{json}");
        }
    }

    struct Bindings;

    impl TextHost for Bindings {
        fn keybind_label<'a>(&'a self, name: &'a str) -> Option<&'a str> {
            (name == "key.jump").then_some("Space")
        }
    }

    #[test]
    fn a_host_with_key_mappings_names_the_bound_key() {
        let json = r#"{"keybind":"key.jump"}"#;
        assert_eq!(shown(&Bindings, json, Projection::Drawn), "Space");
        assert_eq!(shown(&NoHost, json, Projection::Drawn), "key.jump");
    }

    #[test]
    fn a_format_code_changes_the_style_to_the_end_of_the_piece() {
        let text: Text<ItemHover> =
            Text::from_str(r#"{"text":"a§c§lb§rc","bold":true,"color":"blue","extra":["d"]}"#)
                .expect("valid JSON");
        let mut runs = Vec::new();
        flatten(&NoHost, &text, Projection::Drawn, |run, style| {
            runs.push((run.to_owned(), style.color, style.bold));
        });
        let blue = Some(Color::BLUE);
        assert_eq!(
            runs,
            [
                ("a".to_owned(), blue, true),
                ("b".to_owned(), Some(Color::RED), true),
                ("c".to_owned(), blue, true),
                ("d".to_owned(), blue, true),
            ]
        );
    }

    #[test]
    fn an_argument_style_does_not_reach_a_later_piece() {
        let text: Text<ItemHover> = Text::from_str(
            r##"{"translate":"%s-%s-%s","color":"red","bold":false,"with":[{"text":"a","color":"blue"},{"text":"b","bold":true},"c"],"extra":["!"]}"##,
        )
        .expect("valid JSON");
        let mut runs = Vec::new();
        flatten(&NoHost, &text, Projection::Plain, |run, style| {
            runs.push((run.to_owned(), style.color, style.bold));
        });
        let red = Some(Color::RED);
        assert_eq!(
            runs,
            [
                ("a".to_owned(), Some(Color::BLUE), false),
                ("-".to_owned(), red, false),
                ("b".to_owned(), red, true),
                ("-".to_owned(), red, false),
                ("c".to_owned(), red, false),
                ("!".to_owned(), red, false),
            ]
        );
    }

    #[test]
    fn a_child_and_an_argument_inherit_the_shadow_colour() {
        let text: Text<ItemHover> = Text::from_str(
            r#"{"translate":"%s-%s","shadow_color":1,"with":[{"text":"a","shadow_color":2},"b"],"extra":[{"text":"c","shadow_color":3,"extra":["d"]}]}"#,
        )
        .expect("valid JSON");
        let mut runs = Vec::new();
        flatten(&NoHost, &text, Projection::Plain, |run, style| {
            runs.push((run.to_owned(), style.shadow_color.map(|color| color.0)));
        });
        assert_eq!(
            runs,
            [
                ("a".to_owned(), Some(2)),
                ("-".to_owned(), Some(1)),
                ("b".to_owned(), Some(1)),
                ("c".to_owned(), Some(3)),
                ("d".to_owned(), Some(3)),
            ]
        );
    }
}
