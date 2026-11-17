use crate::gui_point::GuiPoint;
use crate::gui_rect::GuiRect;

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Caret {
    Bar(GuiRect),
    Underscore(GuiPoint),
}
