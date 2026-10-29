//! The chat on the GUI: the lines of the log, the overlay message and the open chat screen.

use bevy::math::{IRect, IVec2};
use bevy::prelude::*;
use bevy::time::Real;
use mcrs_minecraft_chat::{ChatScreen, Highlight, MAX_INPUT, Tag, Tooltip as SuggestionTooltip};

use super::chat_suggestions;
use super::font::{self, Font, Hover, Span};
use super::item_decorations::WHITE;
use super::language::Language;
use super::scene::{self, GuiAtlas, GuiConfig, GuiQuad, GuiScene, GuiSize};
use super::text::{self, Wrap};
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
const INPUT_FILL: u32 = 0x8000_0000;
const INPUT_TEXT: u32 = 0xFFE0_E0E0;
pub(super) const GRAY: u32 = 0xFFAA_AAAA;
pub(super) const RED: u32 = 0xFFFF_5555;
const GHOST: u32 = 0xFF80_8080;
const ARGUMENT_COLOURS: [u32; 5] = [
    0xFF55_FFFF,
    0xFFFF_FF55,
    0xFF55_FF55,
    0xFFFF_55FF,
    0xFFFF_AA00,
];
const CARET_BLINK_MS: u128 = 300;
const TAG_TOOLTIP_WIDTH: i32 = 210;
const MODIFIED_ICON: &str = "icon/chat_modified";

/// Whether the chat screen is open, which sets the height of the chat view.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChatFocus {
    Focused,
    Unfocused,
}

/// The lines of the chat view: 20 open, 10 closed (CHAT-LOG-004).
pub fn per_page(focus: ChatFocus) -> usize {
    let height = match focus {
        ChatFocus::Focused => FOCUSED_HEIGHT,
        ChatFocus::Unfocused => UNFOCUSED_HEIGHT,
    };
    ((height * 160.0 + 20.0).floor() as i32 / LINE) as usize
}

pub(crate) fn wrap_width(tag: Option<Tag>) -> i32 {
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

fn holds(rect: IRect, point: IVec2) -> bool {
    rect.min.x <= point.x && point.x < rect.max.x && rect.min.y <= point.y && point.y < rect.max.y
}

/// The mouse over the open chat, and where the log makes its tooltip requests.
pub(crate) struct Pointer<'a> {
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
            text::wrap(&spans, TAG_TOOLTIP_WIDTH, Wrap::Tooltip, font, rows);
        });
    }
}

/// The lines of the chat log (CHAT-LOG-002 to CHAT-LOG-005) at the default options. With a
/// pointer, the chat is open: the tag bars and icons make their tooltip requests, and the
/// result is the hovered text style (CHAT-LOG-007).
pub(crate) fn chat_lines<'c>(
    size: GuiSize,
    chat: &Chat,
    lines: &'c mcrs_minecraft_chat::ChatLines<Vec<Span>>,
    mut pointer: Option<Pointer<'_>>,
    out: &mut Vec<GuiQuad>,
    font: &Font,
) -> Option<&'c Hover> {
    let open = pointer.is_some();
    let bottom = size.height - BOTTOM;
    let page = per_page(if open {
        ChatFocus::Focused
    } else {
        ChatFocus::Unfocused
    });
    let text_origin = |slot: usize| IVec2::new(4, bottom - slot as i32 * LINE - TEXT_OFFSET);
    let (hovered, hovered_message) = pointer.as_ref().map_or((None, None), |pointer| {
        hovered_text(lines, page, text_origin, pointer.at, font)
    });
    let mut message = 0;
    let mut shown = 0;
    for (slot, line) in lines.visible(page) {
        message += usize::from(line.last);
        shown += 1;
        let alpha = if open {
            1.0
        } else {
            mcrs_minecraft_chat::fade(chat.tick.saturating_sub(line.tick))
        };
        if alpha <= 1e-5 {
            continue;
        }
        let slot_bottom = bottom - slot as i32 * LINE;
        out.push(GuiQuad::Fill {
            rect: IRect::new(0, slot_bottom - LINE, CHAT_WIDTH + 12, slot_bottom),
            color: with_alpha(0xFF00_0000, alpha * chat.options.text_background_opacity),
        });
        let origin = text_origin(slot);
        font::draw_tinted_text(
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
                out.push(scene::sprite(icon.min, pointer.icon, MODIFIED_ICON));
            }
        }
    }
    if open {
        scroll_bar(lines, shown, bottom, out);
    }
    hovered
}

