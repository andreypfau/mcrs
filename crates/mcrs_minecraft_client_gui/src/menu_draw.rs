use bevy::math::{IRect, IVec2};
use bevy::prelude::*;
use bevy::time::Real;
use mcrs_minecraft_client_widget::{
    ButtonSprite, CHECKBOX_SIZE, GuiPoint, GuiRect, LINE_HEIGHT, Overflow, TOOLTIP_WRAP_WIDTH,
    Widget, WidgetKind, WidgetPlacement, WidgetSprite, checkbox_label_origin,
};
use mcrs_minecraft_client_world::player::Player;

use crate::edit_box_draw::draw_edit_box;
use crate::font::{Font, draw_str};
use crate::gui_sprite_id::GuiSpriteId;
use crate::gui_texture_id::GuiTextureId;
use crate::menu::{Menu, MenuBackground};
use crate::menu_input::menu_clock;
use crate::scene::{GuiAtlas, GuiAtlasData, GuiQuad, GuiScene, GuiSize};
use crate::scroll_area_draw::{draw_scroll_area, draw_section_label_box};
use crate::tab_bar_draw::draw_tab_bar;
use crate::text::{self, Wrap};
use crate::tooltip::Tooltip;

const GRADIENT_TOP: u32 = 0xC010_1010;
const GRADIENT_BOTTOM: u32 = 0xD010_1010;
const BACKGROUND_TILE: i32 = 32;
const SEPARATOR_HEIGHT: i32 = 2;
const ELLIPSIS: &str = "...";

/// Whether the client is in a world, which picks the in-world textures of the background and
/// of the separators (WID-SCR-004, WID-SCR-006).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldPresence {
    NoWorld,
    InWorld,
}

/// The background, the separators and the visible widgets of `menu` at `now_ms`, in the order
/// of WID-SCR-007; the widgets of a scroll area draw with it (LIST-BG-003). A sprite the pack
/// lacks draws as the missing sprite.
pub fn draw_menu(
    menu: &Menu,
    world: WorldPresence,
    now_ms: u64,
    size: GuiSize,
    atlas: &GuiAtlasData,
    out: &mut Vec<GuiQuad>,
) {
    draw_background(menu, world, size, atlas, out);
    let shown = menu.widgets.iter().filter(|widget| {
        widget.visible && menu.widgets.placement(widget) == WidgetPlacement::Screen
    });
    for widget in shown {
        draw_widget(menu, widget, world, now_ms, atlas, out);
    }
}

pub(crate) fn draw_widget(
    menu: &Menu,
    widget: &Widget,
    world: WorldPresence,
    now_ms: u64,
    atlas: &GuiAtlasData,
    out: &mut Vec<GuiQuad>,
) {
    match widget.kind {
        WidgetKind::SelectionList(_) | WidgetKind::WidgetList(_) | WidgetKind::ScrollPanel(_) => {
            draw_scroll_area(menu, widget, world, now_ms, atlas, out);
        }
        WidgetKind::TabBar(_) => draw_tab_bar(menu, widget, now_ms, atlas, out),
        WidgetKind::SectionLabel => {
            draw_section_label_box(menu, widget, out);
            draw_caption(widget, now_ms, &atlas.font, out);
        }
        WidgetKind::EditBox(ref edit) => {
            draw_sprites(menu, widget, atlas, out);
            draw_edit_box(menu, widget, edit, now_ms, &atlas.font, out);
        }
        _ => {
            draw_sprites(menu, widget, atlas, out);
            draw_caption(widget, now_ms, &atlas.font, out);
        }
    }
}

/// Makes the tooltip request of the widgets of `menu` at `now_ms`, with the lines wrapped at the
/// width of a widget tooltip (WID-TIP-001, WID-TIP-002, WID-TIP-005). Call it once in each frame.
pub fn request_menu_tooltip(menu: &mut Menu, now_ms: u64, font: &Font, tooltip: &mut Tooltip) {
    let Some(request) = menu.widgets.tooltip(now_ms) else {
        return;
    };
    let Some(entry) = menu
        .tooltips
        .iter()
        .find(|entry| entry.widget == request.widget)
    else {
        return;
    };
    tooltip.request_widget(request, menu.widgets.focused(), |lines| {
        text::wrap(&entry.text, TOOLTIP_WRAP_WIDTH, Wrap::Tooltip, font, lines);
    });
}

