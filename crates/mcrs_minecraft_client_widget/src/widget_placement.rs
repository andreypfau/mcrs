use crate::gui_rect::GuiRect;

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetPlacement {
    Screen,
    Area { clip: GuiRect, shift: i32 },
    Hidden,
}
