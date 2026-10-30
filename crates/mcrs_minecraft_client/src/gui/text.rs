//! Text components as styled spans, and the wrap of spans into rows.

use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::text::color::RgbColor;
use mcrs_minecraft_protocol::text::{Color, Text, TextContent, TranslateArg};

use super::font::{ClickEvent, Font, Hover, Span};
use super::language::Language;

#[derive(Clone, Copy)]
struct Inherited<'a> {
    color: u32,
    bold: bool,
    hover: Option<&'a Hover>,
    click: Option<&'a ClickEvent>,
    insertion: Option<&'a str>,
}

impl Inherited<'_> {
    fn apply(self, span: Span) -> Span {
        Span {
            bold: span.bold || self.bold,
            hover: span
                .hover
                .or_else(|| self.hover.map(|hover| Box::new(hover.clone()))),
            click: span
                .click
                .or_else(|| self.click.map(|click| Box::new(click.clone()))),
            insertion: span.insertion.or_else(|| self.insertion.map(Box::from)),
            ..span
        }
    }
}

/// An opaque ARGB colour.
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
        NbtTag::Float(v) => mcrs_minecraft_snbt::java_float(*v),
        NbtTag::Double(v) => mcrs_minecraft_snbt::java_double(*v),
        _ => String::new(),
    }
}

// chisle: every span clones its hover event into a Box; spans borrowing the hover from the Text lift it.
fn push(out: &mut Vec<Span>, text: impl Into<String>, style: Inherited) {
    let text = text.into();
    if !text.is_empty() {
        out.push(style.apply(Span::new(text, style.color)));
    }
}

fn collect<'a>(text: &'a Text, language: &Language, parent: Inherited<'a>, out: &mut Vec<Span>) {
    let style = Inherited {
        color: text.style.color.map_or(parent.color, argb),
        bold: text.style.bold.unwrap_or(parent.bold),
        hover: text.style.hover_event.as_ref().or(parent.hover),
        click: text.style.click_event.as_ref().or(parent.click),
        insertion: text.style.insertion.as_deref().or(parent.insertion),
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
                    .map(|span| style.apply(span)),
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
    collect(
        text,
        language,
        Inherited {
            color,
            bold: false,
            hover: None,
            click: None,
            insertion: None,
        },
        &mut out,
    );
    out
}

/// How [`wrap`] starts a row after a break.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Wrap {
    /// Each row after the first starts with one unstyled space.
    ChatLog,
    /// A space at the wrap point is dropped, and an empty text gives no row.
    Tooltip,
}

/// Breaks spans into rows no wider than `width`: at the last space that fits, or inside a word
/// that alone is too wide. A line break starts a new row.
pub fn wrap(spans: &[Span], width: i32, style: Wrap, font: &Font, rows: &mut Vec<Vec<Span>>) {
    let tooltip = style == Wrap::Tooltip;
    if tooltip && spans.iter().all(|span| span.text.is_empty()) {
        return;
    }
    let mut row: Vec<Span> = Vec::new();
    let mut used = 0;
    let start_row = |row: &mut Vec<Span>, used: &mut i32, rows: &mut Vec<Vec<Span>>| {
        rows.push(std::mem::take(row));
        *used = 0;
        if !tooltip {
            row.push(Span::new(" ", 0xFFFF_FFFF));
            *used = font.advance(' ', false);
        }
    };
    for span in spans {
        for c in span.text.chars() {
            if c == '\n' {
                start_row(&mut row, &mut used, rows);
                continue;
            }
            let advance = font.advance(c, span.bold);
            if used + advance > width && used > 0 {
                if tooltip && c == ' ' {
                    start_row(&mut row, &mut used, rows);
                    continue;
                }
                let carried = take_last_word(&mut row);
                if tooltip {
                    trim_space(&mut row);
                }
                start_row(&mut row, &mut used, rows);
                for carried in carried {
                    used += font.width(std::slice::from_ref(&carried));
                    row.push(carried);
                }
            }
            match row.last_mut() {
                Some(last)
                    if last.color == span.color
                        && last.bold == span.bold
                        && last.hover == span.hover
                        && last.click == span.click
                        && last.insertion == span.insertion =>
                {
                    last.text.push(c)
                }
                _ => row.push(Span {
                    text: c.to_string(),
                    ..span.clone()
                }),
            }
            used += advance;
        }
    }
    rows.push(row);
}

fn trim_space(row: &mut Vec<Span>) {
    if let Some(last) = row.last_mut()
        && last.text.ends_with(' ')
    {
        last.text.pop();
        if last.text.is_empty() {
            row.pop();
        }
    }
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
pub(crate) fn row_texts(rows: &[Vec<Span>]) -> Vec<String> {
    rows.iter()
        .map(|row| row.iter().map(|span| span.text.as_str()).collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::font::corpus_font;
    use mcrs_minecraft_protocol::text::IntoText;

    #[test]
    fn click_event_and_insertion_pass_to_children_and_survive_wrapping() {
        let text = Text::text("ab")
            .on_click_suggest_command("/help")
            .insertion("ins");
        let mut text = text;
        text.extra.push(Text::text("cd").insertion("own"));
        let spans = text_spans(&text, &Language::default(), 0xFFFF_FFFF);
        assert_eq!(spans.len(), 2);
        assert!(spans.iter().all(|span| span.click.is_some()));
        assert_eq!(spans[0].insertion.as_deref(), Some("ins"));
        assert_eq!(spans[1].insertion.as_deref(), Some("own"));
        let mut rows = Vec::new();
        wrap(&spans, 1000, Wrap::ChatLog, &Font::default(), &mut rows);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].len(), 2);
    }

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
        fn a_text_wraps_at_the_last_space_that_fits_or_inside_a_word_too_wide() {
            let font = corpus_font();
            let cases: [(Wrap, &str, &str, &[&str]); 6] = [
                (
                    Wrap::ChatLog,
                    "aaaa aaaa ",
                    "aaaa aaaa aaaa",
                    &["aaaa aaaa ", " aaaa"],
                ),
                (Wrap::Tooltip, "aaaa aaaa", "", &[]),
                (
                    Wrap::Tooltip,
                    "aaaa aaaa",
                    "aaaa aaaa aaaa",
                    &["aaaa aaaa", "aaaa"],
                ),
                (Wrap::Tooltip, "aaaa aaaa", "a\nb", &["a", "b"]),
                (Wrap::Tooltip, "aaaa aaaa", "a\n", &["a", ""]),
                (Wrap::Tooltip, "aaaaa", "aaaaaaaa", &["aaaaa", "aaa"]),
            ];
            for (style, fits, text, lines) in cases {
                let width = font.width(&[Span::new(fits, 0)]);
                let mut rows = Vec::new();
                wrap(
                    &[Span::new(text, 0xFFFF_FFFF)],
                    width,
                    style,
                    font,
                    &mut rows,
                );
                assert_eq!(row_texts(&rows), lines, "{text:?} in {fits:?}");
            }
        }
    }
}