pub(crate) fn draw(
    mut menu: ResMut<Menu>,
    atlas: Option<Res<GuiAtlas>>,
    player: Query<(), With<Player>>,
    time: Res<Time<Real>>,
    mut scene: ResMut<GuiScene>,
) {
    let Some(atlas) = atlas else {
        return;
    };
    let world = if player.is_empty() {
        WorldPresence::NoWorld
    } else {
        WorldPresence::InWorld
    };
    let now_ms = menu_clock(&time);
    let scene = &mut *scene;
    draw_menu(&menu, world, now_ms, scene.size, &atlas.0, &mut scene.quads);
    // The request writes only the hover start of the tooltips, which no other system reads.
    let menu = menu.bypass_change_detection();
    request_menu_tooltip(menu, now_ms, &atlas.0.font, &mut scene.tooltip);
}

fn draw_background(
    menu: &Menu,
    world: WorldPresence,
    size: GuiSize,
    atlas: &GuiAtlasData,
    out: &mut Vec<GuiQuad>,
) {
    let tab_bar = menu
        .widgets
        .iter()
        .find(|widget| widget.visible && matches!(widget.kind, WidgetKind::TabBar(_)));
    let top = tab_bar.map_or(0, |bar| bar.rect().y.saturating_add(bar.rect().height));
    let screen = IRect::new(0, top, size.width, size.height);
    let (background, header, footer) = match world {
        WorldPresence::NoWorld => (
            GuiTextureId::MenuBackground,
            GuiTextureId::HeaderSeparator,
            GuiTextureId::FooterSeparator,
        ),
        WorldPresence::InWorld => (
            GuiTextureId::InworldMenuBackground,
            GuiTextureId::InworldHeaderSeparator,
            GuiTextureId::InworldFooterSeparator,
        ),
    };
    match menu.background {
        MenuBackground::None => {}
        MenuBackground::Gradient => out.push(GuiQuad::FillGradient {
            rect: screen,
            top: GRADIENT_TOP,
            bottom: GRADIENT_BOTTOM,
        }),
        MenuBackground::Texture => {
            let tile = IVec2::splat(BACKGROUND_TILE);
            atlas.tile(background, screen, IVec2::ZERO, tile, out);
        }
    }
    let Some(separators) = menu.separators else {
        return;
    };
    let tops = [
        (header, separators.header_height - 2),
        (footer, size.height - separators.footer_height),
    ];
    for (texture, top) in tops {
        let rect = IRect::new(0, top, size.width, top + SEPARATOR_HEIGHT);
        let tile = IVec2::new(BACKGROUND_TILE, SEPARATOR_HEIGHT);
        atlas.tile(texture, rect, IVec2::new(0, top), tile, out);
    }
}

fn draw_sprites(menu: &Menu, widget: &Widget, atlas: &GuiAtlasData, out: &mut Vec<GuiQuad>) {
    let rect = widget.rect();
    match menu.widgets.sprite(widget) {
        Some(WidgetSprite::Button(button)) => {
            let id = match button {
                ButtonSprite::Normal => GuiSpriteId::Button,
                ButtonSprite::Highlighted => GuiSpriteId::ButtonHighlighted,
                ButtonSprite::Disabled => GuiSpriteId::ButtonDisabled,
            };
            atlas.sprite(id).draw(irect(rect), out);
        }
        Some(WidgetSprite::TextField { highlighted }) => {
            let id = if highlighted {
                GuiSpriteId::TextFieldHighlighted
            } else {
                GuiSpriteId::TextField
            };
            atlas.sprite(id).draw(irect(rect), out);
        }
        Some(WidgetSprite::Checkbox {
            selected,
            highlighted,
        }) => {
            let id = match (selected, highlighted) {
                (false, false) => GuiSpriteId::Checkbox,
                (false, true) => GuiSpriteId::CheckboxHighlighted,
                (true, false) => GuiSpriteId::CheckboxSelected,
                (true, true) => GuiSpriteId::CheckboxSelectedHighlighted,
            };
            let square = GuiRect::new(rect.x, rect.y, CHECKBOX_SIZE, CHECKBOX_SIZE);
            atlas.sprite(id).draw(irect(square), out);
        }
        Some(WidgetSprite::Slider {
            bar_highlighted,
            handle_highlighted,
            handle_rect,
        }) => {
            let bar = if bar_highlighted {
                GuiSpriteId::SliderHighlighted
            } else {
                GuiSpriteId::Slider
            };
            let handle = if handle_highlighted {
                GuiSpriteId::SliderHandleHighlighted
            } else {
                GuiSpriteId::SliderHandle
            };
            atlas.sprite(bar).draw(irect(rect), out);
            atlas.sprite(handle).draw(irect(handle_rect), out);
        }
        Some(WidgetSprite::Image {
            active,
            highlighted,
        }) => {
            let Some(image) = menu.images.iter().find(|image| image.widget == widget.id()) else {
                return;
            };
            let id = match (active, highlighted) {
                (true, false) => image.active,
                (true, true) => image.active_highlighted,
                (false, false) => image.inactive,
                (false, true) => image.inactive_highlighted,
            };
            atlas.sprite(id).draw(irect(rect), out);
        }
        _ => {}
    }
    if let WidgetKind::IconButton { icon, .. } = widget.kind
        && let Some(origin) = widget.icon_origin(atlas.font.str_width(&widget.caption))
        && let Some(entry) = menu.icons.iter().find(|entry| entry.widget == widget.id())
    {
        let bounds = GuiRect::new(origin.x, origin.y, icon.width, icon.height);
        atlas.sprite(entry.sprite).draw(irect(bounds), out);
    }
}

