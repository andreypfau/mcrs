use crate::gui_extent::GuiExtent;
use crate::layout_cell::LayoutCell;

/// A layout element of a fixed size in its cell: a widget, an arranged layout, or a spacer.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Child {
    pub size: GuiExtent,
    pub cell: LayoutCell,
}

impl Child {
    /// The child of `size` in `cell`.
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
