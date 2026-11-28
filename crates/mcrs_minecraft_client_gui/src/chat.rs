use bevy::math::{IRect, IVec2};
use bevy::prelude::*;
use bevy::time::Real;
use mcrs_minecraft_chat_client::{
    ARGUMENT_COLOURS, ChatScreen, Highlight, Tag, Tooltip as SuggestionTooltip,
};
use mcrs_minecraft_client_widget::{EditBoxFrame, EditBoxRun};

use crate::chat_suggestions;
use crate::chat_view::{
    BOTTOM, CHAT_WIDTH, ChatFocus, GRAY, LINE, RED, input_bounds, per_page, text_origin, text_under,
};
use crate::draw_gate::layer_stale;
use crate::edit_box_draw;
use crate::gui_atlas_layout::GuiAtlasLayout;
use crate::gui_band::GuiBand;
use crate::gui_config::GuiConfig;
use crate::gui_layer::{GuiLayer, spawn_layer};
use crate::gui_phase::GuiPhase;
use crate::gui_pointer::GuiPointer;
use crate::gui_quad::GuiQuad;
use crate::gui_quad::WHITE;
use crate::gui_size::GuiSize;
use crate::irect;
use crate::layer_due;
use crate::text_draw;
use crate::tooltip::Tooltip;
use mcrs_minecraft_client_world::GuiTick;
use mcrs_minecraft_client_world::chat::{Chat, ChatInput};
use mcrs_minecraft_client_world::screen::Screen;
use mcrs_minecraft_font::{ClientLanguage, Font, Hover, Span, SpanEvents, Wrap, text_spans, wrap};

const INPUT_FILL: u32 = 0x8000_0000;
const INPUT_TEXT: u32 = 0xFFE0_E0E0;
const ARGUMENT_PALETTE: [u32; ARGUMENT_COLOURS as usize] = [
    0xFF55_FFFF,
    0xFFFF_FF55,
    0xFF55_FF55,
    0xFFFF_55FF,
    0xFFFF_AA00,
];
const TAG_TOOLTIP_WIDTH: i32 = 210;
const MODIFIED_ICON: &str = "icon/chat_modified";

fn with_alpha(color: u32, alpha: f32) -> u32 {
    let base = f32::from((color >> 24) as u8);
    ((base * alpha) as u32) << 24 | (color & 0x00FF_FFFF)
}

pub(crate) struct ChatPointer<'a> {
    pub at: IVec2,
    pub language: &'a ClientLanguage,
    pub tooltip: &'a mut Tooltip,
    pub icon: IVec2,
}

impl ChatPointer<'_> {
    fn tag_tooltip(&mut self, tag: Tag, font: &Font) {
        let language = self.language;
        self.tooltip.request(self.at, |rows| {
            let spans = [Span::new(language.get(tag.tooltip_key()), WHITE)];
            wrap(&spans, TAG_TOOLTIP_WIDTH, Wrap::Tooltip, font, rows);
        });
    }
}

#[derive(Clone, Copy)]
pub(crate) struct LineFade {
    pub now: GuiTick,
    pub background_opacity: f32,
}

pub(crate) fn chat_lines<'c>(
    size: GuiSize,
    fade: LineFade,
    lines: &'c mcrs_minecraft_chat_client::ChatLines<Vec<Span>>,
    events: &'c SpanEvents,
    mut pointer: Option<ChatPointer<'_>>,
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
            mcrs_minecraft_chat_client::fade(fade.now.0.saturating_sub(line.tick))
        };
        if alpha <= 1e-5 {
            continue;
        }
        let slot_bottom = bottom - slot as i32 * LINE;
        out.push(GuiQuad::Fill {
            rect: IRect::new(0, slot_bottom - LINE, CHAT_WIDTH + 12, slot_bottom),
            color: with_alpha(0xFF00_0000, alpha * fade.background_opacity),
        });
        let origin = text_origin(slot);
        text_draw::draw_tinted_text(
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
                out.push(crate::gui_quad::sprite(
                    icon.min,
                    pointer.icon,
                    MODIFIED_ICON,
                ));
            }
        }
    }
    if open {
        scroll_bar(lines, shown, bottom, out);
    }
    hovered
}

