use mcrs_minecraft_client_widget::{
    EditBox, EditBoxFrame, EditBoxHighlight, EditBoxPart, GuiPoint, GuiRect, Widget,
};

use crate::font::{Span, draw_str, draw_text};
use crate::gui_sprite_id::GuiSpriteId;
use crate::menu::Menu;
use crate::menu_draw::{irect, point};
use crate::scene::{GuiAtlasData, GuiQuad};

const UNDERSCORE: &str = "_";

pub(crate) fn draw_edit_box(
    menu: &Menu,
    widget: &Widget,
    edit: &EditBox,
    now_ms: u64,
    atlas: &GuiAtlasData,
    out: &mut Vec<GuiQuad>,
) {
    let hint = menu
        .hints
        .iter()
        .find(|entry| entry.widget == widget.id())
        .map(|entry| entry.text.as_slice());
    let highlight = if menu.widgets.is_highlighted(widget) {
        EditBoxHighlight::On
    } else {
        EditBoxHighlight::Off
    };
    draw_edit_box_in(widget.rect(), hint, highlight, edit, now_ms, atlas, out);
}

pub(crate) fn draw_edit_box_in(
    rect: GuiRect,
    hint: Option<&[Span]>,
    highlight: EditBoxHighlight,
    edit: &EditBox,
    now_ms: u64,
    atlas: &GuiAtlasData,
    out: &mut Vec<GuiQuad>,
) {
    edit.draw(
        EditBoxFrame {
            hint,
            highlight,
            ..EditBoxFrame::new(rect, now_ms)
        },
        &|text| atlas.font.str_width(text),
        &[],
        &mut |part| draw_edit_box_part(atlas, part, out),
    );
}

pub(crate) fn draw_edit_box_part(
    atlas: &GuiAtlasData,
    part: EditBoxPart<'_, &[Span]>,
    out: &mut Vec<GuiQuad>,
) {
    match part {
        EditBoxPart::Frame { rect, highlighted } => {
            let id = if highlighted {
                GuiSpriteId::TextFieldHighlighted
            } else {
                GuiSpriteId::TextField
            };
            atlas.sprite(id).draw(irect(rect), out);
        }
        EditBoxPart::Text {
            at, text, color, ..
        }
        | EditBoxPart::Suggestion { at, text, color } => {
            draw_run(atlas, at, text, color, out);
        }
        EditBoxPart::Hint { at, hint } => draw_text(&atlas.font, point(at), hint, out),
        EditBoxPart::Selection { rect, invert } => out.push(GuiQuad::Highlight {
            rect: irect(rect),
            invert,
        }),
        EditBoxPart::CaretBar { rect, color } => out.push(GuiQuad::Fill {
            rect: irect(rect),
            color,
        }),
        EditBoxPart::CaretUnderscore { at, color } => draw_run(atlas, at, UNDERSCORE, color, out),
        _ => {}
    }
}

fn draw_run(atlas: &GuiAtlasData, at: GuiPoint, text: &str, color: u32, out: &mut Vec<GuiQuad>) {
    draw_str(&atlas.font, point(at).as_vec2(), text, color, false, out);
}
