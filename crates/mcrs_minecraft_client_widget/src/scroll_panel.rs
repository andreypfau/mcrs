use crate::gui_rect::GuiRect;
use crate::scroll_area::{SCROLLBAR_WIDTH, ScrollArea};

const WHEEL_STEP: i32 = 10;

/// A scroll area over a layout of widgets of the screen, which name the panel in
/// [`crate::Widget::area`] (SCRL-PANEL-001, SCRL-PANEL-002).
#[derive(Clone, Debug, PartialEq)]
pub struct ScrollPanel {
    content_height: i32,
    pub(crate) offset: f64,
}

impl ScrollPanel {
    /// A panel over a layout `content_height` high, scrolled to the top.
    #[must_use]
    pub const fn new(content_height: i32) -> Self {
        Self {
            content_height,
            offset: 0.0,
        }
    }

    /// The height of the layout.
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
