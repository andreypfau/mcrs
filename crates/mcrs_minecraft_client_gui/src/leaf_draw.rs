use bevy::math::{IVec2, Vec2};
use mcrs_minecraft_client_widget::{
    ButtonSprite, CHECKBOX_SIZE, GuiRect, LINE_HEIGHT, Overflow, Widget, WidgetKind, WidgetSprite,
    checkbox_label_origin,
};

use crate::edit_box_draw::draw_edit_box;
use crate::font::Font;
use crate::gui_atlas_layout::GuiAtlasLayout;
use crate::gui_quad::{GuiQuad, WHITE};
use crate::gui_sprite_id::GuiSpriteId;
use crate::irect::{irect, point};
use crate::menu::Menu;
use crate::outlined_box::draw_outlined_box;
use crate::tab_bar_draw::draw_tab_bar;
use crate::text_draw::draw_str;

const ELLIPSIS: &str = "...";

pub(crate) fn draw_leaf(
    menu: &Menu,
    widget: &Widget,
    now_ms: u64,
    atlas: &GuiAtlasLayout,
    font: &Font,
    out: &mut Vec<GuiQuad>,
) -> bool {
    match *widget.kind() {
        WidgetKind::TabBar(_) => draw_tab_bar(menu, widget, now_ms, atlas, font, out),
        WidgetKind::SectionLabel => {
            draw_section_label_box(menu, widget, out);
            draw_caption(widget, now_ms, font, out)
        }
        WidgetKind::EditBox(ref edit) => {
            draw_edit_box(menu, widget, edit, now_ms, atlas, font, out);
            false
        }
        _ => {
            draw_sprites(menu, widget, atlas, font, out);
            draw_caption(widget, now_ms, font, out)
        }
    }
}

fn draw_sprites(
    menu: &Menu,
    widget: &Widget,
    atlas: &GuiAtlasLayout,
    font: &Font,
    out: &mut Vec<GuiQuad>,
) {
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
    if let WidgetKind::IconButton { icon, .. } = *widget.kind()
        && let Some(origin) = widget.icon_origin(font.str_width(&widget.caption))
        && let Some(entry) = menu.icons.iter().find(|entry| entry.widget == widget.id())
    {
        let bounds = GuiRect::new(origin.x, origin.y, icon.width, icon.height);
        atlas.sprite(entry.sprite).draw(irect(bounds), out);
    }
}

fn draw_caption(widget: &Widget, now_ms: u64, font: &Font, out: &mut Vec<GuiQuad>) -> bool {
    let caption = widget.caption.as_str();
    let color = widget.caption_color();
    match *widget.kind() {
        WidgetKind::Checkbox { .. } => {
            let count = i32::try_from(caption.lines().count()).unwrap_or(i32::MAX);
            let origin = point(checkbox_label_origin(widget.rect(), count));
            for (line, text) in (0..).zip(caption.lines()) {
                let at = origin + IVec2::new(0, LINE_HEIGHT * line);
                draw_str(font, at.as_vec2(), text, color, false, out);
            }
            false
        }
        WidgetKind::MultiLineText { .. } => {
            for (line, text) in (0..).zip(caption.lines()) {
                if let Some(origin) = widget.line_origin(line, font.str_width(text)) {
                    draw_str(font, point(origin).as_vec2(), text, color, false, out);
                }
            }
            false
        }
        _ => {
            let width = font.str_width(caption);
            let Some(layout) = widget.caption_layout(width, now_ms) else {
                return false;
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
            layout.scrolls
        }
    }
}

fn cut_room(widget: &Widget, text_width: i32, font: &Font) -> Option<i32> {
    let WidgetKind::Text {
        max_width,
        overflow: Overflow::Cut,
    } = *widget.kind()
    else {
        return None;
    };
    let limit = max_width.unwrap_or(widget.rect().width);
    (text_width > limit).then(|| limit - font.str_width(ELLIPSIS))
}

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

pub(crate) fn draw_section_label_box(menu: &Menu, widget: &Widget, out: &mut Vec<GuiQuad>) {
    if menu.widgets.focused() != Some(widget.id()) {
        return;
    }
    draw_outlined_box(irect(widget.rect()), WHITE, out);
}
