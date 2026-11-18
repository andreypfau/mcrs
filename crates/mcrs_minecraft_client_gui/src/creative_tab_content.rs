use bevy::prelude::Resource;
use mcrs_minecraft_creative_content::CreativeContent;
use mcrs_minecraft_creative_content::OperatorCondition;
use mcrs_minecraft_creative_content::SessionInputs;
use mcrs_minecraft_creative_report::CreativeReport;
use mcrs_minecraft_engine_registries::RegistryNames;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_menu_geometry::CREATIVE_TABS;
use mcrs_minecraft_menu_geometry::CreativeTabKind;
use mcrs_minecraft_menu_geometry::VisibleTabs;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::rl;

use crate::creative_search_index::CreativeSearchIndex;
use crate::language::ClientLanguage;

#[derive(Clone, Copy, Debug)]
pub struct CreativeInputs<'a> {
    pub report: Option<&'a CreativeReport>,
    pub registries: &'a Registries,
    pub items: &'a ItemTable,
    pub language: Option<&'a ClientLanguage>,
    pub operator: OperatorCondition,
}

#[derive(Resource, Clone, Debug)]
pub struct CreativeTabContent {
    lists: CreativeContent,
    search: CreativeSearchIndex,
    visible: VisibleTabs,
    operator: OperatorCondition,
}

impl Default for CreativeTabContent {
    fn default() -> Self {
        Self {
            lists: CreativeContent::default(),
            search: CreativeSearchIndex::default(),
            visible: VisibleTabs::always(),
            operator: OperatorCondition::NotMet,
        }
    }
}

impl CreativeTabContent {
    #[must_use]
    pub fn build(inputs: CreativeInputs<'_>) -> Self {
        let enabled_features = [rl!("minecraft:vanilla").to_buf()];
        let names = RegistryNames::new(inputs.registries, inputs.items);
        let lists = match inputs.report {
            Some(report) => CreativeContent::build(
                report,
                SessionInputs {
                    names: &names,
                    items: inputs.items,
                    registries: inputs.registries,
                    enabled_features: &enabled_features,
                    operator: inputs.operator,
                },
            ),
            None => CreativeContent::default(),
        };
        let search = CreativeSearchIndex::build(
            lists.search_list(),
            inputs.items,
            inputs.registries,
            inputs.language,
        );
        let mut visible = VisibleTabs::always();
        for (index, tab) in CREATIVE_TABS.iter().enumerate() {
            if tab.kind == CreativeTabKind::Items && !lists.tab_list(tab.id).is_empty() {
                visible = visible.with(index);
            }
        }
        Self {
            lists,
            search,
            visible,
            operator: inputs.operator,
        }
    }

    #[must_use]
    pub fn tab_list(&self, tab: usize) -> &[ItemStack] {
        CREATIVE_TABS
            .get(tab)
            .map(|tab| self.lists.tab_list(tab.id))
            .unwrap_or_default()
    }

    #[must_use]
    pub fn search_list(&self) -> &[ItemStack] {
        self.lists.search_list()
    }

    #[must_use]
    pub fn search_index(&self) -> &CreativeSearchIndex {
        &self.search
    }

    #[must_use]
    pub fn visible_tabs(&self) -> VisibleTabs {
        self.visible
    }

    #[must_use]
    pub fn operator(&self) -> OperatorCondition {
        self.operator
    }
}
