use crate::CreativeBackground;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CreativeTabKind {
    Items,
    SavedHotbars,
    Search,
    SurvivalInventory,
}

impl CreativeTabKind {
    #[must_use]
    pub const fn has_title(self) -> bool {
        !matches!(self, Self::SurvivalInventory)
    }

    #[must_use]
    pub const fn has_scroll_bar(self) -> bool {
        !matches!(self, Self::SurvivalInventory)
    }

    #[must_use]
    pub const fn background(self) -> CreativeBackground {
        match self {
            Self::Search => CreativeBackground::ItemSearch,
            Self::SurvivalInventory => CreativeBackground::Inventory,
            Self::Items | Self::SavedHotbars => CreativeBackground::Items,
        }
    }
}
