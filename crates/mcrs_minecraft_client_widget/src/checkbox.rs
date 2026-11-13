use crate::gui_point::GuiPoint;
use crate::gui_rect::GuiRect;
use crate::standard_sizes::{CHECKBOX_LABEL_GAP, CHECKBOX_SIZE, LINE_HEIGHT};

const LABEL_ROWS: i32 = 2;

/// The width of a checkbox with a label of `text_width`, at most `max_width` (WID-CHK-001).
#[must_use]
pub fn checkbox_width(text_width: i32, max_width: Option<i32>) -> i32 {
    let width = CHECKBOX_SIZE
        .saturating_add(CHECKBOX_LABEL_GAP)
        .saturating_add(text_width);
    max_width.map_or(width, |max_width| width.min(max_width))
}

/// The height of a checkbox whose label wraps at its width into `lines` lines; a label of more
/// than 2 lines is cut to 2, and only then does the checkbox keep its tooltip (WID-CHK-001,
/// WID-CHK-003).
#[must_use]
pub fn checkbox_height(lines: i32) -> i32 {
    label_height(lines).max(CHECKBOX_SIZE)
}

/// The start of the first label line of a checkbox in `bounds` whose label wraps into `lines`
/// lines (WID-CHK-002).
#[must_use]
pub fn checkbox_label_origin(bounds: GuiRect, lines: i32) -> GuiPoint {
    GuiPoint::new(
        bounds
            .x
            .saturating_add(CHECKBOX_SIZE)
            .saturating_add(CHECKBOX_LABEL_GAP),
        bounds
            .y
            .saturating_add(CHECKBOX_SIZE / 2 - label_height(lines) / 2),
    )
}

fn label_height(lines: i32) -> i32 {
    LINE_HEIGHT * lines.clamp(0, LABEL_ROWS)
}
