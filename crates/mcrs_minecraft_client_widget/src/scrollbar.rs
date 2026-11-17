use crate::gui_rect::GuiRect;

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scrollbar {
    pub track: GuiRect,
    pub thumb: GuiRect,
}
