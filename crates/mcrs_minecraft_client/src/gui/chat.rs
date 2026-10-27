use bevy::math::{IRect, IVec2};
use bevy::prelude::*;
use bevy::time::Real;
use mcrs_minecraft_chat::{ChatScreen, Highlight, Hint, Tag, fade};
use mcrs_minecraft_command::parse_error_line;

use super::font::{Font, Span, draw_centered_text, draw_text};
use super::language::Language;
use super::scene::{GuiAtlas, GuiQuad, GuiScene, GuiSize};
use super::text::text_spans;
use crate::chat::{Chat, ChatInput};
use crate::screen::Screen;

const CHAT_WIDTH: i32 = 320;
const LINE: i32 = 9;
const TEXT_OFFSET: i32 = 8;
const BOTTOM: i32 = 40;
const FOCUSED_HEIGHT: f32 = 1.0;
const UNFOCUSED_HEIGHT: f32 = 0.4375;
const ICON_WIDTH: i32 = 15;
const ROW: i32 = 12;
const ROW_FILL: u32 = 0xD000_0000;
const INPUT_FILL: u32 = 0x8000_0000;
const INPUT_TEXT: u32 = 0xFFE0_E0E0;
const GRAY: u32 = 0xFFAA_AAAA;
const RED: u32 = 0xFFFF_5555;
const SELECTED: u32 = 0xFFFF_FF00;
const GHOST: u32 = 0xFF80_8080;
/// An error line has no colour of its own (CCMD-HINT-002); an experiment can change this.
const ERROR_LINE: u32 = 0xFFFF_FFFF;
const ARGUMENT_COLOURS: [u32; 5] = [
    0xFF55_FFFF,
    0xFFFF_FF55,
    0xFF55_FF55,
    0xFFFF_55FF,
    0xFFFF_AA00,
];
const CARET_BLINK_MS: u128 = 300;
const MAX_INPUT: usize = 256;

/// The lines of the chat view: 20 open, 10 closed (CHAT-LOG-004).
pub fn per_page(open: bool) -> usize {
    let height = if open {
        FOCUSED_HEIGHT
    } else {
        UNFOCUSED_HEIGHT
    };
    ((height * 160.0 + 20.0).floor() as i32 / LINE) as usize
}

pub fn wrap_width(tag: Option<Tag>) -> i32 {
    if tag.is_some_and(Tag::has_icon) {
        CHAT_WIDTH - ICON_WIDTH
    } else {
        CHAT_WIDTH
    }
}

fn with_alpha(color: u32, alpha: f32) -> u32 {
    let base = f32::from((color >> 24) as u8);
    ((base * alpha) as u32) << 24 | (color & 0x00FF_FFFF)
}

fn text_width(font: &Font, text: &str) -> i32 {
    text.chars().map(|c| font.advance(c, false)).sum()
}

/// The lines of the chat log (CHAT-LOG-002 to CHAT-LOG-004) at the default options.
pub fn chat_lines(
    size: GuiSize,
    chat: &Chat,
    open: bool,
    background: f32,
    out: &mut Vec<GuiQuad>,
    font: &Font,
) {
    let bottom = size.height - BOTTOM;
    let page = per_page(open);
    let mut shown = 0;
    for (slot, line) in chat.log.visible(page) {
        shown += 1;
        let alpha = if open {
            1.0
        } else {
            fade(chat.tick.saturating_sub(line.tick))
        };
        if alpha <= 1e-5 {
            continue;
        }
        let slot_bottom = bottom - slot as i32 * LINE;
        out.push(GuiQuad::Fill {
            rect: IRect::new(0, slot_bottom - LINE, CHAT_WIDTH + 12, slot_bottom),
            color: with_alpha(0xFF00_0000, alpha * background),
        });
        if let Some(tag) = line.tag {
            out.push(GuiQuad::Fill {
                rect: IRect::new(0, slot_bottom - LINE, 2, slot_bottom),
                color: with_alpha(0xFF00_0000 | tag.indicator(), alpha),
            });
        }
        let spans: Vec<Span> = line
            .content
            .iter()
            .map(|span| Span {
                color: with_alpha(span.color, alpha),
                ..span.clone()
            })
            .collect();
        draw_text(font, IVec2::new(4, slot_bottom - TEXT_OFFSET), &spans, out);
    }
    let lines = chat.log.lines().len();
    if open && lines > shown && lines > 0 {
        let visible = shown as i32 * LINE;
        let total = lines as i32 * LINE;
        let q = chat.log.scroll() as i32 * visible / lines as i32;
        let h = visible * visible / total;
        let alpha = if q > bottom { 170 } else { 96 };
        let colour = if chat.log.new_since_scroll() {
            0xCC_3333
        } else {
            0x33_33AA
        };
        out.push(GuiQuad::Fill {
            rect: IRect::new(CHAT_WIDTH + 8, bottom - q - h, CHAT_WIDTH + 9, bottom - q),
            color: alpha << 24 | colour,
        });
        out.push(GuiQuad::Fill {
            rect: IRect::new(CHAT_WIDTH + 9, bottom - q - h, CHAT_WIDTH + 10, bottom - q),
            color: alpha << 24 | 0xCC_CCCC,
        });
    }
}