/// The hover event under `at` and the number of the message that holds it (CHAT-LOG-007).
fn hovered_text<'c>(
    lines: &'c mcrs_minecraft_chat::ChatLines<Vec<Span>>,
    page: usize,
    text_origin: impl Fn(usize) -> IVec2,
    at: IVec2,
    font: &Font,
) -> (Option<&'c Hover>, Option<usize>) {
    let mut hovered = None;
    let mut hovered_message = None;
    let mut message = 0;
    for (slot, line) in lines.visible(page) {
        message += usize::from(line.last);
        if let Some(span) = font::span_at(font, text_origin(slot), &line.content, at) {
            hovered_message = Some(message);
            hovered = span.hover.as_deref().or(hovered);
        }
    }
    (hovered, hovered_message)
}

/// The scroll bar of the open chat when the log holds more lines than it shows.
fn scroll_bar(
    lines: &mcrs_minecraft_chat::ChatLines<Vec<Span>>,
    shown: usize,
    bottom: i32,
    out: &mut Vec<GuiQuad>,
) {
    let count = lines.lines().len();
    if count > shown && count > 0 {
        let visible = shown as i32 * LINE;
        let total = count as i32 * LINE;
        let q = lines.scroll() as i32 * visible / count as i32;
        let h = visible * visible / total;
        let alpha = if q > bottom { 170 } else { 96 };
        let colour = if lines.new_since_scroll() {
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
            let spans = text::text_spans(value, language, WHITE);
            text::wrap(&spans, (gui_width / 2).max(200), Wrap::Tooltip, font, rows);
        }
        // chisle: `show_item` shows the item tooltip of the stack, which has no specification
        // yet; it shows nothing until one states its lines.
        Hover::ShowItem(_) => {}
        Hover::ShowEntity { id, uuid, name } => {
            if !advanced {
                return;
            }
            if let Some(name) = name {
                rows.push(text::text_spans(name, language, WHITE));
            }
            let key = format!("entity.{}.{}", id.namespace(), id.path().replace('/', "."));
            let kind = vec![Span::new(language.get(&key), WHITE)];
            rows.push(language.translate("gui.entity_tooltip.type", WHITE, &[kind]));
            rows.push(vec![Span::new(uuid.to_string(), WHITE)]);
        }
    }
}

