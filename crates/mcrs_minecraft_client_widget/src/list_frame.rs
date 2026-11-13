use crate::gui_point::GuiPoint;
use crate::gui_rect::GuiRect;
use crate::saturate;
use crate::scroll_area::ScrollArea;

const BACKGROUND_TILE: i64 = 32;
const SEPARATOR_HEIGHT: i32 = 2;
const ROW_SPACE: i32 = 2;
const SCROLLBAR_GAP: i32 = 8;

/// What a list draws outside its rows: the background and the two separators (LIST-BG-001,
/// LIST-BG-002).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListFrame {
    /// The point of the 32 × 32 background tile that shows at the top left corner of the list.
    pub background_origin: GuiPoint,
    /// The rectangle of the header separator.
    pub header_separator: GuiRect,
    /// The rectangle of the footer separator.
    pub footer_separator: GuiRect,
}

impl ListFrame {
    pub(crate) fn new(area: GuiRect, shift: i32) -> Self {
        let right = i64::from(area.x) + i64::from(area.width);
        let bottom = i64::from(area.y) + i64::from(area.height);
        Self {
            background_origin: GuiPoint::new(
                saturate::saturate(right.rem_euclid(BACKGROUND_TILE)),
                saturate::saturate((bottom + i64::from(shift)).rem_euclid(BACKGROUND_TILE)),
            ),
            header_separator: GuiRect::new(
                area.x,
                area.y.saturating_sub(SEPARATOR_HEIGHT),
                area.width,
                SEPARATOR_HEIGHT,
            ),
            footer_separator: GuiRect::new(
                area.x,
                area.y.saturating_add(area.height),
                area.width,
                SEPARATOR_HEIGHT,
            ),
        }
    }
}

pub(crate) fn row_left(area: GuiRect, row_width: i32) -> i32 {
    saturate::saturate(
        i64::from(area.x) + i64::from(area.width).div_euclid(2)
            - i64::from(row_width).div_euclid(2),
    )
}

pub(crate) fn scroll_area(
    area: GuiRect,
    row_width: i32,
    rows_height: i64,
    row_height: i32,
    offset: f64,
) -> ScrollArea {
    ScrollArea {
        rect: area,
        content_height: saturate::saturate(rows_height.saturating_add(2 * i64::from(ROW_SPACE))),
        wheel_step: row_height.div_euclid(2),
        scrollbar_x: scrollbar_x(area, row_width),
        stored: offset,
    }
}

fn scrollbar_x(area: GuiRect, row_width: i32) -> i32 {
    row_left(area, row_width)
        .saturating_add(row_width)
        .saturating_add(SCROLLBAR_GAP)
}

pub(crate) fn rows_top(area: GuiRect) -> i32 {
    area.y.saturating_add(ROW_SPACE)
}

pub(crate) fn in_rows(area: GuiRect, row_width: i32, x: f64, y: f64) -> bool {
    let left = f64::from(row_left(area, row_width));
    area.contains_f64(x, y) && left <= x && x < left + f64::from(row_width)
}
