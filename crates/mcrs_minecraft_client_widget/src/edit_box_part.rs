use crate::edit_box_hint::EditBoxHint;
use crate::gui_point::GuiPoint;
use crate::gui_rect::GuiRect;

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditBoxPart<'a, H = EditBoxHint<'a>> {
    Frame {
        rect: GuiRect,
        highlighted: bool,
    },
    Text {
        at: GuiPoint,
        text: &'a str,
        color: u32,
    },
    Hint {
        at: GuiPoint,
        hint: H,
    },
    Suggestion {
        at: GuiPoint,
        text: &'a str,
        color: u32,
    },
    Selection {
        rect: GuiRect,
        invert: bool,
    },
    CaretBar {
        rect: GuiRect,
        color: u32,
    },
    CaretUnderscore {
        at: GuiPoint,
        color: u32,
    },
}
