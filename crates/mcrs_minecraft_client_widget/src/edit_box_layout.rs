use core::ops::Range;

use crate::caret::Caret;
use crate::gui_point::GuiPoint;
use crate::gui_rect::GuiRect;

/// Where the parts of an edit box draw in a frame, in the order of EDIT-DRAW-004.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditBoxLayout {
    /// The text origin (`tx`, `ty`) of the visible text (EDIT-DRAW-002).
    pub origin: GuiPoint,
    /// The byte range of the value that shows; its start is the position that a formatter
    /// takes (EDIT-DRAW-003, EDIT-API-004).
    pub visible: Range<usize>,
    /// The ARGB colour of the text and the cursor before a formatter (EDIT-DRAW-003).
    pub color: u32,
    /// The cursor, when it shows (EDIT-CUR-001, EDIT-CUR-002).
    pub caret: Option<Caret>,
    /// The rectangle of the selection highlight, when the selection is not empty (EDIT-SEL-001).
    pub selection: Option<GuiRect>,
    /// The origin of the hint, when it shows (EDIT-HINT-001).
    pub hint: Option<GuiPoint>,
    /// The origin of the suggestion, when it shows (EDIT-HINT-002).
    pub suggestion: Option<GuiPoint>,
}

impl EditBoxLayout {
    /// The ARGB colour of a hint whose style has no colour (EDIT-HINT-001).
    pub const HINT_COLOR: u32 = 0xFF55_5555;
    /// The ARGB colour of the suggestion (EDIT-HINT-002).
    pub const SUGGESTION_COLOR: u32 = 0xFF80_8080;
}
