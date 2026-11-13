use core::f64::consts::{FRAC_PI_2, TAU};

use crate::gui_point::GuiPoint;
use crate::gui_rect::GuiRect;
use crate::saturate;
use crate::standard_sizes::LINE_HEIGHT;

/// Where a caption draws: the start of its text, and the label area that cuts it, if any.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptionLayout {
    pub origin: GuiPoint,
    pub clip: Option<GuiRect>,
}

/// How far left the label scroll has moved a caption that is `overflow` GUI units wider than its
/// label area, at `now_ms` on the clock of the client (WID-BTN-005).
#[must_use]
fn label_scroll_offset(overflow: i32, now_ms: u64) -> i32 {
    let overflow = f64::from(overflow);
    let period = (overflow * 0.5).max(3.0);
    let seconds = now_ms as f64 / 1000.0;
    let amount = libm::sin(FRAC_PI_2 * libm::cos(TAU * seconds / period)) / 2.0 + 0.5;
    (amount * overflow) as i32
}

pub(crate) fn label(area: GuiRect, centre: i32, text_width: i32, now_ms: u64) -> CaptionLayout {
    if text_width > area.width {
        return scrolled(area, text_width, now_ms);
    }
    let left = i64::from(area.x);
    let right = left + i64::from(area.width);
    let half = i64::from(text_width / 2);
    let low = left + half;
    let high = right - half;
    let centre = i64::from(centre);
    let clamped = if centre < low { low } else { centre.min(high) };
    CaptionLayout {
        origin: GuiPoint::new(saturate::saturate(clamped - half), text_top(area)),
        clip: Some(area),
    }
}

pub(crate) fn scrolled(area: GuiRect, text_width: i32, now_ms: u64) -> CaptionLayout {
    let overflow = text_width.saturating_sub(area.width).max(0);
    CaptionLayout {
        origin: GuiPoint::new(
            area.x.saturating_sub(label_scroll_offset(overflow, now_ms)),
            text_top(area),
        ),
        clip: Some(area),
    }
}

fn text_top(area: GuiRect) -> i32 {
    let top = i64::from(area.y);
    let bottom = top + i64::from(area.height);
    saturate::saturate((top + bottom - i64::from(LINE_HEIGHT)) / 2 + 1)
}
