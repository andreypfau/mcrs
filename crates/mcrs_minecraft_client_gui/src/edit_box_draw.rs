use mcrs_minecraft_client_widget::{
    EditBox, EditBoxFrame, EditBoxHighlight, EditBoxPart, GuiPoint, Widget,
};

use crate::font::{Font, Span};
use crate::gui_atlas_layout::GuiAtlasLayout;
use crate::gui_quad::GuiQuad;
use crate::gui_sprite_id::GuiSpriteId;
use crate::irect::{irect, point};
use crate::menu::Menu;
use crate::text_draw::{draw_str, draw_text};

const UNDERSCORE: &str = "_";

pub(crate) fn draw_edit_box(
    menu: &Menu,
    widget: &Widget,
    edit: &EditBox,
    now_ms: u64,
    atlas: &GuiAtlasLayout,
    font: &Font,
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
    let frame = EditBoxFrame {
        hint,
        highlight,
        ..EditBoxFrame::new(widget.rect(), now_ms)
    };
    draw_edit_box_in(frame, edit, atlas, font, out);
}

pub(crate) fn draw_edit_box_in(
    frame: EditBoxFrame<'_, &[Span]>,
    edit: &EditBox,
    atlas: &GuiAtlasLayout,
    font: &Font,
    out: &mut Vec<GuiQuad>,
) {
    edit.draw(frame, &|text| font.str_width(text), &[], &mut |part| {
        draw_edit_box_part(atlas, font, part, out)
    });
}

pub(crate) fn draw_edit_box_part(
    atlas: &GuiAtlasLayout,
    font: &Font,
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
            draw_run(font, at, text, color, out);
        }
        EditBoxPart::Hint { at, hint } => draw_text(font, point(at), hint, out),
        EditBoxPart::Selection { rect, invert } => out.push(GuiQuad::Highlight {
            rect: irect(rect),
            invert,
        }),
        EditBoxPart::CaretBar { rect, color } => out.push(GuiQuad::Fill {
            rect: irect(rect),
            color,
        }),
        EditBoxPart::CaretUnderscore { at, color } => draw_run(font, at, UNDERSCORE, color, out),
        _ => {}
    }
}

fn draw_run(font: &Font, at: GuiPoint, text: &str, color: u32, out: &mut Vec<GuiQuad>) {
    draw_str(font, point(at).as_vec2(), text, color, false, out);
}
