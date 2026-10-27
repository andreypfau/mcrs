use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::text::color::RgbColor;
use mcrs_minecraft_protocol::text::{Color, Text, TextContent, TranslateArg};

use super::font::{Font, Span};
use super::language::Language;

#[derive(Clone, Copy)]
struct Inherited {
    color: u32,
    bold: bool,
}

pub fn argb(color: Color) -> u32 {
    let rgb = match color {
        Color::Rgb(rgb) => rgb,
        Color::Named(named) => RgbColor::from(named),
    };
    0xFF00_0000 | u32::from(rgb.r) << 16 | u32::from(rgb.g) << 8 | u32::from(rgb.b)
}

fn number(tag: &NbtTag) -> String {
    match tag {
        NbtTag::Byte(v) => v.to_string(),
        NbtTag::Short(v) => v.to_string(),
        NbtTag::Int(v) => v.to_string(),
        NbtTag::Long(v) => v.to_string(),
        NbtTag::Float(v) => v.to_string(),
        NbtTag::Double(v) => v.to_string(),
        _ => String::new(),
    }
}

fn push(out: &mut Vec<Span>, text: impl Into<String>, style: Inherited) {
    let text = text.into();
    if !text.is_empty() {
        out.push(Span {
            text,
            color: style.color,
            bold: style.bold,
        });
    }
}

fn collect(text: &Text, language: &Language, parent: Inherited, out: &mut Vec<Span>) {
    let style = Inherited {
        color: text.style.color.map_or(parent.color, argb),
        bold: text.style.bold.unwrap_or(parent.bold),
    };
    match &text.content {
        TextContent::Text { text } => push(out, text.as_ref(), style),
        TextContent::Translate {
            translate,
            fallback,
            with,
        } => {
            let args: Vec<Vec<Span>> = with
                .iter()
                .map(|arg| {
                    let mut spans = Vec::new();
                    match arg {
                        TranslateArg::Text(text) => collect(text, language, style, &mut spans),
                        TranslateArg::Bool(value) => push(&mut spans, value.to_string(), style),
                        TranslateArg::Number(tag) => push(&mut spans, number(tag), style),
                    }
                    spans
                })
                .collect();
            let key = match fallback {
                Some(fallback) if !language.contains(translate) => fallback.as_ref(),
                _ => translate.as_ref(),
            };
            out.extend(
                language
                    .translate(key, style.color, &args)
                    .into_iter()
                    .map(|span| Span {
                        bold: span.bold || style.bold,
                        ..span
                    }),
            );
        }
        TextContent::Keybind { keybind } => push(out, language.get(keybind), style),
        _ => {}
    }
    for child in &text.extra {
        collect(child, language, style, out);
    }
}

/// The spans of a text component in `color` where it sets none. Content that needs the world
/// (scores, selectors, NBT) shows nothing.
pub fn text_spans(text: &Text, language: &Language, color: u32) -> Vec<Span> {
    let mut out = Vec::new();
    collect(text, language, Inherited { color, bold: false }, &mut out);
    out
}

/// Breaks spans into rows no wider than `width`: at the last space that fits, or inside a word
/// that alone is too wide. Each row after the first starts with one unstyled space
/// (CHAT-LOG-001).
pub fn wrap(spans: &[Span], width: i32, font: &Font, rows: &mut Vec<Vec<Span>>) {
    let mut row: Vec<Span> = Vec::new();
    let mut used = 0;
    let start_row = |row: &mut Vec<Span>, used: &mut i32, rows: &mut Vec<Vec<Span>>| {
        rows.push(std::mem::take(row));
        row.push(Span::new(" ", 0xFFFF_FFFF));
        *used = font.advance(' ', false);
    };
    for span in spans {
        for c in span.text.chars() {
            if c == '\n' {
                start_row(&mut row, &mut used, rows);
                continue;
            }
            let advance = font.advance(c, span.bold);
            if used + advance > width && used > 0 {
                let carried = take_last_word(&mut row);
                start_row(&mut row, &mut used, rows);
                for carried in carried {
                    used += font.width(std::slice::from_ref(&carried));
                    row.push(carried);
                }
            }
            match row.last_mut() {
                Some(last) if last.color == span.color && last.bold == span.bold => {
                    last.text.push(c)
                }
                _ => row.push(Span {
                    text: c.to_string(),
                    color: span.color,
                    bold: span.bold,
                }),
            }
            used += advance;
        }
    }
    rows.push(row);
}

/// Removes the text after the last space of a row, when the row has a space after its start.
fn take_last_word(row: &mut Vec<Span>) -> Vec<Span> {
    let Some((index, at)) = row
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, span)| span.text.rfind(' ').map(|at| (index, at)))
        .filter(|&(index, at)| index > 0 || at > 0)
    else {
        return Vec::new();
    };
    let mut carried: Vec<Span> = row.drain(index + 1..).collect();
    if let Some(span) = row.get_mut(index) {
        let tail = span.text.split_off(at + 1);
        if !tail.is_empty() {
            carried.insert(
                0,
                Span {
                    text: tail,
                    ..span.clone()
                },
            );
        }
    }
    carried
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::font::corpus_font;
    use mcrs_minecraft_protocol::text::IntoText;

    mod jar {
        use super::*;

        #[test]
        fn a_translation_takes_the_colour_of_its_component_and_its_arguments_keep_theirs() {
            let text = Text::translate(
                "chat.type.text",
                vec!["Alex".color(Color::GOLD), "hi".into_text()],
            )
            .color(Color::GRAY);
            let spans = text_spans(&text, Language::corpus(), 0xFFFF_FFFF);
            assert_eq!(
                spans,
                [
                    Span::new("<", 0xFFAA_AAAA),
                    Span::new("Alex", 0xFFFF_AA00),
                    Span::new("> ", 0xFFAA_AAAA),
                    Span::new("hi", 0xFFAA_AAAA),
                ]
            );
        }

        #[test]
        fn a_long_line_breaks_after_a_space_and_continues_indented() {
            let font = corpus_font();
            let word = "aaaa";
            let spans = [Span::new(format!("{word} {word} {word}"), 0xFFFF_FFFF)];
            let width = font.width(&[Span::new(format!("{word} {word} "), 0)]);
            let mut rows = Vec::new();
            wrap(&spans, width, font, &mut rows);
            let texts: Vec<String> = rows
                .iter()
                .map(|row| row.iter().map(|span| span.text.as_str()).collect())
                .collect();
            assert_eq!(texts, ["aaaa aaaa ", " aaaa"]);
        }
    }
}