fn scroll_bar(
    lines: &mcrs_minecraft_chat_client::ChatLines<Vec<Span>>,
    shown: usize,
    bottom: i32,
    out: &mut Vec<GuiQuad>,
) {
    let count = lines.lines().len();
    if count > shown {
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
            let spans = text_spans(value, language, WHITE);
            wrap(&spans, (gui_width / 2).max(200), Wrap::Tooltip, font, rows);
        }
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

fn input_box(
    size: GuiSize,
    atlas: &GuiAtlasLayout,
    font: &Font,
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
    runs.clear();
    input_runs(screen, runs);
    screen.edit().draw(
        EditBoxFrame {
            suggestion: screen.commands.list().and_then(|list| list.ghost(input)),
            ..EditBoxFrame::new(input_bounds(size), now_ms)
        },
        &|text| font.str_width(text),
        runs,
        &mut |part| edit_box_draw::draw_edit_box_part(atlas, font, part, out),
    );
}

#[cfg_attr(mcrs_lint, mcrs::alloc_sites(27))]
pub fn draw(
    (chat, lines): (Res<Chat>, Res<crate::chat_input::ChatLines>),
    tick: Res<GuiTick>,
    input: Res<ChatInput>,
    screen: Res<Screen>,
    (language, atlas, font): (
        Option<Res<ClientLanguage>>,
        Option<Res<GuiAtlasLayout>>,
        Option<Res<Font>>,
    ),
    time: Res<Time<Real>>,
    config: Res<GuiConfig>,
    size: Res<GuiSize>,
    pointer: Res<GuiPointer>,
    mut layer: Single<(&mut GuiLayer, &mut Tooltip), With<ChatLayer>>,
    mut runs: Local<Vec<EditBoxRun>>,
) {
    let (layer, tooltip) = &mut *layer;
    let out = layer.redraw();
    tooltip.clear();
    let (Some(atlas), Some(font), Some(language)) = (atlas, font, language) else {
        return;
    };
    let font = &*font;
    let size = *size;
    let mouse = irect::point(pointer.point());
    let open = *screen == Screen::Chat;
    let pointer = open.then(|| ChatPointer {
        at: mouse,
        language: &language,
        tooltip: &mut *tooltip,
        icon: atlas.chat_modified.size(),
    });
    let hovered = chat_lines(
        size,
        LineFade {
            now: *tick,
            background_opacity: chat.options.text_background_opacity,
        },
        &lines.lines,
        &lines.events,
        pointer,
        out,
        font,
    );
    if let Some((text, alpha)) = chat.overlay.shown(tick.0, 0.0) {
        let spans = text_spans(text, &language, u32::from(alpha) << 24 | 0x00FF_FFFF);
        text_draw::draw_centered_text(font, size.width / 2, size.height - 72, &spans, out);
    }
    if !open {
        return;
    }
    let screen = &input.0;
    let now_ms = layer_due::gui_clock(&time);
    let blink = screen.edit().next_blink_ms(now_ms);
    input_box(size, &atlas, font, screen, now_ms, &mut runs, out);
    if let Some((list, rows)) = screen
        .commands
        .list()
        .zip(chat_suggestions::list_rows(size, font, screen))
    {
        chat_suggestions::suggestion_list(font, list, &rows, out);
        let tip =
            chat_suggestions::row_at(rows.rect, mouse).and(list.entries().tooltip(list.selected()));
        if let Some(tip) = tip {
            tooltip.request(mouse, |rows| {
                rows.push(match tip {
                    SuggestionTooltip::Key(key) => vec![Span::new(language.get(key), WHITE)],
                    SuggestionTooltip::Text(text) => text_spans(text, &language, WHITE),
                });
            });
        }
    } else {
        chat_suggestions::hint_lines(size, font, &language, screen, out);
    }
    if let Some(hover) = hovered {
        tooltip.request(mouse, |rows| {
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
    if let Some(at) = blink {
        layer.redraw_at(at);
    }
}

#[cfg_attr(mcrs_lint, mcrs::event_condition)]
fn chat_fading(
    tick: Res<GuiTick>,
    chat: Res<Chat>,
    lines: Res<crate::chat_input::ChatLines>,
) -> bool {
    tick.is_changed()
        && (lines
            .lines
            .lines()
            .any(|line| mcrs_minecraft_chat_client::fades_at(tick.0.saturating_sub(line.tick)))
            || chat.overlay.fades_at(tick.0))
}

#[derive(Component, Default)]
#[require(GuiLayer = GuiLayer::new(GuiBand::Hud, 2), Tooltip)]
pub struct ChatLayer;

pub struct ChatDrawPlugin;

impl Plugin for ChatDrawPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_layer::<ChatLayer>)
            .add_systems(
                PostUpdate,
                draw.in_set(GuiPhase::Draw).run_if(
                    layer_stale::<ChatLayer>()
                        .or_else(resource_changed::<Chat>)
                        .or_else(resource_changed::<crate::chat_input::ChatLines>)
                        .or_else(resource_changed::<ChatInput>)
                        .or_else(resource_changed::<Screen>)
                        .or_else(resource_changed::<GuiPointer>)
                        .or_else(resource_changed::<GuiConfig>)
                        .or_else(resource_exists_and_changed::<ClientLanguage>)
                        .or_else(chat_fading),
                ),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus_font::corpus_font;
    use crate::corpus_language::corpus_language;
    use mcrs_minecraft_font::{Weight, row_texts};

    #[test]
    fn a_closed_chat_redraws_only_on_the_ticks_where_a_line_fades() {
        use bevy::ecs::system::RunSystemOnce;
        use mcrs_minecraft_chat_client::ChatLog;

        let mut log = ChatLog::<u8>::default();
        log.push(0, 0, None);
        let mut lines = crate::chat_input::ChatLines::default();
        lines
            .lines
            .sync(&log, None, |_, _, rows| rows.push(Vec::new()));
        let mut world = World::new();
        world.insert_resource(Chat::default());
        world.insert_resource(lines);
        let mut fading = |age: u64| {
            world.insert_resource(GuiTick(age));
            world.run_system_once(chat_fading).unwrap()
        };
        assert!(!fading(100));
        assert!(fading(185));
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
                    corpus_language(),
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
                rows.push(text_spans(text, corpus_language(), 0xFFFF_FFFF));
            });
            let size = GuiSize {
                width: 400,
                height: 240,
                scale: 2,
            };
            let mut out = Vec::new();
            chat_lines(
                size,
                LineFade {
                    now: GuiTick(20),
                    background_opacity: chat.options.text_background_opacity,
                },
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
            chat_lines(
                size,
                LineFade {
                    now: GuiTick(200),
                    background_opacity: chat.options.text_background_opacity,
                },
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
