use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::ListKind;
use crate::StackList;

#[derive(Clone, Debug)]
pub struct ItemsTab {
    pub(crate) id: ResourceLocationBuf,
    pub(crate) needs_operator: bool,
    pub(crate) tab_list: StackList,
    pub(crate) search_list: StackList,
}

impl ItemsTab {
    #[must_use]
    pub fn id(&self) -> ResourceLocation<'_> {
        self.id.as_location()
    }

    #[must_use]
    pub fn needs_operator(&self) -> bool {
        self.needs_operator
    }

    #[must_use]
    pub fn list(&self, list: ListKind) -> &StackList {
        match list {
            ListKind::TabList => &self.tab_list,
            ListKind::SearchList => &self.search_list,
        }
    }
}
