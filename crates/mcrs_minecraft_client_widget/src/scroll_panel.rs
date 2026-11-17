use crate::gui_rect::GuiRect;
use crate::scroll_area::{SCROLLBAR_WIDTH, ScrollArea};

const WHEEL_STEP: i32 = 10;

#[derive(Clone, Debug, PartialEq)]
pub struct ScrollPanel {
    content_height: i32,
    pub(crate) offset: f64,
}

impl ScrollPanel {
    #[must_use]
    pub const fn new(content_height: i32) -> Self {
        Self {
            content_height,
            offset: 0.0,
        }
    }

    #[must_use]
    pub const fn content_height(&self) -> i32 {
        self.content_height
    }

    pub(crate) fn scroll_area(&self, area: GuiRect) -> ScrollArea {
        ScrollArea {
            rect: area,
            content_height: self.content_height,
            wheel_step: WHEEL_STEP,
            scrollbar_x: area
                .x
                .saturating_add(area.width)
                .saturating_sub(SCROLLBAR_WIDTH),
            stored: self.offset,
        }
    }
}
