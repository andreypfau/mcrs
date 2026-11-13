use bevy::math::IVec2;
use mcrs_minecraft_client_widget::{Caret, EditBox, EditBoxLayout, Widget};

use crate::font::{Font, draw_str, draw_str_without_shadow, draw_text};
use crate::menu::Menu;
use crate::menu_draw::{irect, point};
use crate::scene::GuiQuad;

/// The parts of the edit box `edit` of `widget` at `now_ms` after its frame, in the order of
/// EDIT-DRAW-004: the visible text, the hint, the suggestion, the selection highlight and the
/// cursor.
pub(crate) fn draw_edit_box(
    menu: &Menu,
    widget: &Widget,
    edit: &EditBox,
    now_ms: u64,
    font: &Font,
    out: &mut Vec<GuiQuad>,
) {
    let EditBoxLayout {
        origin,
        visible,
        color,
        caret,
        selection,
        hint,
        suggestion,
        ..
    } = edit.layout(widget.rect(), now_ms, &|text| font.str_width(text));
    let run = |at: IVec2, text: &str, color: u32, out: &mut Vec<GuiQuad>| {
        if edit.text_shadow {
            draw_str(font, at, text, color, false, out);
        } else {
            draw_str_without_shadow(font, at, text, color, out);
        }
    };
    run(
        point(origin),
        edit.text().get(visible).unwrap_or_default(),
        color,
        out,
    );
    if let Some(at) = hint
        && let Some(entry) = menu.hints.iter().find(|entry| entry.widget == widget.id())
    {
        draw_text(font, point(at), &entry.text, out);
    }
    if let (Some(at), Some(text)) = (suggestion, edit.suggestion.as_deref()) {
        run(point(at), text, EditBoxLayout::SUGGESTION_COLOR, out);
    }
    if let Some(rect) = selection {
        out.push(GuiQuad::Highlight {
            rect: irect(rect),
            invert: edit.invert,
        });
    }
    match caret {
        Some(Caret::Bar(rect)) => out.push(GuiQuad::Fill {
            rect: irect(rect),
            color,
        }),
        Some(Caret::Underscore(at)) => run(point(at), "_", color, out),
        _ => {}
    }
}
