use crate::gui_point::GuiPoint;
use crate::gui_rect::GuiRect;

/// The form and the place of the cursor of an edit box in a frame (EDIT-CUR-001).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Caret {
    /// A fill of the text colour over this rectangle.
    Bar(GuiRect),
    /// The text `_` drawn from this point.
    Underscore(GuiPoint),
}
