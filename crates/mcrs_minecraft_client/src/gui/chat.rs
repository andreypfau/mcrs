use bevy::math::{IRect, IVec2};
use bevy::prelude::*;
use bevy::time::Real;
use mcrs_minecraft_chat::{ChatScreen, Highlight, Hint, Tag, Tooltip as SuggestionTooltip, fade};
use mcrs_minecraft_command::parse_error_line;

use super::font::{Font, Hover, Span, draw_centered_text, draw_text, draw_tinted_text, span_at};
use super::language::Language;
use super::scene::{GuiAtlas, GuiConfig, GuiQuad, GuiScene, GuiSize, sprite};
use super::text::{Wrap, text_spans, wrap};
use super::tooltip::Tooltip;
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
const WHITE: u32 = 0xFFFF_FFFF;
const TAG_TOOLTIP_WIDTH: i32 = 210;
const MODIFIED_ICON: &str = "icon/chat_modified";
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

fn holds(rect: IRect, point: IVec2) -> bool {
    rect.min.x <= point.x && point.x < rect.max.x && rect.min.y <= point.y && point.y < rect.max.y
}

/// The mouse over the open chat, and where the log makes its tooltip requests.
pub struct Pointer<'a> {
    pub at: IVec2,
    pub language: &'a Language,
    pub tooltip: &'a mut Tooltip,
    pub icon: IVec2,
}

impl Pointer<'_> {
    fn tag_tooltip(&mut self, tag: Tag, font: &Font) {
        let language = self.language;
        self.tooltip.request(self.at, |rows| {
            // chisle: a `Modified` tooltip ends with the original text in gray (CHAT-LOG-005),
            // which the log does not keep; no message is tagged `Modified` yet.
            let spans = [Span::new(language.get(tag.tooltip_key()), WHITE)];
            wrap(&spans, TAG_TOOLTIP_WIDTH, Wrap::Tooltip, font, rows);
        });
    }
}

/// The lines of the chat log (CHAT-LOG-002 to CHAT-LOG-005) at the default options. With a
/// pointer, the chat is open: the tag bars and icons make their tooltip requests, and the
/// result is the hovered text style (CHAT-LOG-007).
pub fn chat_lines<'c>(
    size: GuiSize,
    chat: &'c Chat,
    background: f32,
    mut pointer: Option<Pointer<'_>>,
    out: &mut Vec<GuiQuad>,
    font: &Font,
) -> Option<&'c Hover> {
    let open = pointer.is_some();
    let bottom = size.height - BOTTOM;
    let page = per_page(open);
    let text_origin = |slot: usize| IVec2::new(4, bottom - slot as i32 * LINE - TEXT_OFFSET);
    let mut hovered = None;
    let mut hovered_message = None;
    if let Some(pointer) = &pointer {
        let mut message = 0;
        for (slot, line) in chat.log.visible(page) {
            message += usize::from(line.last);
            if let Some(span) = span_at(font, text_origin(slot), &line.content, pointer.at) {
                hovered_message = Some(message);
                hovered = span.hover.as_deref().or(hovered);
            }
        }
    }
    let mut message = 0;
    let mut shown = 0;
    for (slot, line) in chat.log.visible(page) {
        message += usize::from(line.last);
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
        let origin = text_origin(slot);
        draw_tinted_text(
            font,
            origin,
            &line.content,
            |color| with_alpha(color, alpha),
            out,
        );
        let Some(tag) = line.tag else {
            continue;
        };
        let bar = IRect::new(0, slot_bottom - LINE, 2, slot_bottom);
        out.push(GuiQuad::Fill {
            rect: bar,
            color: with_alpha(0xFF00_0000 | tag.indicator(), alpha),
        });
        let Some(pointer) = pointer.as_mut() else {
            continue;
        };
        if holds(bar, pointer.at) {
            pointer.tag_tooltip(tag, font);
        }
        if tag.has_icon() && line.last {
            let left = origin.x + font.width(&line.content) + 4;
            let icon = IRect::new(left, origin.y - 1, left + pointer.icon.x, origin.y + 9);
            let over = holds(icon, pointer.at);
            if over {
                pointer.tag_tooltip(tag, font);
            }
            if over || hovered_message == Some(message) {
                out.push(sprite(icon.min, pointer.icon, MODIFIED_ICON));
            }
        }
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
    hovered
}