/// The left x of the suggestion list or the hint lines over `start` (CCMD-LIST-002).
fn column(size: GuiSize, font: &Font, input: &str, start: usize, width: i32) -> i32 {
    let x0 = input
        .get(..start)
        .map_or(4, |before| 4 + text_width(font, before));
    x0.clamp(0, (size.width - width).max(0))
}

/// Where the rows of the shown suggestion list are, and the entry of the top row.
pub struct ListRows {
    pub rect: IRect,
    pub first: usize,
    x: i32,
}

pub fn list_rows(size: GuiSize, font: &Font, screen: &ChatScreen) -> Option<ListRows> {
    let list = screen.commands.list()?;
    let entries = list.entries();
    let width = entries.iter().map(|text| text_width(font, text)).max()?;
    let rows = entries.len().min(10) as i32;
    let x = column(size, font, screen.input(), entries.range().0, width);
    let top = size.height - 15 - ROW * rows;
    Some(ListRows {
        rect: IRect::new(x - 1, top, x + width, top + ROW * rows),
        first: list.offset(),
        x,
    })
}

fn suggestion_list(size: GuiSize, font: &Font, screen: &ChatScreen, out: &mut Vec<GuiQuad>) {
    let (Some(list), Some(rows)) = (screen.commands.list(), list_rows(size, font, screen)) else {
        return;
    };
    let count = list.entries().len();
    let rect = rows.rect;
    if count > 10 {
        for (y, dotted) in [
            (rect.min.y - 1, list.offset() > 0),
            (rect.max.y, list.offset() + 10 < count),
        ] {
            out.push(GuiQuad::Fill {
                rect: IRect::new(rect.min.x, y, rect.max.x, y + 1),
                color: ROW_FILL,
            });
            if dotted {
                for x in (rect.min.x..rect.max.x).step_by(2) {
                    out.push(GuiQuad::Fill {
                        rect: IRect::new(x, y, x + 1, y + 1),
                        color: 0xFFFF_FFFF,
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
        draw_text(
            font,
            IVec2::new(rows.x, top + 2),
            &[Span::new(text, colour)],
            out,
        );
    }
}

fn hint_lines(
    size: GuiSize,
    font: &Font,
    language: &Language,
    screen: &ChatScreen,
    out: &mut Vec<GuiQuad>,
) {
    let commands = &screen.commands;
    let lines: Vec<Vec<Span>> = commands
        .hints()
        .iter()
        .map(|hint| match hint {
            Hint::Error(error) => text_spans(
                &parse_error_line(commands.input(), error),
                language,
                ERROR_LINE,
            ),
            Hint::Usage(range) => vec![Span::new(commands.usage_text(range.clone()), GRAY)],
            Hint::Restriction(key) => vec![Span::new(language.get(key), RED)],
        })
        .collect();
    let Some(width) = lines.iter().map(|line| font.width(line)).max() else {
        return;
    };
    let x = if commands.hints_from_chat_text() {
        0
    } else {
        column(size, font, commands.input(), commands.start(), width)
    };
    for (index, line) in lines.iter().enumerate() {
        let top = size.height - 27 - ROW * index as i32;
        out.push(GuiQuad::Fill {
            rect: IRect::new(x - 1, top, x + width + 1, top + ROW),
            color: ROW_FILL,
        });
        draw_text(font, IVec2::new(x, top + 2), line, out);
    }
}

/// The input with the colours of CCMD-HL-001, or one colour for chat text and a draft.
fn input_spans(screen: &ChatScreen) -> Vec<Span> {
    let input = screen.input();
    if screen.is_draft() {
        return vec![Span::new(input, GRAY)];
    }
    if !screen.commands.is_command() || screen.commands.input() != input {
        return vec![Span::new(input, INPUT_TEXT)];
    }
    let mut spans = Vec::new();
    let mut at = 0;
    for &(span, highlight) in screen.commands.highlights() {
        if span.start > at {
            spans.push(Span::new(
                input.get(at..span.start).unwrap_or_default(),
                GRAY,
            ));
        }
        let colour = match highlight {
            Highlight::Argument(index) => ARGUMENT_COLOURS[usize::from(index) % 5],
            Highlight::Trailing => RED,
        };
        spans.push(Span::new(
            input.get(span.start..span.end).unwrap_or_default(),
            colour,
        ));
        at = span.end.max(at);
    }
    if at < input.len() {
        spans.push(Span::new(input.get(at..).unwrap_or_default(), GRAY));
    }
    spans
}

/// The input box with its caret and ghost text (CHAT-BOX-001, CCMD-LIST-003).
fn input_box(
    size: GuiSize,
    font: &Font,
    screen: &ChatScreen,
    caret_on: bool,
    out: &mut Vec<GuiQuad>,
) {
    out.push(GuiQuad::Fill {
        rect: IRect::new(2, size.height - 14, size.width - 2, size.height - 2),
        color: INPUT_FILL,
    });
    let input = screen.input();
    let caret = screen.edit().caret();
    let visible = size.width - 8;
    let mut skip = 0;
    while skip < caret && text_width(font, input.get(skip..caret).unwrap_or_default()) > visible {
        skip += input
            .get(skip..)
            .and_then(|rest| rest.chars().next())
            .map_or(1, char::len_utf8);
    }
    let y = size.height - 12;
    let spans: Vec<Span> = input_spans(screen)
        .into_iter()
        .scan(0usize, |at, span| {
            let start = *at;
            *at += span.text.len();
            let cut = skip.saturating_sub(start).min(span.text.len());
            Some(Span {
                text: span.text.get(cut..).unwrap_or_default().to_owned(),
                ..span
            })
        })
        .collect();
    draw_text(font, IVec2::new(4, y), &spans, out);
    let caret_x = 4 + text_width(font, input.get(skip..caret).unwrap_or_default());
    let at_end = caret == input.len();
    if at_end
        && input.chars().map(char::len_utf16).sum::<usize>() < MAX_INPUT
        && let Some(ghost) = screen.commands.list().and_then(|list| list.ghost(input))
    {
        draw_text(
            font,
            IVec2::new(caret_x - 1, y),
            &[Span::new(ghost, GHOST)],
            out,
        );
    }
    if caret_on {
        if at_end {
            draw_text(
                font,
                IVec2::new(caret_x, y),
                &[Span::new("_", INPUT_TEXT)],
                out,
            );
        } else {
            out.push(GuiQuad::Fill {
                rect: IRect::new(caret_x, y - 1, caret_x + 1, y + 10),
                color: 0xFFD0_D0D0,
            });
        }
    }
}

pub fn draw(
    chat: Res<Chat>,
    input: Res<ChatInput>,
    screen: Res<Screen>,
    language: Option<Res<Language>>,
    atlas: Option<Res<GuiAtlas>>,
    time: Res<Time<Real>>,
    mut scene: ResMut<GuiScene>,
) {
    let (Some(atlas), Some(language)) = (atlas, language) else {
        return;
    };
    let font = &atlas.0.font;
    let scene = &mut *scene;
    let size = scene.size;
    let open = *screen == Screen::Chat;
    chat_lines(
        size,
        &chat,
        open,
        chat.options.text_background_opacity,
        &mut scene.quads,
        font,
    );
    if let Some((text, alpha)) = chat.overlay.shown(0.0) {
        let spans = text_spans(text, &language, u32::from(alpha) << 24 | 0x00FF_FFFF);
        draw_centered_text(
            font,
            size.width / 2,
            size.height - 72,
            &spans,
            &mut scene.quads,
        );
    }
    if !open {
        return;
    }
    let screen = &input.0;
    let caret_on = (time.elapsed().as_millis() / CARET_BLINK_MS).is_multiple_of(2);
    input_box(size, font, screen, caret_on, &mut scene.quads);
    if screen.commands.list().is_some() {
        suggestion_list(size, font, screen, &mut scene.quads);
    } else {
        hint_lines(size, font, &language, screen, &mut scene.quads);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::font::corpus_font;

    mod jar {
        use super::*;
        use mcrs_minecraft_protocol::text::Text;

        #[test]
        fn the_newest_line_sits_forty_units_above_the_bottom_and_is_gone_after_ten_seconds() {
            let mut chat = Chat::default();
            let font = corpus_font();
            chat.log
                .push(Text::text("hi"), 0, None, false, |text, rows| {
                    rows.push(text_spans(text, Language::corpus(), 0xFFFF_FFFF));
                });
            let size = GuiSize {
                width: 400,
                height: 240,
                scale: 2,
            };
            let mut out = Vec::new();
            chat.tick = 20;
            chat_lines(size, &chat, false, 0.5, &mut out, font);
            assert_eq!(
                out[0],
                GuiQuad::Fill {
                    rect: IRect::new(0, 191, 332, 200),
                    color: 0x7F00_0000
                }
            );
            assert!(matches!(
                out[1],
                GuiQuad::Glyph { origin, glyph: 'h', .. } if origin == IVec2::new(5, 193)
            ));
            out.clear();
            chat.tick = 200;
            chat_lines(size, &chat, false, 0.5, &mut out, font);
            assert!(out.is_empty());
        }
    }
}
