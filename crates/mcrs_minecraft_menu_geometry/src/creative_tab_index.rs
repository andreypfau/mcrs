use crate::CREATIVE_TABS;
use crate::CreativeTab;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CreativeTabIndex {
    index: usize,
    tab: &'static CreativeTab,
}

impl CreativeTabIndex {
    pub const FIRST: Self = {
        let [tab, ..] = &CREATIVE_TABS;
        Self { index: 0, tab }
    };

    #[must_use]
    pub const fn new(index: usize) -> Option<Self> {
        let tabs: &'static [CreativeTab] = &CREATIVE_TABS;
        match tabs.split_at_checked(index) {
            Some((_, [tab, ..])) => Some(Self { index, tab }),
            _ => None,
        }
    }

    pub fn all() -> impl Iterator<Item = Self> {
        let tabs: &'static [CreativeTab] = &CREATIVE_TABS;
        tabs.iter()
            .enumerate()
            .map(|(index, tab)| Self { index, tab })
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self.index
    }

    #[must_use]
    pub const fn tab(self) -> &'static CreativeTab {
        self.tab
    }
}
