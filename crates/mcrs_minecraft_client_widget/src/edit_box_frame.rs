use crate::edit_box_highlight::EditBoxHighlight;
use crate::edit_box_hint::EditBoxHint;
use crate::gui_rect::GuiRect;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditBoxFrame<'a, H = EditBoxHint<'a>> {
    pub rect: GuiRect,
    pub now_ms: u64,
    pub hint: Option<H>,
    pub suggestion: Option<&'a str>,
    pub highlight: EditBoxHighlight,
}

impl<'a, H> EditBoxFrame<'a, H> {
    #[must_use]
    pub const fn new(rect: GuiRect, now_ms: u64) -> Self {
        Self {
            rect,
            now_ms,
            hint: None,
            suggestion: None,
            highlight: EditBoxHighlight::Off,
        }
    }
}
