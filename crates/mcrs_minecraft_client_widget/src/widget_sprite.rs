use crate::button_sprite::ButtonSprite;
use crate::gui_rect::GuiRect;

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetSprite {
    Button(ButtonSprite),
    Image {
        active: bool,
        highlighted: bool,
    },
    Checkbox {
        selected: bool,
        highlighted: bool,
    },
    Slider {
        bar_highlighted: bool,
        handle_highlighted: bool,
        handle_rect: GuiRect,
    },
}
