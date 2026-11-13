use bevy::math::{IRect, IVec2};
use mcrs_minecraft_client_widget::{SelectionList, Widget, WidgetKind, WidgetPlacement};

use crate::font::{Font, draw_str};
use crate::gui_sprite_id::GuiSpriteId;
use crate::gui_texture_id::GuiTextureId;
use crate::item_decorations::WHITE;
use crate::menu::Menu;
use crate::menu_draw::{WorldPresence, draw_widget, irect, point};
use crate::scene::{GuiAtlasData, GuiQuad};

const BLACK: u32 = 0xFF00_0000;
const BACKGROUND_TILE: i32 = 32;
const ROW_INSET: i32 = 2;

/// A selection list, a widget list or a scroll panel in the order of LIST-BG-003: the
/// background, the rows and the member widgets inside the clip, the separators, then the
/// scrollbar. A scroll panel has no background and no separators (SCRL-PANEL-001).
pub(crate) fn draw_scroll_area(
    menu: &Menu,
    widget: &Widget,
    world: WorldPresence,
    now_ms: u64,
    atlas: &GuiAtlasData,
    out: &mut Vec<GuiQuad>,
) {
    let rect = irect(widget.rect());
    let frame = widget.list_frame();
    let (background, header, footer) = match world {
        WorldPresence::NoWorld => (
            GuiTextureId::MenuListBackground,
            GuiTextureId::HeaderSeparator,
            GuiTextureId::FooterSeparator,
        ),
        WorldPresence::InWorld => (
            GuiTextureId::InworldMenuListBackground,
            GuiTextureId::InworldHeaderSeparator,
            GuiTextureId::InworldFooterSeparator,
        ),
    };
    if let Some(frame) = frame {
        let start = rect.min.saturating_sub(point(frame.background_origin));
        let tile = IVec2::splat(BACKGROUND_TILE);
        atlas.tile(background, rect, start, tile, out);
    }
    out.push(GuiQuad::Clip { rect });
    if let WidgetKind::SelectionList(list) = &widget.kind {
        draw_rows(menu, widget, list, &atlas.font, out);
    }
    let members = menu.widgets.iter().filter(|member| {
        member.visible && member.area.is_some_and(|slot| slot.area == widget.id())
    });
    for member in members {
        if let WidgetPlacement::Area { shift, .. } = menu.widgets.placement(member) {
            let start = out.len();
            draw_widget(menu, member, world, now_ms, atlas, out);
            if let Some(quads) = out.get_mut(start..) {
                shift_up(quads, shift);
            }
        }
    }
    out.push(GuiQuad::EndClip);
    if let Some(frame) = frame {
        for (texture, separator) in [
            (header, frame.header_separator),
            (footer, frame.footer_separator),
        ] {
            let separator = irect(separator);
            let tile = IVec2::new(BACKGROUND_TILE, separator.height());
            atlas.tile(texture, separator, separator.min, tile, out);
        }
    }
    if let Some(scrollbar) = widget.scrollbar() {
        let track = atlas.sprite(GuiSpriteId::ScrollerBackground);
        track.draw(irect(scrollbar.track), out);
        atlas
            .sprite(GuiSpriteId::Scroller)
            .draw(irect(scrollbar.thumb), out);
    }
}

/// The black box and the white outline behind a section label that has the focus
/// (LIST-OPT-002, step 5).
pub(crate) fn draw_section_label_box(menu: &Menu, widget: &Widget, out: &mut Vec<GuiQuad>) {
    if menu.widgets.focused() != Some(widget.id()) {
        return;
    }
    let IRect { min, max } = irect(widget.rect());
    let fill = IRect::new(min.x.saturating_add(1), min.y, max.x, max.y);
    out.push(GuiQuad::Fill {
        rect: fill,
        color: BLACK,
    });
    let edges = [
        IRect::new(min.x, min.y, max.x, min.y.saturating_add(1)),
        IRect::new(min.x, max.y.saturating_sub(1), max.x, max.y),
        IRect::new(min.x, min.y, min.x.saturating_add(1), max.y),
        IRect::new(max.x.saturating_sub(1), min.y, max.x, max.y),
    ];
    for rect in edges {
        out.push(GuiQuad::Fill { rect, color: WHITE });
    }
}

