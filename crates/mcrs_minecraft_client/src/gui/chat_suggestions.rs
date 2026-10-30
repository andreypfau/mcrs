//! The suggestion list and the hint lines over the input of the open chat screen.

use bevy::math::{IRect, IVec2};
use mcrs_minecraft_chat_client::{ChatScreen, Hint, LIST_ROWS, SuggestionMenu};

use super::chat::{GRAY, RED};
use super::font::{self, Font, Span};
use super::item_decorations::WHITE;
use super::language::Language;
use super::scene::{GuiQuad, GuiSize};
use super::text;

const ROW: i32 = 12;
const ROW_FILL: u32 = 0xD000_0000;
const SELECTED: u32 = 0xFFFF_FF00;
/// An error line has no colour of its own.
const ERROR_LINE: u32 = 0xFFFF_FFFF;

/// The left x of the suggestion list or the hint lines over `start`.
fn column(size: GuiSize, font: &Font, input: &str, start: usize, width: i32) -> i32 {
    let x0 = input
        .get(..start)
        .map_or(4, |before| 4 + font.str_width(before));
    x0.clamp(0, (size.width - width).max(0))
}

/// The row under `point`, with the edge pixels of each row excluded.
pub fn row_at(rect: IRect, point: IVec2) -> Option<usize> {
    let dy = point.y - rect.min.y;
    let inside = rect.min.x < point.x && point.x < rect.max.x && 0 < dy && point.y < rect.max.y;
    (inside && dy % ROW != 0).then_some((dy / ROW) as usize)
}

/// The row under `point` for a click or a wheel step, which count the edges.
pub fn row_under(rect: IRect, point: IVec2) -> Option<usize> {
    rect.contains(point)
        .then(|| ((point.y - rect.min.y) / ROW) as usize)
}

/// Where the rows of the shown suggestion list are, and the entry of the top row.
pub(crate) struct ListRows {
    pub rect: IRect,
    pub first: usize,
}

pub(crate) fn list_rows(size: GuiSize, font: &Font, screen: &ChatScreen) -> Option<ListRows> {
    let list = screen.commands.list()?;
    let entries = list.entries();
    let width = entries.iter().map(|text| font.str_width(text)).max()?;
    let rows = entries.len().min(LIST_ROWS) as i32;
    let x = column(size, font, screen.input(), entries.range().start, width);
    let top = size.height - 15 - ROW * rows;
    Some(ListRows {
        rect: IRect::new(x - 1, top, x + width, top + ROW * rows),
        first: list.offset(),
    })
}

pub(super) fn suggestion_list(
    font: &Font,
    list: &SuggestionMenu,
    rows: &ListRows,
    out: &mut Vec<GuiQuad>,
) {
    let count = list.entries().len();
    let rect = rows.rect;
    if count > LIST_ROWS {
        for (y, dotted) in [
            (rect.min.y - 1, list.offset() > 0),
            (rect.max.y, list.offset() + LIST_ROWS < count),
        ] {
            out.push(GuiQuad::Fill {
                rect: IRect::new(rect.min.x, y, rect.max.x, y + 1),
                color: ROW_FILL,
            });
            if dotted {
                for x in (rect.min.x..rect.max.x).step_by(2) {
                    out.push(GuiQuad::Fill {
                        rect: IRect::new(x, y, x + 1, y + 1),
                        color: WHITE,
                    });
                }
            }
        }
    }
    for (row, (index, text)) in list.rows().enumerate() {
        let top = rect.min.y + ROW * row as i32;
        out.push(GuiQuad::Fill {
            rect: IRect::new(rect.min.x, top, rect.max.x, top + ROW),
            color: ROW_FILL,
        });
        let colour = if index == list.selected() {
            SELECTED
        } else {
            GRAY
        };
        font::draw_text(
            font,
            IVec2::new(rect.min.x + 1, top + 2),
            &[Span::new(text, colour)],
            out,
        );
    }
}

pub(super) fn hint_lines(
    size: GuiSize,
    font: &Font,
    language: &Language,
    screen: &ChatScreen,
    out: &mut Vec<GuiQuad>,
) {
    let commands = &screen.commands;
    let hints = commands.hints();
    let width = |hint: &Hint| match hint {
        // chisle: an error line builds its spans, with a String each, on every frame it shows; caching them per input lifts it.
        Hint::Error(_) => font.width(&error_spans(commands.input(), hint, language)),
        Hint::Usage(range) => font.str_width(commands.usage_text(range.clone())),
        Hint::Restriction(key) => font.str_width(language.get(key)),
    };
    let Some(width) = hints.iter().map(width).max() else {
        return;
    };
    let x = if !commands.is_command() {
        0
    } else {
        column(
            size,
            font,
            commands.input(),
            commands.suggestions().range().start,
            width,
        )
    };
    for (index, hint) in hints.iter().enumerate() {
        let top = size.height - 27 - ROW * index as i32;
        out.push(GuiQuad::Fill {
            rect: IRect::new(x - 1, top, x + width + 1, top + ROW),
            color: ROW_FILL,
        });
        let at = IVec2::new(x, top + 2);
        match hint {
            Hint::Error(_) => {
                font::draw_text(
                    font,
                    at,
                    &error_spans(commands.input(), hint, language),
                    out,
                );
            }
            Hint::Usage(range) => {
                font::draw_str(
                    font,
                    at,
                    commands.usage_text(range.clone()),
                    GRAY,
                    false,
                    out,
                );
            }
            Hint::Restriction(key) => {
                font::draw_str(font, at, language.get(key), RED, false, out);
            }
        }
    }
}

fn error_spans(input: &str, hint: &Hint, language: &Language) -> Vec<Span> {
    let Hint::Error(error) = hint else {
        return Vec::new();
    };
    let Ok(line) = mcrs_minecraft_protocol::text::named(&mcrs_minecraft_command::parse_error_line(
        input, error,
    )) else {
        return Vec::new();
    };
    text::text_spans(&line, language, ERROR_LINE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_answers_the_mouse_inside_its_edges_only() {
        let rect = IRect::new(9, 100, 50, 124);
        for (point, row) in [
            ((10, 101), Some(0)),
            ((49, 111), Some(0)),
            ((20, 113), Some(1)),
            ((20, 123), Some(1)),
            ((9, 105), None),
            ((50, 105), None),
            ((20, 100), None),
            ((20, 112), None),
            ((20, 124), None),
        ] {
            assert_eq!(row_at(rect, IVec2::from(point)), row, "{point:?}");
        }
    }

    #[test]
    fn a_click_or_a_wheel_step_counts_the_edges_of_the_list() {
        let rect = IRect::new(9, 100, 50, 124);
        for (point, row) in [
            ((9, 100), Some(0)),
            ((20, 111), Some(0)),
            ((20, 112), Some(1)),
            ((50, 124), Some(2)),
            ((8, 105), None),
            ((51, 105), None),
            ((20, 99), None),
            ((20, 125), None),
        ] {
            assert_eq!(row_under(rect, IVec2::from(point)), row, "{point:?}");
        }
    }
}
