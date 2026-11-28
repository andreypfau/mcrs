use crate::gui_extent::GuiExtent;
use crate::standard_sizes::LINE_HEIGHT;

#[must_use]
pub fn text_line_extent(text_width: i32, max_width: Option<i32>) -> GuiExtent {
    let width = max_width.map_or(text_width, |max_width| text_width.min(max_width));
    GuiExtent::new(width, LINE_HEIGHT)
}

#[must_use]
pub fn text_lines_extent(line_widths: impl IntoIterator<Item = i32>) -> GuiExtent {
    let mut extent = GuiExtent::default();
    for width in line_widths {
        extent.width = extent.width.max(width);
        extent.height = extent.height.saturating_add(LINE_HEIGHT);
    }
    extent
}
