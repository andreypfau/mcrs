use crate::CREATIVE_TABS;
use crate::CreativeTab;
use crate::CreativeTabIndex;
use crate::CreativeTabKind;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VisibleTabs(u16);

impl VisibleTabs {
    pub const NONE: Self = Self(0);

    #[must_use]
    pub const fn always() -> Self {
        let mut visible = Self::NONE;
        let mut index = 0;
        let mut tabs: &[CreativeTab] = &CREATIVE_TABS;
        while let [tab, rest @ ..] = tabs {
            if !matches!(tab.kind, CreativeTabKind::Items) {
                visible = visible.with_bit(index);
            }
            index += 1;
            tabs = rest;
        }
        visible
    }

    #[must_use]
    pub const fn with(self, tab: CreativeTabIndex) -> Self {
        self.with_bit(tab.index())
    }

    #[must_use]
    pub const fn contains(self, tab: CreativeTabIndex) -> bool {
        match bit(tab.index()) {
            Some(bit) => self.0 & bit != 0,
            None => false,
        }
    }

    const fn with_bit(self, index: usize) -> Self {
        match bit(index) {
            Some(bit) => Self(self.0 | bit),
            None => self,
        }
    }
}

const fn bit(index: usize) -> Option<u16> {
    if index < u16::BITS as usize {
        Some(1u16 << index)
    } else {
        None
    }
}
