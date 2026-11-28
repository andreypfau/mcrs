use mcrs_minecraft_protocol_game::text::color::RgbColor;
use mcrs_minecraft_protocol_game::text::{Color, GlyphFont, Text};
use mcrs_minecraft_text_flatten::{Projection, UnitBudget, flatten};

use crate::font::{Font, Span};
use crate::language::ClientLanguage;
use crate::span_events::{Interner, SpanEvents};

pub fn argb(color: Color) -> u32 {
    let rgb = match color {
        Color::Rgb(rgb) => rgb,
        Color::Named(named) => RgbColor::from(named),
    };
    0xFF00_0000 | u32::from(rgb.r) << 16 | u32::from(rgb.g) << 8 | u32::from(rgb.b)
}

pub fn text_spans(text: &Text, language: &ClientLanguage, color: u32) -> Vec<Span> {
    spans(text, language, color, None)
}

pub fn text_spans_with_events(
    text: &Text,
    language: &ClientLanguage,
    color: u32,
    events: &mut SpanEvents,
) -> Vec<Span> {
    spans(text, language, color, Some(events))
}

fn spans(
    text: &Text,
    language: &ClientLanguage,
    color: u32,
    events: Option<&mut SpanEvents>,
) -> Vec<Span> {
    let mut interner = events.map(Interner::new);
    let mut out = Vec::new();
    let mut budget = UnitBudget::new();
    flatten(language, text, Projection::Drawn, |run, style| {
        if matches!(style.font, Some(GlyphFont::Object(_))) {
            return;
        }
        let run = budget.take(run);
        if run.is_empty() {
            return;
        }
        let (hover, click) = match interner.as_mut() {
            Some(interner) => (
                style.hover_event.and_then(|hover| interner.hover(hover)),
                style.click_event.and_then(|click| interner.click(click)),
            ),
            None => (None, None),
        };
        out.push(Span {
            text: run.to_owned(),
            color: style.color.map_or(color, argb),
            bold: style.bold,
            hover,
            click,
            insertion: style.insertion.map(Box::from),
        });
    });
    out
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Wrap {
    ChatLog,
    Tooltip,
}

pub fn wrap(spans: &[Span], width: i32, style: Wrap, font: &Font, rows: &mut Vec<Vec<Span>>) {
    let tooltip = style == Wrap::Tooltip;
    if tooltip && spans.iter().all(|span| span.text.is_empty()) {
        return;
    }
    let mut row: Vec<Span> = Vec::new();
    let mut used = 0.0;
    let width = width as f32;
    let start_row = |row: &mut Vec<Span>, used: &mut f32, rows: &mut Vec<Vec<Span>>| {
        rows.push(std::mem::take(row));
        *used = 0.0;
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
            if used + advance > width && used > 0.0 {
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
                    used += font.exact_width(std::slice::from_ref(&carried));
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
    use crate::font::corpus_font;
    use mcrs_minecraft_protocol_game::text::IntoText;
    use mcrs_minecraft_text_flatten::MAX_UNITS;

    #[test]
    fn drawn_spans_stop_at_the_unit_bound() {
        let language = ClientLanguage::from_json(br#"{"grow":"%1$s%1$s%1$s%1$s"}"#);
        let mut text = Text::text("a".repeat(30_000));
        for _ in 0..3 {
            text = Text::translate_args("grow", vec![text.into()]);
        }
        let spans = text_spans(&text, &language, 0xFFFF_FFFF);
        let units: usize = spans
            .iter()
            .map(|span| span.text.encode_utf16().count())
            .sum();
        assert_eq!(units, MAX_UNITS);
    }

    #[test]
    fn many_runs_under_one_event_store_its_payload_once() {
        let text = Text::text("§lX".repeat(1000)).on_hover_show_text("tip");
        let mut events = SpanEvents::default();
        let spans =
            text_spans_with_events(&text, &ClientLanguage::default(), 0xFFFF_FFFF, &mut events);
        assert_eq!(spans.len(), 1000);
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn click_event_and_insertion_pass_to_children_and_survive_wrapping() {
        let text = Text::text("ab")
            .on_click_suggest_command("/help")
            .insertion("ins");
        let mut text = text;
        text.extra.push(Text::text("cd").insertion("own"));
        let mut events = SpanEvents::default();
        let spans =
            text_spans_with_events(&text, &ClientLanguage::default(), 0xFFFF_FFFF, &mut events);
        assert_eq!(spans.len(), 2);
        assert!(
            spans
                .iter()
                .all(|span| span.click.and_then(|index| events.click(index)).is_some())
        );
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
            let spans = text_spans(&text, ClientLanguage::corpus(), 0xFFFF_FFFF);
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
