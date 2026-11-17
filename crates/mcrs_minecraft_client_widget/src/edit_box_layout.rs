use core::ops::Range;

use crate::caret::Caret;
use crate::gui_point::GuiPoint;
use crate::gui_rect::GuiRect;

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditBoxLayout {
    pub origin: GuiPoint,
    pub visible: Range<usize>,
    pub color: u32,
    pub caret: Option<Caret>,
    pub selection: Option<GuiRect>,
    pub hint: Option<GuiPoint>,
    pub suggestion: Option<GuiPoint>,
}

impl EditBoxLayout {
    pub const HINT_COLOR: u32 = 0xFF55_5555;
    pub const SUGGESTION_COLOR: u32 = 0xFF80_8080;
}
