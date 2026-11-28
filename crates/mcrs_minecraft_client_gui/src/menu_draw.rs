use bevy::math::{IRect, IVec2};
use bevy::prelude::*;
use bevy::time::Real;
use mcrs_minecraft_client_widget::{TOOLTIP_WRAP_WIDTH, Widget, WidgetKind, WidgetPlacement};

use crate::gui_atlas_layout::GuiAtlasLayout;
use crate::gui_band::GuiBand;
use crate::gui_layer::GuiLayer;
use crate::gui_quad::GuiQuad;
use crate::gui_size::GuiSize;
use crate::gui_texture_id::GuiTextureId;
use crate::in_game_gradient::{DIM_BOTTOM, DIM_TOP};
use crate::layer_due::gui_clock;
use crate::leaf_draw::draw_leaf;
use crate::menu::{Menu, MenuBackground};
use crate::scroll_area_draw::draw_scroll_area;
use crate::tooltip::{LineGap, Tooltip};
use crate::world_presence::WorldPresence;
use mcrs_minecraft_font::font::Font;
use mcrs_minecraft_font::text::{self, Wrap};

const ERROR_GRADIENT_TOP: u32 = 0xFF40_2020;
const ERROR_GRADIENT_BOTTOM: u32 = 0xFF50_1010;
const BACKGROUND_TILE: i32 = 32;
const SEPARATOR_HEIGHT: i32 = 2;

#[derive(Component, Default)]
#[require(GuiLayer = GuiLayer::new(GuiBand::Screen, 0), Tooltip)]
pub struct MenuLayer;

pub fn draw_menu(
    menu: &Menu,
    now_ms: u64,
    size: GuiSize,
    atlas: &GuiAtlasLayout,
    font: &Font,
    out: &mut Vec<GuiQuad>,
) -> bool {
    draw_background(menu, size, atlas, out);
    let shown = menu.widgets.iter().filter(|widget| {
        widget.visible && menu.widgets.placement(widget) == WidgetPlacement::Screen
    });
    let mut scrolls = false;
    for widget in shown {
        scrolls |= draw_widget(menu, widget, now_ms, atlas, font, out);
    }
    scrolls
}

pub(crate) fn draw_widget(
    menu: &Menu,
    widget: &Widget,
    now_ms: u64,
    atlas: &GuiAtlasLayout,
    font: &Font,
    out: &mut Vec<GuiQuad>,
) -> bool {
    if widget.kind().is_scroll_container() {
        draw_scroll_area(menu, widget, now_ms, atlas, font, out)
    } else {
        draw_leaf(menu, widget, now_ms, atlas, font, out)
    }
}

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
    tooltip.request_widget(request, menu.widgets.focused(), LineGap::Uniform, |lines| {
        text::wrap(&entry.text, TOOLTIP_WRAP_WIDTH, Wrap::Tooltip, font, lines);
    });
}

#[cfg_attr(mcrs_lint, mcrs::alloc_sites(6))]
pub(crate) fn draw(
    menu: Option<ResMut<Menu>>,
    atlas: Option<Res<GuiAtlasLayout>>,
    font: Option<Res<Font>>,
    time: Res<Time<Real>>,
    size: Res<GuiSize>,
    mut layer: Single<(&mut GuiLayer, &mut Tooltip), With<MenuLayer>>,
) {
    let (layer, tooltip) = &mut *layer;
    let out = layer.redraw();
    tooltip.clear();
    let (Some(mut menu), Some(atlas), Some(font)) = (menu, atlas, font) else {
        return;
    };
    let now_ms = gui_clock(&time);
    let scrolls = draw_menu(&menu, now_ms, *size, &atlas, &font, out);
    if scrolls {
        layer.redraw_at(now_ms);
    }
    if let Some(at) = menu.widgets.next_redraw_ms(now_ms) {
        layer.redraw_at(at);
    }
    let menu = menu.bypass_change_detection();
    request_menu_tooltip(menu, now_ms, &font, &mut *tooltip);
}

