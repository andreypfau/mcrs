use crate::gui_extent::GuiExtent;
use crate::layout_cell::LayoutCell;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Child {
    pub size: GuiExtent,
    pub cell: LayoutCell,
}

impl Child {
    #[must_use]
    pub const fn new(size: GuiExtent, cell: LayoutCell) -> Self {
        Self { size, cell }
    }

    pub(crate) fn cell_size(self) -> GuiExtent {
        let padding = self.cell.padding;
        GuiExtent::new(
            self.size
                .width
                .saturating_add(padding.left)
                .saturating_add(padding.right),
            self.size
                .height
                .saturating_add(padding.top)
                .saturating_add(padding.bottom),
        )
    }
}