/// The outlines of the selected and the hovered row (LIST-SEL-001), then the text of each row
/// in its inner box, for the rows that reach into the list (LIST-BG-003).
fn draw_rows(
    menu: &Menu,
    widget: &Widget,
    list: &SelectionList,
    font: &Font,
    out: &mut Vec<GuiQuad>,
) {
    let area = irect(widget.rect());
    let shift = widget
        .scroll_offset()
        .map_or(0, |offset| offset.floor() as i32);
    let shown = |row: usize| {
        let bounds = irect(list.row_rect(widget.rect(), row)?);
        let bounds = IRect::from_corners(
            bounds.min.saturating_sub(IVec2::new(0, shift)),
            bounds.max.saturating_sub(IVec2::new(0, shift)),
        );
        (bounds.max.y >= area.min.y && bounds.min.y <= area.max.y).then_some(bounds)
    };
    for row in 0..list.rows() {
        if let Some(color) = menu.widgets.row_outline(widget, row)
            && let Some(bounds) = shown(row)
        {
            out.push(GuiQuad::Fill {
                rect: bounds,
                color,
            });
            out.push(GuiQuad::Fill {
                rect: bounds.inflate(-1),
                color: BLACK,
            });
        }
    }
    let texts = menu
        .row_texts
        .iter()
        .filter(|text| text.list == widget.id());
    for text in texts {
        if let Some(bounds) = shown(text.row) {
            let inner = bounds.inflate(-ROW_INSET);
            draw_str(font, inner.min, &text.text, WHITE, false, out);
        }
    }
}