fn draw_background(menu: &Menu, size: GuiSize, atlas: &GuiAtlasLayout, out: &mut Vec<GuiQuad>) {
    let tab_bar = menu
        .widgets
        .iter()
        .find(|widget| widget.visible && matches!(widget.kind(), WidgetKind::TabBar(_)));
    let top = tab_bar.map_or(0, |bar| bar.rect().y.saturating_add(bar.rect().height));
    let screen = IRect::new(0, top, size.width, size.height);
    let (background, header, footer) = match menu.world {
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
            top: DIM_TOP,
            bottom: DIM_BOTTOM,
        }),
        MenuBackground::ErrorGradient => out.push(GuiQuad::FillGradient {
            rect: screen,
            top: ERROR_GRADIENT_TOP,
            bottom: ERROR_GRADIENT_BOTTOM,
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
        separators.header_height.map(|height| (header, height - 2)),
        Some((footer, size.height - separators.footer_height)),
    ];
    for (texture, top) in tops.into_iter().flatten() {
        let rect = IRect::new(0, top, size.width, top + SEPARATOR_HEIGHT);
        let tile = IVec2::new(BACKGROUND_TILE, SEPARATOR_HEIGHT);
        atlas.tile(texture, rect, IVec2::new(0, top), tile, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus_font::corpus_font;
    use crate::gui_quad::WHITE;
    use crate::gui_sprite_id::GuiSpriteId;
    use crate::irect::irect;
    use crate::menu::WidgetTooltip;
    use crate::sprite_draw::draw_sprite;
    use crate::text_draw::shadow_color;
    use mcrs_minecraft_client_widget::{
        EditBox, EditBoxLook, GuiExtent, GuiInput, GuiKey, GuiPoint, GuiRect, KeyModifiers,
        Overflow, Slider, TooltipDelay as Delay, WidgetId, Widgets,
    };
    use mcrs_minecraft_font::font::Span;

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

        use crate::menu_test_app::ATLAS;

        use super::*;

        fn quads(menu: &Menu) -> Vec<GuiQuad> {
            let mut quads = Vec::new();
            draw_menu(menu, 0, GUI, &ATLAS.layout, corpus_font(), &mut quads);
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
                    draw_sprite(&ATLAS.layout.sprite(id), irect(at), &mut expected);
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
            let text = 0xFFE0_E0E0;
            let after_a = tx + corpus_font().str_width("a");
            let after_end = tx + corpus_font().str_width("ab") + 1;
            let underscore = [
                GuiQuad::Glyph {
                    pen: Vec2::new((after_end + 1) as f32, (ty + 1) as f32),
                    glyph: '_',
                    image: corpus_font().glyph('_').image.unwrap(),
                    color: shadow_color(text),
                    weight: mcrs_minecraft_font::font_glyph::Weight::Regular,
                },
                GuiQuad::Glyph {
                    pen: Vec2::new((after_end) as f32, (ty) as f32),
                    glyph: '_',
                    image: corpus_font().glyph('_').image.unwrap(),
                    color: text,
                    weight: mcrs_minecraft_font::font_glyph::Weight::Regular,
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
                let width = |text: &str| corpus_font().str_width(text);
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
                let look = if border {
                    EditBoxLook::Form
                } else {
                    EditBoxLook::Chat
                };
                let mut edit = EditBox::new(look);
                edit.set("ab");
                let widget = Widget::new(BOX, rect, String::new(), WidgetKind::EditBox(edit));
                let mut menu = menu(widget, GuiPoint::new(300, 200));
                act(&mut menu.widgets);
                let mut quads = Vec::new();
                draw_menu(&menu, now_ms, GUI, &ATLAS.layout, corpus_font(), &mut quads);
                let mut expected = Vec::new();
                if let Some(id) = frame {
                    draw_sprite(&ATLAS.layout.sprite(id), irect(rect), &mut expected);
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
            let limit = corpus_font().str_width("Opt...");
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

        #[test]
        fn the_caret_of_a_focused_edit_box_blinks_without_any_input() {
            use crate::menu_test_app::{advance, title_app};
            use mcrs_minecraft_client_world::screen::{CreateWorldParent, Screen};
            use mcrs_minecraft_client_world::world_list::WorldTypes;

            let mut app = title_app();
            app.insert_resource(WorldTypes::Ready(crate::menu_test_app::world_types()));
            *app.world_mut().resource_mut::<Screen>() =
                Screen::CreateWorld(CreateWorldParent::Title);
            advance(&mut app, 0);
            let quads = |app: &mut App| {
                app.world_mut()
                    .query_filtered::<&GuiLayer, With<MenuLayer>>()
                    .single(app.world())
                    .unwrap()
                    .quads()
                    .to_vec()
            };
            let first = quads(&mut app);
            advance(&mut app, 300);
            let second = quads(&mut app);
            advance(&mut app, 300);
            let third = quads(&mut app);
            assert_ne!(first, second, "the caret goes off after 300 ms");
            assert_eq!(first, third, "the caret is on again after 600 ms");
        }
    }
}