/// The caption of `widget`. A checkbox label and a multi-line text draw one line for each line
/// of the caption, which the screen wraps and cuts.
fn draw_caption(widget: &Widget, now_ms: u64, font: &Font, out: &mut Vec<GuiQuad>) {
    let caption = widget.caption.as_str();
    let color = widget.caption_color();
    match widget.kind {
        WidgetKind::Checkbox { .. } => {
            let count = i32::try_from(caption.lines().count()).unwrap_or(i32::MAX);
            let origin = point(checkbox_label_origin(widget.rect(), count));
            for (line, text) in (0..).zip(caption.lines()) {
                let at = origin + IVec2::new(0, LINE_HEIGHT * line);
                draw_str(font, at.as_vec2(), text, color, false, out);
            }
        }
        WidgetKind::MultiLineText { .. } => {
            for (line, text) in (0..).zip(caption.lines()) {
                if let Some(origin) = widget.line_origin(line, font.str_width(text)) {
                    draw_str(font, point(origin).as_vec2(), text, color, false, out);
                }
            }
        }
        _ => {
            let width = font.str_width(caption);
            let Some(layout) = widget.caption_layout(width, now_ms) else {
                return;
            };
            if let Some(clip) = layout.clip {
                out.push(GuiQuad::Clip { rect: irect(clip) });
            }
            let origin = point(layout.origin);
            match cut_room(widget, width, font) {
                Some(room) => {
                    let start = caption.get(..cut_end(font, caption, room)).unwrap_or("");
                    let x = draw_str(font, origin.as_vec2(), start, color, false, out);
                    draw_str(
                        font,
                        Vec2::new(x, origin.y as f32),
                        ELLIPSIS,
                        color,
                        false,
                        out,
                    );
                }
                None => {
                    draw_str(font, origin.as_vec2(), caption, color, false, out);
                }
            }
            if layout.clip.is_some() {
                out.push(GuiQuad::EndClip);
            }
        }
    }
}

/// The width left for the start of a one-line text that its limit cuts (WID-TXT-001, step 5).
fn cut_room(widget: &Widget, text_width: i32, font: &Font) -> Option<i32> {
    let WidgetKind::Text {
        max_width,
        overflow: Overflow::Cut,
    } = widget.kind
    else {
        return None;
    };
    let limit = max_width.unwrap_or(widget.rect().width);
    (text_width > limit).then(|| limit - font.str_width(ELLIPSIS))
}

/// The byte length of the longest start of `text` no wider than `room`.
fn cut_end(font: &Font, text: &str, room: i32) -> usize {
    let mut width = 0.0;
    for (index, ch) in text.char_indices() {
        width += font.advance(ch, false);
        if width > room as f32 {
            return index;
        }
    }
    text.len()
}

pub(crate) fn irect(rect: GuiRect) -> IRect {
    IRect::new(
        rect.x,
        rect.y,
        rect.x.saturating_add(rect.width),
        rect.y.saturating_add(rect.height),
    )
}

