use crate::gui_rect::GuiRect;

/// The scrollbar of a scrollable scroll area in GUI units (SCRL-BAR-001, SCRL-BAR-002).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scrollbar {
    /// The rectangle of the sprite `widget/scroller_background`.
    pub track: GuiRect,
    /// The rectangle of the sprite `widget/scroller`.
    pub thumb: GuiRect,
}
