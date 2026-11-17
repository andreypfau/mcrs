use crate::gui_rect::GuiRect;
use crate::list_frame;
use crate::saturate;
use crate::scroll_area::ScrollArea;

const DEFAULT_ROW_WIDTH: i32 = 220;

#[derive(Clone, Debug, PartialEq)]
pub struct SelectionList {
    pub(crate) rows: usize,
    row_height: i32,
    row_width: i32,
    pub(crate) selected: Option<usize>,
    pub(crate) offset: f64,
}

impl SelectionList {
    #[must_use]
    pub const fn new(rows: usize, row_height: i32) -> Self {
        Self {
            rows,
            row_height,
            row_width: DEFAULT_ROW_WIDTH,
            selected: None,
            offset: 0.0,
        }
    }

    #[must_use]
    pub const fn with_row_width(mut self, row_width: i32) -> Self {
        self.row_width = row_width;
        self
    }

    #[must_use]
    pub const fn rows(&self) -> usize {
        self.rows
    }

    #[must_use]
    pub const fn row_height(&self) -> i32 {
        self.row_height
    }

    #[must_use]
    pub const fn selected(&self) -> Option<usize> {
        self.selected
    }

    #[must_use]
    pub fn row_rect(&self, area: GuiRect, row: usize) -> Option<GuiRect> {
        if row >= self.rows {
            return None;
        }
        let above = i64::try_from(row).unwrap_or(i64::MAX) * i64::from(self.row_height);
        Some(GuiRect::new(
            list_frame::row_left(area, self.row_width),
            saturate::saturate(i64::from(list_frame::rows_top(area)).saturating_add(above)),
            self.row_width,
            self.row_height,
        ))
    }

    pub(crate) fn scroll_area(&self, area: GuiRect) -> ScrollArea {
        let rows = i64::try_from(self.rows).unwrap_or(i64::MAX);
        let rows_height = rows.saturating_mul(i64::from(self.row_height));
        list_frame::scroll_area(
            area,
            self.row_width,
            rows_height,
            self.row_height,
            self.offset,
        )
    }

    pub(crate) fn row_under(&self, area: GuiRect, x: f64, y: f64) -> Option<usize> {
        if self.row_height <= 0 || !list_frame::in_rows(area, self.row_width, x, y) {
            return None;
        }
        let top = f64::from(list_frame::rows_top(area)) - f64::from(self.scroll_area(area).shift());
        let row = libm::floor((y - top) / f64::from(self.row_height));
        (row >= 0.0 && row < self.rows as f64).then_some(row as usize)
    }

    pub(crate) fn insert_row(&mut self, area: GuiRect, index: usize) {
        if index > self.rows {
            return;
        }
        let before = self.scroll_area(area);
        self.rows = self.rows.saturating_add(1);
        self.selected = self.selected.map(|row| {
            if row >= index {
                row.saturating_add(1)
            } else {
                row
            }
        });
        if index == 0 {
            self.keep_bottom_distance(area, before);
        }
    }

    pub(crate) fn remove_row(&mut self, area: GuiRect, index: usize) {
        if index >= self.rows {
            return;
        }
        let before = self.scroll_area(area);
        self.rows = self.rows.saturating_sub(1);
        self.selected = match self.selected {
            Some(row) if row == index => None,
            Some(row) if row > index => Some(row.saturating_sub(1)),
            selected => selected,
        };
        if index == 0 {
            self.keep_bottom_distance(area, before);
        }
    }

    fn keep_bottom_distance(&mut self, area: GuiRect, before: ScrollArea) {
        let after = self.scroll_area(area);
        self.offset = after.clamp(after.max_offset() - (before.max_offset() - before.offset()));
    }

    pub(crate) fn reveal(&mut self, area: GuiRect, row: usize, keyboard: bool) {
        if let Some(rect) = self.row_rect(area, row) {
            self.offset = self
                .scroll_area(area)
                .revealed_row(rect.y, rect.bottom(), keyboard);
        }
    }
}
