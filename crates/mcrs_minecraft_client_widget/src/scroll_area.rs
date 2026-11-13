use crate::gui_rect::GuiRect;
use crate::saturate;
use crate::scrollbar::Scrollbar;

pub(crate) const SCROLLBAR_WIDTH: i32 = 6;
const MIN_THUMB_HEIGHT: i64 = 32;
const ROW_MARGIN: i64 = 2;

/// The geometry of one scroll area with its stored scroll offset.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ScrollArea {
    pub(crate) rect: GuiRect,
    pub(crate) content_height: i32,
    pub(crate) wheel_step: i32,
    pub(crate) scrollbar_x: i32,
    pub(crate) stored: f64,
}

impl ScrollArea {
    pub(crate) fn max_offset(self) -> f64 {
        (i64::from(self.content_height) - i64::from(self.rect.height)).max(0) as f64
    }

    pub(crate) fn clamp(self, offset: f64) -> f64 {
        let max = self.max_offset();
        if offset > max {
            max
        } else if offset > 0.0 {
            offset
        } else {
            0.0
        }
    }

    pub(crate) fn offset(self) -> f64 {
        self.clamp(self.stored)
    }

    pub(crate) fn shift(self) -> i32 {
        libm::floor(self.offset()) as i32
    }

    pub(crate) fn wheel(self, down: f64) -> f64 {
        self.clamp(self.stored - down * f64::from(self.wheel_step))
    }

    pub(crate) fn scrollbar(self) -> Option<Scrollbar> {
        let max = self.max_offset() as i64;
        if max <= 0 {
            return None;
        }
        let rect = self.rect;
        let top = i64::from(rect.y);
        let thumb_height = self.thumb_height();
        let travel = (i64::from(self.shift()) * (i64::from(rect.height) - thumb_height))
            .checked_div_euclid(max)
            .unwrap_or(0);
        let thumb_top = top.max(top + travel);
        Some(Scrollbar {
            track: GuiRect::new(self.scrollbar_x, rect.y, SCROLLBAR_WIDTH, rect.height),
            thumb: GuiRect::new(
                self.scrollbar_x,
                saturate::saturate(thumb_top),
                SCROLLBAR_WIDTH,
                saturate::saturate(thumb_height),
            ),
        })
    }

    pub(crate) fn on_scrollbar(self, x: f64, y: f64) -> bool {
        let left = f64::from(self.scrollbar_x);
        let top = f64::from(self.rect.y);
        self.max_offset() > 0.0
            && left <= x
            && x <= left + f64::from(SCROLLBAR_WIDTH)
            && top <= y
            && y < top + f64::from(self.rect.height)
    }

    pub(crate) fn dragged(self, y: f64, dy: f64) -> f64 {
        let top = f64::from(self.rect.y);
        let max = self.max_offset();
        if y < top {
            0.0
        } else if y > top + f64::from(self.rect.height) {
            max
        } else {
            let travel = (i64::from(self.rect.height) - self.thumb_height()).max(1) as f64;
            self.clamp(self.stored + dy * max.max(1.0) / travel)
        }
    }

    /// LIST-SEL-003 for a row from `top` to `bottom` at the scroll offset 0.
    pub(crate) fn revealed_row(self, top: i32, bottom: i32, keyboard: bool) -> f64 {
        let area_top = i64::from(self.rect.y);
        let area_bottom = area_top + i64::from(self.rect.height);
        let shown = |offset: f64, edge: i32| i64::from(edge) - libm::floor(offset) as i64;
        let mut offset = self.stored;
        let partly_out = shown(offset, top) + ROW_MARGIN < area_top
            || shown(offset, bottom) - ROW_MARGIN > area_bottom;
        if !keyboard && !partly_out {
            return self.offset();
        }
        let above = shown(offset, top) - area_top - ROW_MARGIN;
        if above < 0 {
            offset = self.clamp(offset + above as f64);
        }
        let below = area_bottom - shown(offset, bottom) - ROW_MARGIN;
        if below < 0 {
            offset = self.clamp(offset - below as f64);
        }
        self.clamp(offset)
    }

    /// SCRL-PANEL-002 for a widget from `top` to `bottom` at the scroll offset 0.
    pub(crate) fn revealed_widget(self, top: i32, bottom: i32, keyboard: bool) -> f64 {
        let shift = libm::floor(self.stored) as i64;
        let area_top = i64::from(self.rect.y);
        let area_bottom = area_top + i64::from(self.rect.height);
        let (top, bottom) = (i64::from(top) - shift, i64::from(bottom) - shift);
        let step = i64::from(self.wheel_step);
        if !keyboard {
            self.offset()
        } else if top < area_top {
            self.clamp(self.stored + (top - area_top - step) as f64)
        } else if bottom > area_bottom {
            self.clamp(self.stored + (bottom - area_bottom + step) as f64)
        } else {
            self.offset()
        }
    }

    fn thumb_height(self) -> i64 {
        let height = i64::from(self.rect.height);
        (height * height)
            .checked_div(i64::from(self.content_height))
            .unwrap_or(0)
            .max(MIN_THUMB_HEIGHT)
            .min((height - 8).max(MIN_THUMB_HEIGHT))
    }
}
