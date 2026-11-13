use crate::button_sprite::ButtonSprite;
use crate::gui_rect::GuiRect;

/// The sprites that a widget draws in a frame.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetSprite {
    /// The sprite that fills the bounds of a button or an icon button (WID-BTN-002).
    Button(ButtonSprite),
    /// The state of an image button; the sprite that the screen gives for it fills the bounds
    /// (WID-ICON-002).
    Image { active: bool, highlighted: bool },
    /// The box sprite of a checkbox, 17 × 17 at the top left of its bounds (WID-CHK-002).
    Checkbox { selected: bool, highlighted: bool },
    /// The bar sprite that fills the bounds of a slider and the handle sprite in `handle_rect`
    /// (WID-SLD-001).
    Slider {
        bar_highlighted: bool,
        handle_highlighted: bool,
        handle_rect: GuiRect,
    },
    /// The frame sprite that fills the bounds of an edit box (EDIT-DRAW-001).
    TextField { highlighted: bool },
}
