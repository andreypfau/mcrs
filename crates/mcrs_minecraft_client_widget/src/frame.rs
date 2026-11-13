use crate::child::Child;
use crate::gui_extent::GuiExtent;
use crate::gui_rect::GuiRect;

/// Children that overlap, each placed in the whole frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Frame<'a> {
    pub min_size: GuiExtent,
    pub children: &'a [Child],
}

impl<'a> Frame<'a> {
    /// The largest of the minimum size and the cell sizes, on each axis.
    #[must_use]
    pub fn size(self) -> GuiExtent {
        self.children.iter().fold(self.min_size, |size, child| {
            let cell = child.cell_size();
            GuiExtent::new(size.width.max(cell.width), size.height.max(cell.height))
        })
    }

    /// The bounds of the children, in order, for the frame at (`x`, `y`).
    pub fn rects(self, x: i32, y: i32) -> impl Iterator<Item = GuiRect> + 'a {
        let size = self.size();
        let area = GuiRect::new(x, y, size.width, size.height);
        self.children
            .iter()
            .map(move |child| child.cell.place(child.size, area))
    }
}
