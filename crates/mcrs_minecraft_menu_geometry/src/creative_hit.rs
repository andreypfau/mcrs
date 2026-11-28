use crate::CreativeTabIndex;
use crate::creative_geometry::HOTBAR_MENU_START;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CreativeHit {
    Tab(CreativeTabIndex),
    GridSlot(u16),
    HotbarSlot(u16),
    InventoryEntry(u16),
    DestroySlot,
    ScrollBar,
    SearchBox,
    Empty,
    Outside,
}

impl CreativeHit {
    #[must_use]
    pub const fn menu_index(self) -> Option<u16> {
        match self {
            Self::HotbarSlot(column) => column.checked_add(HOTBAR_MENU_START),
            Self::InventoryEntry(index) => Some(index),
            Self::Tab(_)
            | Self::GridSlot(_)
            | Self::DestroySlot
            | Self::ScrollBar
            | Self::SearchBox
            | Self::Empty
            | Self::Outside => None,
        }
    }
}
