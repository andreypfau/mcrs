use bevy::math::{IRect, IVec2};
use bevy::prelude::*;
use bevy::time::Real;
use mcrs_minecraft_chat_client::{
    ARGUMENT_COLOURS, ChatScreen, Highlight, Tag, Tooltip as SuggestionTooltip,
};
use mcrs_minecraft_client_widget::{EditBoxFrame, EditBoxRun, GuiRect};

use crate::chat_suggestions;
use crate::edit_box_draw;
use crate::font::{self, Font, Hover, Span};
use crate::item_decorations::WHITE;
use crate::language::ClientLanguage;
use crate::menu_input;
use crate::scene::{self, GuiAtlas, GuiAtlasData, GuiConfig, GuiQuad, GuiScene, GuiSize};
use crate::span_events::SpanEvents;
use crate::text::{self, Wrap};
use crate::tooltip::Tooltip;
use mcrs_minecraft_client_world::chat::{Chat, ChatInput};
use mcrs_minecraft_client_world::screen::Screen;

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
const ARGUMENT_PALETTE: [u32; ARGUMENT_COLOURS as usize] = [
    0xFF55_FFFF,
    0xFFFF_FF55,
    0xFF55_FF55,
    0xFFFF_55FF,
    0xFFFF_AA00,
];
const TAG_TOOLTIP_WIDTH: i32 = 210;
const MODIFIED_ICON: &str = "icon/chat_modified";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChatFocus {
    Focused,
    Unfocused,
}

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

pub(crate) struct Pointer<'a> {
    pub at: IVec2,
    pub language: &'a ClientLanguage,
    pub tooltip: &'a mut Tooltip,
    pub icon: IVec2,
}

impl Pointer<'_> {
    fn tag_tooltip(&mut self, tag: Tag, font: &Font) {
        let language = self.language;
        self.tooltip.request(self.at, |rows| {
            let spans = [Span::new(language.get(tag.tooltip_key()), WHITE)];
            text::wrap(&spans, TAG_TOOLTIP_WIDTH, Wrap::Tooltip, font, rows);
        });
    }
}

pub(crate) fn chat_lines<'c>(
    size: GuiSize,
    chat: &Chat,
    lines: &'c mcrs_minecraft_chat_client::ChatLines<Vec<Span>>,
    events: &'c SpanEvents,
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
    let text_origin = |slot: usize| text_origin(size, slot);
    let (hovered, hovered_message) = pointer.as_ref().map_or((None, None), |pointer| {
        text_under(lines, page, text_origin, pointer.at, font).map_or((None, None), |under| {
            (
                under.span.hover.and_then(|index| events.hover(index)),
                Some(under.message),
            )
        })
    });
    let mut message = 0;
    let mut shown = 0;
    for (slot, line) in lines.visible(page) {
        message += usize::from(line.last);
        shown += 1;
        let alpha = if open {
            1.0
        } else {
            mcrs_minecraft_chat_client::fade(chat.tick.saturating_sub(line.tick))
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
        if bar.contains(pointer.at) {
            pointer.tag_tooltip(tag, font);
        }
        if tag.has_icon() && line.last {
            let left = origin.x + font.width(&line.content) + 4;
            let icon = IRect::new(left, origin.y - 1, left + pointer.icon.x, origin.y + 9);
            let over = icon.contains(pointer.at);
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

fn text_origin(size: GuiSize, slot: usize) -> IVec2 {
    IVec2::new(4, size.height - BOTTOM - slot as i32 * LINE - TEXT_OFFSET)
}

struct TextUnder<'c> {
    span: &'c Span,
    message: usize,
}

fn text_under<'c>(
    lines: &'c mcrs_minecraft_chat_client::ChatLines<Vec<Span>>,
    page: usize,
    origin: impl Fn(usize) -> IVec2,
    at: IVec2,
    font: &Font,
) -> Option<TextUnder<'c>> {
    let mut message = 0;
    for (slot, line) in lines.visible(page) {
        message += usize::from(line.last);
        if let Some(span) = font::span_at(font, origin(slot), &line.content, at) {
            return Some(TextUnder { span, message });
        }
    }
    None
}

pub(crate) fn open_chat_span_at<'c>(
    size: GuiSize,
    lines: &'c mcrs_minecraft_chat_client::ChatLines<Vec<Span>>,
    at: IVec2,
    font: &Font,
) -> Option<&'c Span> {
    let page = per_page(ChatFocus::Focused);
    text_under(lines, page, |slot| text_origin(size, slot), at, font).map(|under| under.span)
}

