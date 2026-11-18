use std::collections::BTreeMap;

use mcrs_minecraft_keys::creative_mode_tab;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_resource::rl;

use crate::ItemsTab;
use crate::ListKind;
use crate::ReportError;
use crate::StackList;
use crate::raw_report::RawReport;

const VANILLA: ResourceLocation<'static> = rl!("minecraft:vanilla");

const OTHER_TABS: [ResourceLocation<'static>; 3] = [
    creative_mode_tab::HOTBAR,
    creative_mode_tab::SEARCH,
    creative_mode_tab::INVENTORY,
];

#[derive(Clone, Debug)]
pub struct CreativeReport {
    item_features: BTreeMap<ResourceLocationBuf, Vec<ResourceLocationBuf>>,
    tabs: Vec<ItemsTab>,
}

impl CreativeReport {
    pub fn read(json: &[u8]) -> Result<Self, ReportError> {
        let mut raw: RawReport = serde_json::from_slice(json).map_err(ReportError::Malformed)?;
        let mut tabs: Vec<ItemsTab> = Vec::new();
        for &id in creative_mode_tab::ENTRIES {
            if OTHER_TABS.contains(&id) {
                continue;
            }
            let buf = id.to_buf();
            let Some(tab) = raw.tabs.remove(&buf) else {
                return Err(ReportError::MissingTab(buf));
            };
            tabs.push(ItemsTab {
                needs_operator: tab.needs_operator,
                tab_list: StackList::locate(id, ListKind::TabList, tab.tab_list)?,
                search_list: StackList::locate(id, ListKind::SearchList, tab.search_list)?,
                id: buf,
            });
        }
        if let Some(unknown) = raw.tabs.into_keys().next() {
            return Err(ReportError::UnknownTab(unknown));
        }
        Ok(Self {
            item_features: raw.item_features,
            tabs,
        })
    }

    #[must_use]
    pub fn tabs(&self) -> &[ItemsTab] {
        &self.tabs
    }

    #[must_use]
    pub fn tab(&self, id: ResourceLocation<'_>) -> Option<&ItemsTab> {
        self.tabs.iter().find(|tab| tab.id == id)
    }

    pub fn item_features(
        &self,
        item: ResourceLocation<'_>,
    ) -> impl Iterator<Item = ResourceLocation<'_>> {
        let listed = self
            .item_features
            .iter()
            .find(|(listed_item, _)| **listed_item == item)
            .map(|(_, flags)| flags.as_slice());
        listed
            .unwrap_or_default()
            .iter()
            .map(|flag| flag.as_location())
            .chain(listed.is_none().then_some(VANILLA))
    }
}