pub(crate) fn point(point: GuiPoint) -> IVec2 {
    IVec2::new(point.x, point.y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::{Span, corpus_font, shadow_color};
    use crate::item_decorations::WHITE;
    use crate::menu::WidgetTooltip;
    use mcrs_minecraft_client_widget::{
        EditBox, GuiExtent, GuiInput, GuiKey, KeyModifiers, Slider, TooltipDelay as Delay,
        WidgetId, Widgets,
    };

    const GUI: GuiSize = GuiSize {
        width: 320,
        height: 240,
        scale: 2,
    };

    fn menu(widget: Widget, mouse: GuiPoint) -> Menu {
        let input = GuiInput::new(GuiPoint::new(-100, -100));
        let mut widgets = Widgets::new(GuiExtent::new(GUI.width, GUI.height), input);
        widgets.push(widget).unwrap();
        widgets.mouse_move(mouse);
        Menu::new(widgets, MenuBackground::None)
    }

    mod jar {
        use std::sync::LazyLock;

        use mcrs_minecraft_model::resource_set::ResourceSet;

        use super::*;

        static ATLAS: LazyLock<GuiAtlasData> =
            LazyLock::new(|| GuiAtlasData::load(ResourceSet::corpus()).unwrap());

        fn quads(menu: &Menu) -> Vec<GuiQuad> {
            let mut quads = Vec::new();
            draw_menu(menu, WorldPresence::NoWorld, 0, GUI, &ATLAS, &mut quads);
            quads
        }

        #[test]
        fn each_state_of_a_button_and_a_slider_draws_its_sprites_and_caption_colour() {
            let button = GuiRect::new(10, 20, 150, 20);
            let slider = GuiRect::new(10, 50, 150, 20);
            let handle = GuiRect::new(81, 50, 8, 20);
            let away = GuiPoint::new(300, 200);
            let on_button = GuiPoint::new(20, 25);
            let on_slider = GuiPoint::new(20, 55);
            let (white, grey) = (0xFFFF_FFFF, 0xFFA0_A0A0);
            let half = || WidgetKind::Slider(Slider::continuous(0.5));
            let cases = [
                (
                    WidgetKind::Button,
                    button,
                    true,
                    away,
                    vec![(GuiSpriteId::Button, button)],
                    white,
                ),
                (
                    WidgetKind::Button,
                    button,
                    true,
                    on_button,
                    vec![(GuiSpriteId::ButtonHighlighted, button)],
                    white,
                ),
                (
                    WidgetKind::Button,
                    button,
                    false,
                    on_button,
                    vec![(GuiSpriteId::ButtonDisabled, button)],
                    grey,
                ),
                (
                    half(),
                    slider,
                    true,
                    away,
                    vec![
                        (GuiSpriteId::Slider, slider),
                        (GuiSpriteId::SliderHandle, handle),
                    ],
                    white,
                ),
                (
                    half(),
                    slider,
                    true,
                    on_slider,
                    vec![
                        (GuiSpriteId::Slider, slider),
                        (GuiSpriteId::SliderHandleHighlighted, handle),
                    ],
                    white,
                ),
                (
                    half(),
                    slider,
                    false,
                    on_slider,
                    vec![
                        (GuiSpriteId::Slider, slider),
                        (GuiSpriteId::SliderHandle, handle),
                    ],
                    grey,
                ),
            ];
            for (case, (kind, rect, active, mouse, sprites, colour)) in
                cases.into_iter().enumerate()
            {
                let mut widget = Widget::new(WidgetId(1), rect, "Done".into(), kind);
                widget.active = active;
                let quads = quads(&menu(widget, mouse));
                let mut expected = Vec::new();
                for (id, at) in sprites {
                    ATLAS.sprite(id).draw(irect(at), &mut expected);
                }
                assert_eq!(quads[..expected.len()], expected[..], "case {case}");
                let colours: Vec<u32> = quads[expected.len()..]
                    .iter()
                    .filter_map(|quad| match quad {
                        GuiQuad::Glyph { color, .. } => Some(*color),
                        _ => None,
                    })
                    .collect();
                assert!(!colours.is_empty(), "case {case}");
                assert!(
                    colours
                        .iter()
                        .all(|c| *c == colour || *c == shadow_color(colour)),
                    "case {case}: {colours:x?}"
                );
            }
        }

        #[test]
        fn an_edit_box_draws_its_frame_cursor_and_selection_by_state_and_blink() {
            const BOX: WidgetId = WidgetId(1);
            let rect = GuiRect::new(10, 20, 150, 20);
            let (tx, ty) = (14, 26);
            let text = EditBox::default().text_color;
            let after_a = tx + ATLAS.font.str_width("a");
            let after_end = tx + ATLAS.font.str_width("ab") + 1;
            let underscore = [
                GuiQuad::Glyph {
                    pen: Vec2::new((after_end + 1) as f32, (ty + 1) as f32),
                    glyph: '_',
                    image: ATLAS.font.glyph('_').image.unwrap(),
                    color: shadow_color(text),
                    weight: crate::font_glyph::Weight::Regular,
                },
                GuiQuad::Glyph {
                    pen: Vec2::new((after_end) as f32, (ty) as f32),
                    glyph: '_',
                    image: ATLAS.font.glyph('_').image.unwrap(),
                    color: text,
                    weight: crate::font_glyph::Weight::Regular,
                },
            ];
            let bar = GuiQuad::Fill {
                rect: IRect::new(after_a, ty - 1, after_a + 1, ty + 10),
                color: text,
            };
            let highlight = GuiQuad::Highlight {
                rect: IRect::new(tx - 1, ty - 1, after_end, ty + 10),
                invert: true,
            };
            let idle: fn(&mut Widgets) = |_| {};
            let focus: fn(&mut Widgets) = |widgets| {
                widgets.focus_initial(Some(BOX), 0, &mut Vec::new());
            };
            let focus_left: fn(&mut Widgets) = |widgets| {
                widgets.focus_initial(Some(BOX), 0, &mut Vec::new());
                let width = |text: &str| ATLAS.font.str_width(text);
                widgets.key_press(GuiKey::Left, KeyModifiers::NONE, 0, &width, &mut Vec::new());
            };
            let focus_all: fn(&mut Widgets) = |widgets| {
                widgets.focus_initial(Some(BOX), 0, &mut Vec::new());
                widgets.select_all();
            };
            const FIELD: Option<GuiSpriteId> = Some(GuiSpriteId::TextField);
            const LIT: Option<GuiSpriteId> = Some(GuiSpriteId::TextFieldHighlighted);
            let cases = [
                (false, idle, 0, None, vec![]),
                (true, idle, 0, FIELD, vec![]),
                (true, focus, 0, LIT, underscore.to_vec()),
                (true, focus, 300, LIT, vec![]),
                (true, focus_left, 0, LIT, vec![bar]),
                (true, focus_left, 300, LIT, vec![]),
                (
                    true,
                    focus_all,
                    0,
                    LIT,
                    [highlight].into_iter().chain(underscore).collect(),
                ),
                (true, focus_all, 300, LIT, vec![highlight]),
            ];
            for (case, (border, act, now_ms, frame, parts)) in cases.into_iter().enumerate() {
                let mut edit = EditBox::default();
                edit.border = border;
                edit.set("ab");
                let widget = Widget::new(BOX, rect, String::new(), WidgetKind::EditBox(edit));
                let mut menu = menu(widget, GuiPoint::new(300, 200));
                act(&mut menu.widgets);
                let mut quads = Vec::new();
                draw_menu(
                    &menu,
                    WorldPresence::NoWorld,
                    now_ms,
                    GUI,
                    &ATLAS,
                    &mut quads,
                );
                let mut expected = Vec::new();
                if let Some(id) = frame {
                    ATLAS.sprite(id).draw(irect(rect), &mut expected);
                }
                assert_eq!(quads[..expected.len()], expected[..], "case {case}");
                let drawn: Vec<GuiQuad> = quads[expected.len()..]
                    .iter()
                    .filter(|quad| match quad {
                        GuiQuad::Glyph { glyph, .. } => *glyph == '_',
                        GuiQuad::Fill { .. } | GuiQuad::Highlight { .. } => true,
                        _ => false,
                    })
                    .copied()
                    .collect();
                assert_eq!(drawn, parts, "case {case}");
            }
        }

        #[test]
        fn a_text_wider_than_its_limit_is_cut_before_an_ellipsis() {
            let limit = ATLAS.font.str_width("Opt...");
            let kind = WidgetKind::Text {
                max_width: Some(limit),
                overflow: Overflow::Cut,
            };
            let widget = Widget::new(
                WidgetId(1),
                GuiRect::new(0, 0, limit, 9),
                "Options".into(),
                kind,
            );
            let text: String = quads(&menu(widget, GuiPoint::new(0, 0)))
                .iter()
                .filter_map(|quad| match quad {
                    GuiQuad::Glyph {
                        glyph,
                        color: WHITE,
                        ..
                    } => Some(*glyph),
                    _ => None,
                })
                .collect();
            assert_eq!(text, "Opt...");
        }

        #[test]
        fn a_widget_tooltip_shows_only_after_its_delay() {
            let mut widget = Widget::new(
                WidgetId(1),
                GuiRect::new(10, 20, 150, 20),
                "Done".into(),
                WidgetKind::Button,
            );
            widget.tooltip = Some(Delay { delay_ms: 500 });
            let mut menu = menu(widget, GuiPoint::new(20, 25));
            menu.tooltips.push(WidgetTooltip {
                widget: WidgetId(1),
                text: vec![Span::new("Saves the options", WHITE)],
            });
            let mut tooltip = Tooltip::default();
            let shown = [1000, 1500, 1501].map(|now_ms| {
                tooltip.clear();
                request_menu_tooltip(&mut menu, now_ms, corpus_font(), &mut tooltip);
                tooltip.lines().is_some()
            });
            assert_eq!(shown, [false, false, true]);
        }
    }
}