fn shift_up(quads: &mut [GuiQuad], shift: i32) {
    let offset = IVec2::new(0, shift);
    for quad in quads {
        match quad {
            GuiQuad::Sprite { rect, .. }
            | GuiQuad::Fill { rect, .. }
            | GuiQuad::FillGradient { rect, .. }
            | GuiQuad::Texels { rect, .. }
            | GuiQuad::Highlight { rect, .. }
            | GuiQuad::Clip { rect } => {
                rect.min = rect.min.saturating_sub(offset);
                rect.max = rect.max.saturating_sub(offset);
            }
            GuiQuad::Glyph { origin, .. } | GuiQuad::Item { origin, .. } => {
                *origin = origin.saturating_sub(offset);
            }
            GuiQuad::EndClip => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod jar {
        use std::sync::LazyLock;

        use mcrs_minecraft_client_widget::{
            AreaSlot, GuiExtent, GuiInput, GuiPoint, GuiRect, KeyModifiers, PointerButton,
            PointerPress, WidgetId, WidgetList, Widgets,
        };
        use mcrs_minecraft_model::resource_set::ResourceSet;

        use super::*;
        use crate::menu::MenuBackground;
        use crate::menu_draw::draw_menu;
        use crate::scene::GuiSize;

        const GUI: GuiSize = GuiSize {
            width: 427,
            height: 240,
            scale: 2,
        };
        const LIST: WidgetId = WidgetId(1);

        static ATLAS: LazyLock<GuiAtlasData> =
            LazyLock::new(|| GuiAtlasData::load(ResourceSet::corpus()).unwrap());

        fn screen() -> Widgets {
            let extent = GuiExtent::new(GUI.width, GUI.height);
            Widgets::new(extent, GuiInput::new(GuiPoint::new(-100, -100)))
        }

        fn quads(widgets: Widgets) -> Vec<GuiQuad> {
            let mut quads = Vec::new();
            let menu = Menu::new(widgets, MenuBackground::None);
            draw_menu(&menu, WorldPresence::NoWorld, 0, GUI, &ATLAS, &mut quads);
            quads
        }

        fn sprite_run(id: GuiSpriteId, rect: IRect) -> Vec<GuiQuad> {
            let mut run = Vec::new();
            ATLAS.sprite(id).draw(rect, &mut run);
            run
        }

        fn run_at(quads: &[GuiQuad], run: &[GuiQuad]) -> Option<usize> {
            quads.windows(run.len()).position(|window| window == run)
        }

        #[test]
        fn an_option_list_draws_the_rows_in_its_clip_then_the_thumb_of_its_offset() {
            let area = GuiRect::new(0, 33, 427, 70);
            let mut list = WidgetList::options();
            for _ in 0..4 {
                list.push_row(25);
            }
            let cases = [
                ("at the top", 0.0, [true, true, true, false], 0, 33),
                ("one notch down", -1.0, [true, true, true, true], 12, 41),
            ];
            for (case, wheel, shown, shift, thumb_top) in cases {
                let mut widgets = screen();
                let rows = list.clone();
                let kind = WidgetKind::WidgetList(rows);
                widgets
                    .push(Widget::new(LIST, area, String::new(), kind))
                    .unwrap();
                for row in 0..4 {
                    let bounds = list.row_rect(area, row).unwrap();
                    let origin = WidgetList::option_origin(GUI.width, bounds, 0);
                    let rect = GuiRect::new(origin.x, origin.y, 150, 20);
                    let id = WidgetId(10 + u32::try_from(row).unwrap());
                    let mut button = Widget::new(id, rect, String::new(), WidgetKind::Button);
                    button.area = Some(AreaSlot { area: LIST, row });
                    widgets.push(button).unwrap();
                }
                widgets.mouse_wheel(200.0, 50.0, wheel, &mut Vec::new());
                let quads = quads(widgets);

                let clip = run_at(&quads, &[GuiQuad::Clip { rect: irect(area) }]).unwrap();
                let end = quads
                    .iter()
                    .rposition(|quad| *quad == GuiQuad::EndClip)
                    .unwrap();
                for (row, shown) in (0..).zip(shown) {
                    let top = 37 + 25 * row - shift;
                    let button =
                        sprite_run(GuiSpriteId::Button, IRect::new(58, top, 208, top + 20));
                    let at = run_at(&quads, &button);
                    assert_eq!(at.is_some(), shown, "{case}: row {row}");
                    assert!(
                        at.is_none_or(|at| clip < at && at < end),
                        "{case}: row {row}"
                    );
                }
                let mut scrollbar = sprite_run(
                    GuiSpriteId::ScrollerBackground,
                    IRect::new(376, 33, 382, 103),
                );
                scrollbar.extend(sprite_run(
                    GuiSpriteId::Scroller,
                    IRect::new(376, thumb_top, 382, thumb_top + 47),
                ));
                assert!(quads.ends_with(&scrollbar), "{case}");
            }
        }

        #[test]
        fn the_selected_row_has_a_white_outline_while_its_list_has_the_focus_and_a_grey_one_after()
        {
            let mut widgets = screen();
            let area = GuiRect::new(0, 38, 427, 150);
            let kind = WidgetKind::SelectionList(SelectionList::new(3, 36));
            widgets
                .push(Widget::new(LIST, area, String::new(), kind))
                .unwrap();
            let other = GuiRect::new(10, 200, 50, 20);
            widgets
                .push(Widget::new(
                    WidgetId(2),
                    other,
                    String::new(),
                    WidgetKind::Button,
                ))
                .unwrap();
            let press = |widgets: &mut Widgets, x: i32, y: i32| {
                widgets.mouse_move(GuiPoint::new(x, y));
                let (x, y) = (f64::from(x), f64::from(y));
                let modifiers = KeyModifiers::NONE;
                let width = |_: &str| 0;
                widgets.mouse_press(
                    PointerPress {
                        x,
                        y,
                        button: PointerButton::Left,
                        modifiers,
                        now_ms: 0,
                    },
                    &width,
                    &mut Vec::new(),
                );
                widgets.mouse_release(PointerButton::Left);
            };
            let outline = |color: u32| {
                [
                    GuiQuad::Fill {
                        rect: IRect::new(103, 40, 323, 76),
                        color,
                    },
                    GuiQuad::Fill {
                        rect: IRect::new(104, 41, 322, 75),
                        color: BLACK,
                    },
                ]
            };

            press(&mut widgets, 150, 50);
            assert_eq!(widgets.focused(), Some(LIST));
            let focused = quads(widgets.clone());
            press(&mut widgets, 20, 205);
            let unfocused = quads(widgets);

            assert!(run_at(&focused, &outline(0xFFFF_FFFF)).is_some());
            assert!(run_at(&unfocused, &outline(0xFF80_8080)).is_some());
        }
    }
}
