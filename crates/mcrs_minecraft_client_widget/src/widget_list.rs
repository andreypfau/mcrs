use alloc::vec::Vec;

use crate::gui_extent::GuiExtent;
use crate::gui_point::GuiPoint;
use crate::gui_rect::GuiRect;
use crate::list_frame;
use crate::saturate;
use crate::scroll_area::ScrollArea;

const OPTION_ROW_WIDTH: i32 = 310;
const OPTION_ROW_HEIGHT: i32 = 25;
const OPTION_COLUMN_STEP: i64 = 160;
const OPTION_WIDGET_TOP: i32 = 2;
const SECTION_LABEL_HEIGHT: i32 = 18;
const SECTION_LABEL_PADDING: i32 = 8;

/// A list whose rows hold widgets of the screen, which name the list and their row in
/// [`crate::Widget::area`] (LIST-WID-001, LIST-WID-002).
#[derive(Clone, Debug, PartialEq)]
pub struct WidgetList {
    row_width: i32,
    default_row_height: i32,
    heights: Vec<i32>,
    rows_height: i64,
    pub(crate) offset: f64,
}

impl WidgetList {
    /// A list with no rows, rows of `row_width` and the default row height
    /// `default_row_height`, scrolled to the top (LIST-GEO-002).
    #[must_use]
    pub const fn new(row_width: i32, default_row_height: i32) -> Self {
        Self {
            row_width,
            default_row_height,
            heights: Vec::new(),
            rows_height: 0,
            offset: 0.0,
        }
    }

    /// An option list with no rows: rows of 310 and the default row height 25 (LIST-OPT-001).
    #[must_use]
    pub const fn options() -> Self {
        Self::new(OPTION_ROW_WIDTH, OPTION_ROW_HEIGHT)
    }

    /// The row height of a row that the screen gives no other height.
    #[must_use]
    pub const fn default_row_height(&self) -> i32 {
        self.default_row_height
    }

    /// The number of rows.
    #[must_use]
    pub fn rows(&self) -> usize {
        self.heights.len()
    }

    /// Adds a row of `height` after the others and returns its index; the scroll offset stays
    /// (SCRL-AREA-004).
    pub fn push_row(&mut self, height: i32) -> usize {
        let row = self.heights.len();
        self.heights.push(height);
        self.rows_height = self.rows_height.saturating_add(i64::from(height));
        row
    }

    /// Adds a section label row after the others and returns its index (LIST-OPT-002).
    pub fn push_section_label_row(&mut self) -> usize {
        let space = if self.heights.is_empty() {
            0
        } else {
            SECTION_LABEL_HEIGHT
        };
        self.push_row(SECTION_LABEL_HEIGHT + space)
    }

    /// The rectangle of row `row` of a list at `area` with the scroll offset 0, which is where
    /// the screen places its widgets (LIST-GEO-002).
    #[must_use]
    pub fn row_rect(&self, area: GuiRect, row: usize) -> Option<GuiRect> {
        let height = *self.heights.get(row)?;
        // chisle: a row top sums the heights above it; a prefix sum lifts it for long lists.
        let above: i64 = self.heights.iter().take(row).map(|h| i64::from(*h)).sum();
        Some(GuiRect::new(
            list_frame::row_left(area, self.row_width),
            saturate::saturate(i64::from(list_frame::rows_top(area)) + above),
            self.row_width,
            height,
        ))
    }

    /// The top left corner of the widget in column `column`, from 0, of the option row `row` on a
    /// screen `screen_width` wide (LIST-OPT-001).
    #[must_use]
    pub fn option_origin(screen_width: i32, row: GuiRect, column: i32) -> GuiPoint {
        let left = i64::from(screen_width).div_euclid(2) - i64::from(OPTION_ROW_WIDTH / 2)
            + OPTION_COLUMN_STEP * i64::from(column);
        GuiPoint::new(
            saturate::saturate(left),
            row.y.saturating_add(OPTION_WIDGET_TOP),
        )
    }

    /// The bounds of the label of the section label row `row` with a text of `text`, on a
    /// screen `screen_width` wide (LIST-OPT-002).
    #[must_use]
    pub fn section_label_rect(
        &self,
        area: GuiRect,
        row: usize,
        screen_width: i32,
        text: GuiExtent,
    ) -> Option<GuiRect> {
        let row_rect = self.row_rect(area, row)?;
        let space = if row == 0 { 0 } else { SECTION_LABEL_HEIGHT };
        let width = text.width.saturating_add(SECTION_LABEL_PADDING);
        Some(GuiRect::new(
            saturate::saturate(
                i64::from(screen_width).div_euclid(2) - i64::from(width).div_euclid(2),
            ),
            row_rect
                .y
                .saturating_add(OPTION_WIDGET_TOP)
                .saturating_add(space),
            width,
            text.height.saturating_add(SECTION_LABEL_PADDING),
        ))
    }

    pub(crate) fn scroll_area(&self, area: GuiRect) -> ScrollArea {
        list_frame::scroll_area(
            area,
            self.row_width,
            self.rows_height,
            self.default_row_height,
            self.offset,
        )
    }

    pub(crate) fn row_under(&self, area: GuiRect, x: f64, y: f64) -> Option<usize> {
        if !list_frame::in_rows(area, self.row_width, x, y) {
            return None;
        }
        let shift = f64::from(self.scroll_area(area).shift());
        let mut top = f64::from(list_frame::rows_top(area)) - shift;
        for (row, height) in self.heights.iter().enumerate() {
            let bottom = top + f64::from(*height);
            if top <= y && y < bottom {
                return Some(row);
            }
            top = bottom;
        }
        None
    }
}
