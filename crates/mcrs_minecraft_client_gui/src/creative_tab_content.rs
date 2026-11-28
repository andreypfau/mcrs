use bevy::prelude::Resource;
use mcrs_minecraft_creative_content::CreativeContent;
use mcrs_minecraft_creative_content::OperatorCondition;
use mcrs_minecraft_creative_content::SessionInputs;
use mcrs_minecraft_creative_report::CreativeReport;
use mcrs_minecraft_engine_registries::RegistryNames;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_menu_geometry::CREATIVE_TABS;
use mcrs_minecraft_menu_geometry::CreativeTabIndex;
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
    icons: [Option<ItemStack>; CREATIVE_TABS.len()],
}

impl Default for CreativeTabContent {
    fn default() -> Self {
        Self {
            lists: CreativeContent::default(),
            search: CreativeSearchIndex::default(),
            visible: VisibleTabs::always(),
            operator: OperatorCondition::NotMet,
            icons: [const { None }; CREATIVE_TABS.len()],
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
        let visible = CreativeTabIndex::all()
            .filter(|index| {
                let tab = index.tab();
                tab.kind == CreativeTabKind::Items && !lists.tab_list(tab.id).is_empty()
            })
            .fold(VisibleTabs::always(), VisibleTabs::with);
        let icons = CREATIVE_TABS.map(|tab| {
            let id = inputs.items.id(NameTable::Item, tab.icon)?;
            Some(ItemStack::new(Kind(id), Count::new(1)?))
        });
        Self {
            lists,
            search,
            visible,
            operator: inputs.operator,
            icons,
        }
    }

    #[must_use]
    pub fn tab_list(&self, tab: CreativeTabIndex) -> &[ItemStack] {
        self.lists.tab_list(tab.tab().id)
    }

    pub fn tabs_holding<'a>(
        &'a self,
        stack: &'a ItemStack,
    ) -> impl Iterator<Item = CreativeTabIndex> + 'a {
        CreativeTabIndex::all().filter(move |tab| {
            tab.tab().kind == CreativeTabKind::Items
                && self.visible.contains(*tab)
                && self
                    .lists
                    .tab_search_list(tab.tab().id)
                    .iter()
                    .any(|held| held.mergeable(stack))
        })
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
    pub fn tab_icon(&self, tab: CreativeTabIndex) -> Option<&ItemStack> {
        self.icons.get(tab.index())?.as_ref()
    }

    #[must_use]
    pub fn operator(&self) -> OperatorCondition {
        self.operator
    }
}
