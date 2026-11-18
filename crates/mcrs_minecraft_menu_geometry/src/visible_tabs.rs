use crate::CREATIVE_TABS;
use crate::CreativeTab;
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
                visible = visible.with(index);
            }
            index += 1;
            tabs = rest;
        }
        visible
    }

    #[must_use]
    pub const fn with(self, index: usize) -> Self {
        match bit(index) {
            Some(bit) => Self(self.0 | bit),
            None => self,
        }
    }

    #[must_use]
    pub const fn contains(self, index: usize) -> bool {
        match bit(index) {
            Some(bit) => self.0 & bit != 0,
            None => false,
        }
    }
}

const fn bit(index: usize) -> Option<u16> {
    if index >= u16::BITS as usize {
        return None;
    }
    1u16.checked_shl(index as u32)
}
