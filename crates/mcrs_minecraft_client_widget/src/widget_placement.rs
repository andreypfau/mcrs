use crate::gui_rect::GuiRect;

/// Where a widget draws in a frame (SCRL-AREA-003, SCRL-PANEL-001, LIST-BG-003).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetPlacement {
    /// At its bounds.
    Screen,
    /// In a scroll area: `shift` GUI units above its bounds, cut to `clip`.
    Area { clip: GuiRect, shift: i32 },
    /// Nowhere: its scroll area is not visible, or its row or its bounds are out of the area.
    Hidden,
}
