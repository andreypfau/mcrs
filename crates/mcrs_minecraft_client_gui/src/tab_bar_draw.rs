use bevy::math::{IRect, IVec2};
use mcrs_minecraft_client_widget::{TabBar, TabSprite, Widget, WidgetKind};

use crate::font::Font;
use crate::gui_atlas_layout::GuiAtlasLayout;
use crate::gui_quad::GuiQuad;
use crate::gui_sprite_id::GuiSpriteId;
use crate::gui_texture_id::GuiTextureId;
use crate::irect::{irect, point};
use crate::menu::Menu;
use crate::sprite_draw::draw_sprite;
use crate::text_draw::draw_str;

const HEADER_TILE: i32 = 16;
const BACKGROUND_TILE: i32 = 32;

pub(crate) fn draw_tab_bar(
    menu: &Menu,
    widget: &Widget,
    now_ms: u64,
    atlas: &GuiAtlasLayout,
    font: &Font,
    out: &mut Vec<GuiQuad>,
) -> bool {
    let WidgetKind::TabBar(bar) = widget.kind() else {
        return false;
    };
    let screen_width = menu.widgets.screen().width;
    let rect = irect(widget.rect());
    let header = IRect::new(0, rect.min.y, screen_width, rect.max.y);
    let tile = IVec2::splat(HEADER_TILE);
    atlas.tile(
        GuiTextureId::TabHeaderBackground,
        header,
        IVec2::ZERO,
        tile,
        out,
    );
    for separator in TabBar::separators(widget.rect(), screen_width) {
        let separator = irect(separator);
        let tile = IVec2::new(BACKGROUND_TILE, separator.height());
        atlas.tile(
            GuiTextureId::HeaderSeparator,
            separator,
            separator.min,
            tile,
            out,
        );
    }
    let mut scrolls = false;
    for (index, tab) in bar.tabs().iter().enumerate() {
        let Some(bounds) = bar.tab_rect(widget.rect(), index) else {
            continue;
        };
        if let Some(sprite) = menu.widgets.tab_sprite(widget, index) {
            draw_sprite(&atlas.sprite(tab_sprite_id(sprite)), irect(bounds), out);
        }
        let label_width = font.str_width(&tab.label);
        let color = tab.label_color();
        if index == bar.current() {
            if let Some(background) = bar.current_background(widget.rect()) {
                let background = irect(background);
                let tile = IVec2::splat(BACKGROUND_TILE);
                atlas.tile(
                    GuiTextureId::MenuBackground,
                    background,
                    background.min,
                    tile,
                    out,
                );
            }
            if let Some(line) = bar.current_line(widget.rect(), label_width) {
                out.push(GuiQuad::Fill {
                    rect: irect(line),
                    color,
                });
            }
        }
        let Some(layout) = bar.label_layout(widget.rect(), index, label_width, now_ms) else {
            continue;
        };
        scrolls |= layout.scrolls;
        if let Some(clip) = layout.clip {
            out.push(GuiQuad::Clip { rect: irect(clip) });
        }
        draw_str(
            font,
            point(layout.origin).as_vec2(),
            &tab.label,
            color,
            false,
            out,
        );
        if layout.clip.is_some() {
            out.push(GuiQuad::EndClip);
        }
    }
    scrolls
}

fn tab_sprite_id(sprite: TabSprite) -> GuiSpriteId {
    match (sprite.current, sprite.highlighted) {
        (false, false) => GuiSpriteId::Tab,
        (false, true) => GuiSpriteId::TabHighlighted,
        (true, false) => GuiSpriteId::TabSelected,
        (true, true) => GuiSpriteId::TabSelectedHighlighted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod jar {

        use mcrs_minecraft_client_widget::{GuiExtent, GuiInput, GuiPoint, Tab, WidgetId, Widgets};

        use super::*;
        use crate::font::corpus_font;
        use crate::gui_size::GuiSize;
        use crate::menu::MenuBackground;
        use crate::menu_draw::draw_menu;
        use crate::menu_test_app::ATLAS;

        const GUI: GuiSize = GuiSize {
            width: 427,
            height: 240,
            scale: 2,
        };

        #[test]
        fn each_tab_at_427_draws_the_sprite_of_its_state_in_its_rectangle() {
            let tab = |label: &str| Tab {
                label: label.into(),
                active: true,
            };
            let bar = TabBar::new(vec![tab("Game"), tab("World"), tab("More")]);
            let rect = bar.menu_rect(GUI.width);
            let cases = [
                (
                    GuiPoint::new(300, 10),
                    [
                        GuiSpriteId::TabSelected,
                        GuiSpriteId::Tab,
                        GuiSpriteId::TabHighlighted,
                    ],
                ),
                (
                    GuiPoint::new(50, 10),
                    [
                        GuiSpriteId::TabSelectedHighlighted,
                        GuiSpriteId::Tab,
                        GuiSpriteId::Tab,
                    ],
                ),
            ];
            for (mouse, sprites) in cases {
                let extent = GuiExtent::new(GUI.width, GUI.height);
                let away = GuiInput::new(GuiPoint::new(-100, -100));
                let mut widgets = Widgets::new(extent, away);
                let kind = WidgetKind::TabBar(bar.clone());
                let widget = Widget::new(WidgetId(1), rect, String::new(), kind);
                widgets.push(widget).unwrap();
                widgets.mouse_move(mouse);
                let menu = Menu::new(widgets, MenuBackground::None);
                let mut quads = Vec::new();
                draw_menu(&menu, 0, GUI, &ATLAS.layout, corpus_font(), &mut quads);

                let mut from = 0;
                for (left, name) in [28, 152, 276].into_iter().zip(sprites) {
                    let mut run = Vec::new();
                    draw_sprite(
                        &ATLAS.layout.sprite(name),
                        IRect::new(left, 0, left + 124, 24),
                        &mut run,
                    );
                    let at = quads
                        .get(from..)
                        .and_then(|rest| rest.windows(run.len()).position(|w| w == run));
                    assert!(at.is_some(), "{mouse:?}: {name:?} at {left}");
                    from += at.unwrap() + run.len();
                }
            }
        }
    }
}