/// Calls `run` for each coloured part of the input (CCMD-HL-001), or once for chat text and a draft.
fn for_each_input_run(screen: &ChatScreen, mut run: impl FnMut(&str, u32)) {
    let input = screen.input();
    if screen.is_draft() {
        return run(input, GRAY);
    }
    if !screen.commands.is_command() || screen.commands.input() != input {
        return run(input, INPUT_TEXT);
    }
    let mut at = 0;
    for &(span, highlight) in screen.commands.highlights() {
        if span.start > at {
            run(input.get(at..span.start).unwrap_or_default(), GRAY);
        }
        let colour = match highlight {
            Highlight::Argument(index) => ARGUMENT_COLOURS[usize::from(index) % 5],
            Highlight::Trailing => RED,
        };
        run(input.get(span.start..span.end).unwrap_or_default(), colour);
        at = span.end.max(at);
    }
    if at < input.len() {
        run(input.get(at..).unwrap_or_default(), GRAY);
    }
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
    let mut width = font.str_width(input.get(..caret).unwrap_or_default());
    for ch in input.get(..caret).unwrap_or_default().chars() {
        if width <= visible {
            break;
        }
        width -= font.advance(ch, false);
        skip += ch.len_utf8();
    }
    let y = size.height - 12;
    let mut x = 4;
    let mut start = 0;
    for_each_input_run(screen, |text, color| {
        let cut = skip.saturating_sub(start).min(text.len());
        start += text.len();
        x = font::draw_str(
            font,
            IVec2::new(x, y),
            text.get(cut..).unwrap_or_default(),
            color,
            false,
            out,
        );
    });
    let caret_x = 4 + font.str_width(input.get(skip..caret).unwrap_or_default());
    let at_end = caret == input.len();
    if at_end
        && mcrs_text_span::utf16_len(input) < MAX_INPUT
        && let Some(ghost) = screen.commands.list().and_then(|list| list.ghost(input))
    {
        font::draw_str(font, IVec2::new(caret_x - 1, y), ghost, GHOST, false, out);
    }
    if caret_on {
        if at_end {
            font::draw_str(font, IVec2::new(caret_x, y), "_", INPUT_TEXT, false, out);
        } else {
            out.push(GuiQuad::Fill {
                rect: IRect::new(caret_x, y - 1, caret_x + 1, y + 10),
                color: 0xFFD0_D0D0,
            });
        }
    }
}

/// Draws the chat log and the overlay message, and the input box, the suggestions, the hints
/// and the tooltips of the open chat screen.
pub fn draw(
    (chat, lines): (Res<Chat>, Res<super::chat_input::ChatLines>),
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
        icon: atlas.0.chat_modified.size(),
    });
    let hovered = chat_lines(size, &chat, &lines.0, pointer, &mut scene.quads, font);
    if let Some((text, alpha)) = chat.overlay.shown(0.0) {
        let spans = text::text_spans(text, &language, u32::from(alpha) << 24 | 0x00FF_FFFF);
        font::draw_centered_text(
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
    if let Some((list, rows)) = screen
        .commands
        .list()
        .zip(chat_suggestions::list_rows(size, font, screen))
    {
        chat_suggestions::suggestion_list(font, list, &rows, &mut scene.quads);
        let tip = chat_suggestions::row_at(rows.rect, mouse)
            .and_then(|_| list.entries().tooltip(list.selected()));
        if let Some(tip) = tip {
            scene.tooltip.request(mouse, |rows| {
                rows.push(match tip {
                    SuggestionTooltip::Key(key) => vec![Span::new(language.get(key), WHITE)],
                    SuggestionTooltip::Text(text) => text::text_spans(text, &language, WHITE),
                });
            });
        }
    } else {
        chat_suggestions::hint_lines(size, font, &language, screen, &mut scene.quads);
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
            let mut lines = mcrs_minecraft_chat::ChatLines::default();
            chat.log.push(Text::text("hi"), 0, None);
            lines.sync(&chat.log, None, |text, _, rows| {
                rows.push(text::text_spans(text, Language::corpus(), 0xFFFF_FFFF));
            });
            let size = GuiSize {
                width: 400,
                height: 240,
                scale: 2,
            };
            let mut out = Vec::new();
            chat.tick = 20;
            chat_lines(size, &chat, &lines, None, &mut out, font);
            assert_eq!(
                out[0],
                GuiQuad::Fill {
                    rect: IRect::new(0, 191, 332, 200),
                    color: 0x7F00_0000
                }
            );
            let (chat_bottom, text_offset, frame_x) = (size.height - 40, 8, 4);
            assert!(
                out.contains(&GuiQuad::Glyph {
                    origin: IVec2::new(frame_x, chat_bottom - text_offset),
                    glyph: 'h',
                    color: 0xFFFF_FFFF,
                }),
                "{out:?}"
            );
            out.clear();
            chat.tick = 200;
            chat_lines(size, &chat, &lines, None, &mut out, font);
            assert!(out.is_empty());
        }
    }
}