fn scroll_bar(
    lines: &mcrs_minecraft_chat_client::ChatLines<Vec<Span>>,
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

fn hover_lines(
    hover: &Hover,
    gui_width: i32,
    advanced: bool,
    language: &ClientLanguage,
    font: &Font,
    rows: &mut Vec<Vec<Span>>,
) {
    match hover {
        Hover::ShowText { value } => {
            let spans = text::text_spans(value, language, WHITE);
            text::wrap(&spans, (gui_width / 2).max(200), Wrap::Tooltip, font, rows);
        }
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

fn input_runs(screen: &ChatScreen, runs: &mut Vec<EditBoxRun>) {
    let input = screen.input();
    if screen.is_draft() {
        return runs.push(EditBoxRun::new(0..input.len(), GRAY));
    }
    if !screen.commands.is_command() || screen.commands.input() != input {
        return runs.push(EditBoxRun::new(0..input.len(), INPUT_TEXT));
    }
    let mut at = 0;
    for &(span, highlight) in screen.commands.highlights() {
        if span.start > at {
            runs.push(EditBoxRun::new(at..span.start, GRAY));
        }
        let colour = match highlight {
            Highlight::Argument(index) => ARGUMENT_PALETTE
                .get(usize::from(index))
                .copied()
                .unwrap_or(GRAY),
            Highlight::Trailing => RED,
        };
        runs.push(EditBoxRun::new(span.start..span.end, colour));
        at = span.end.max(at);
    }
    if at < input.len() {
        runs.push(EditBoxRun::new(at..input.len(), GRAY));
    }
}

pub(crate) fn input_bounds(size: GuiSize) -> GuiRect {
    GuiRect::new(4, size.height - 12, (size.width - 8).max(0), 12)
}

fn input_box(
    size: GuiSize,
    atlas: &GuiAtlasData,
    screen: &ChatScreen,
    now_ms: u64,
    runs: &mut Vec<EditBoxRun>,
    out: &mut Vec<GuiQuad>,
) {
    out.push(GuiQuad::Fill {
        rect: IRect::new(2, size.height - 14, size.width - 2, size.height - 2),
        color: INPUT_FILL,
    });
    let input = screen.input();
    let font = &atlas.font;
    runs.clear();
    input_runs(screen, runs);
    screen.edit().draw(
        EditBoxFrame {
            suggestion: screen.commands.list().and_then(|list| list.ghost(input)),
            ..EditBoxFrame::new(input_bounds(size), now_ms)
        },
        &|text| font.str_width(text),
        runs,
        &mut |part| edit_box_draw::draw_edit_box_part(atlas, part, out),
    );
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::ungated_system,
        reason = "begin_gui_frame clears the scene on each frame, and the chat lines and the overlay fade with time, so the chat draws on each frame"
    )
)]
#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "the immediate-mode GUI takes owned text spans, and the fade of each line changes its colour on each frame, so a span cache would be rebuilt on each frame too"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(27))]
pub fn draw(
    (chat, lines): (Res<Chat>, Res<crate::chat_input::ChatLines>),
    input: Res<ChatInput>,
    screen: Res<Screen>,
    (language, atlas): (Option<Res<ClientLanguage>>, Option<Res<GuiAtlas>>),
    time: Res<Time<Real>>,
    config: Res<GuiConfig>,
    mut scene: ResMut<GuiScene>,
    mut runs: Local<Vec<EditBoxRun>>,
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
    let hovered = chat_lines(
        size,
        &chat,
        &lines.lines,
        &lines.events,
        pointer,
        &mut scene.quads,
        font,
    );
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
    let now_ms = menu_input::menu_clock(&time);
    input_box(size, &atlas.0, screen, now_ms, &mut runs, &mut scene.quads);
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
    use crate::font::corpus_font;
    use crate::font_glyph::Weight;
    use crate::text::row_texts;

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
                    ClientLanguage::corpus(),
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
        use mcrs_minecraft_protocol_game::text::Text;

        #[test]
        fn the_newest_line_sits_forty_units_above_the_bottom_and_is_gone_after_ten_seconds() {
            let mut chat = Chat::default();
            let font = corpus_font();
            let mut lines = mcrs_minecraft_chat_client::ChatLines::default();
            chat.log.push(Text::text("hi"), 0, None);
            lines.sync(&chat.log, None, |text, _, rows| {
                rows.push(text::text_spans(
                    text,
                    ClientLanguage::corpus(),
                    0xFFFF_FFFF,
                ));
            });
            let size = GuiSize {
                width: 400,
                height: 240,
                scale: 2,
            };
            let mut out = Vec::new();
            chat.tick = 20;
            chat_lines(
                size,
                &chat,
                &lines,
                &SpanEvents::default(),
                None,
                &mut out,
                font,
            );
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
                    pen: Vec2::new(frame_x as f32, (chat_bottom - text_offset) as f32),
                    glyph: 'h',
                    image: font.glyph('h').image.unwrap(),
                    color: 0xFFFF_FFFF,
                    weight: Weight::Regular,
                }),
                "{out:?}"
            );
            out.clear();
            chat.tick = 200;
            chat_lines(
                size,
                &chat,
                &lines,
                &SpanEvents::default(),
                None,
                &mut out,
                font,
            );
            assert!(out.is_empty());
        }
    }
}