/// The lines a hover event shows (TIP-HOVER-001).
fn hover_lines(
    hover: &Hover,
    gui_width: i32,
    advanced: bool,
    language: &Language,
    font: &Font,
    rows: &mut Vec<Vec<Span>>,
) {
    match hover {
        Hover::ShowText { value } => {
            let spans = text_spans(value, language, WHITE);
            wrap(&spans, (gui_width / 2).max(200), Wrap::Tooltip, font, rows);
        }
        // chisle: `show_item` shows the item tooltip of the stack, which has no specification
        // yet; it shows nothing until one states its lines.
        Hover::ShowItem(_) => {}
        Hover::ShowEntity { id, uuid, name } => {
            if !advanced {
                return;
            }
            if let Some(name) = name {
                rows.push(text_spans(name, language, WHITE));
            }
            let key = format!("entity.{}.{}", id.namespace(), id.path().replace('/', "."));
            let kind = vec![Span::new(language.get(&key), WHITE)];
            rows.push(language.translate("gui.entity_tooltip.type", WHITE, &[kind]));
            rows.push(vec![Span::new(uuid.to_string(), WHITE)]);
        }
    }
}

/// The left x of the suggestion list or the hint lines over `start` (CCMD-LIST-002).
fn column(size: GuiSize, font: &Font, input: &str, start: usize, width: i32) -> i32 {
    let x0 = input
        .get(..start)
        .map_or(4, |before| 4 + text_width(font, before));
    x0.clamp(0, (size.width - width).max(0))
}

/// The row under `point`, with the edge pixels of each row excluded (CCMD-LIST-007, step 1).
pub fn row_at(rect: IRect, point: IVec2) -> Option<usize> {
    let dy = point.y - rect.min.y;
    let inside = rect.min.x < point.x && point.x < rect.max.x && 0 < dy && point.y < rect.max.y;
    (inside && dy % ROW != 0).then_some((dy / ROW) as usize)
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
    (language, atlas): (Option<Res<Language>>, Option<Res<GuiAtlas>>),
    time: Res<Time<Real>>,
    config: Res<GuiConfig>,
    mut scene: ResMut<GuiScene>,
) {
    let (Some(atlas), Some(language)) = (atlas, language) else {
        return;
    };
    let font = &atlas.0.font;
    let scene = &mut *scene;
    let size = scene.size;
    let mouse = scene.cursor;
    let open = *screen == Screen::Chat;
    let pointer = open.then(|| Pointer {
        at: mouse,
        language: &language,
        tooltip: &mut scene.tooltip,
        icon: atlas.0.region(MODIFIED_ICON).size(),
    });
    let hovered = chat_lines(
        size,
        &chat,
        chat.options.text_background_opacity,
        pointer,
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
    if let Some(list) = screen.commands.list() {
        suggestion_list(size, font, screen, &mut scene.quads);
        let tip = list_rows(size, font, screen)
            .and_then(|rows| row_at(rows.rect, mouse))
            .and_then(|_| list.entries().tooltip(list.selected()));
        if let Some(tip) = tip {
            scene.tooltip.request(mouse, |rows| {
                rows.push(match tip {
                    SuggestionTooltip::Key(key) => vec![Span::new(language.get(key), WHITE)],
                    SuggestionTooltip::Text(text) => text_spans(text, &language, WHITE),
                });
            });
        }
    } else {
        hint_lines(size, font, &language, screen, &mut scene.quads);
    }
    if let Some(hover) = hovered {
        scene.tooltip.request(mouse, |rows| {
            hover_lines(
                hover,
                size.width,
                config.advanced_tooltips,
                &language,
                font,
                rows,
            );
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::font::corpus_font;
    use crate::gui::text::row_texts;

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

    mod jar {
        use super::*;

        #[test]
        fn an_entity_hover_shows_its_type_and_uuid_only_with_advanced_tooltips() {
            let lines = |json: &str, advanced: bool| {
                let hover: Hover = serde_json::from_str(json).unwrap();
                let mut rows = Vec::new();
                hover_lines(
                    &hover,
                    427,
                    advanced,
                    Language::corpus(),
                    corpus_font(),
                    &mut rows,
                );
                row_texts(&rows)
            };
            let pig = r#"{"action": "show_entity", "id": "minecraft:pig",
                "uuid": "00000000-0000-0000-0000-000000000001"}"#;
            assert_eq!(
                lines(pig, true),
                ["Type: Pig", "00000000-0000-0000-0000-000000000001"]
            );
            assert!(lines(pig, false).is_empty());
            let nested = r#"{"action": "show_entity", "id": "example:a/b", "name": "Bob",
                "uuid": "00000000-0000-0000-0000-000000000002"}"#;
            assert_eq!(
                lines(nested, true),
                [
                    "Bob",
                    "Type: entity.example.a.b",
                    "00000000-0000-0000-0000-000000000002"
                ]
            );
        }
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
            chat_lines(size, &chat, 0.5, None, &mut out, font);
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
            chat_lines(size, &chat, 0.5, None, &mut out, font);
            assert!(out.is_empty());
        }
    }
}
