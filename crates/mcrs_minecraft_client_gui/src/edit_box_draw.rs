use bevy::math::IVec2;
use mcrs_minecraft_client_widget::{Caret, EditBox, EditBoxLayout, GuiRect, Widget};

use crate::font::{Font, Span, draw_str, draw_str_without_shadow, draw_text};
use crate::menu::Menu;
use crate::menu_draw::{irect, point};
use crate::scene::GuiQuad;

pub(crate) fn draw_edit_box(
    menu: &Menu,
    widget: &Widget,
    edit: &EditBox,
    now_ms: u64,
    font: &Font,
    out: &mut Vec<GuiQuad>,
) {
    let hint = menu
        .hints
        .iter()
        .find(|entry| entry.widget == widget.id())
        .map(|entry| entry.text.as_slice());
    draw_edit_box_in(widget.rect(), hint, edit, now_ms, font, out);
}

pub(crate) fn draw_edit_box_in(
    rect: GuiRect,
    hint_text: Option<&[Span]>,
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
    } = edit.layout(rect, now_ms, &|text| font.str_width(text));
    let run = |at: IVec2, text: &str, color: u32, out: &mut Vec<GuiQuad>| {
        if edit.text_shadow {
            draw_str(font, at.as_vec2(), text, color, false, out);
        } else {
            draw_str_without_shadow(font, at.as_vec2(), text, color, out);
        }
    };
    run(
        point(origin),
        edit.text().get(visible).unwrap_or_default(),
        color,
        out,
    );
    if let (Some(at), Some(text)) = (hint, hint_text) {
        draw_text(font, point(at), text, out);
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
