use alloc::vec::Vec;

use mcrs_minecraft_creative_report::CreativeReport;
use mcrs_minecraft_creative_report::ItemsTab;
use mcrs_minecraft_creative_report::ListKind;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_stack::read_saved;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::OperatorCondition;
use crate::SessionInputs;
use crate::group_stacks;
use crate::same_stacks;

#[derive(Clone, Debug, Default)]
pub struct CreativeContent {
    tabs: Vec<TabList>,
    search_list: Vec<ItemStack>,
}

#[derive(Clone, Debug)]
struct TabList {
    tab: ResourceLocationBuf,
    stacks: Vec<ItemStack>,
    search: Vec<ItemStack>,
}

impl CreativeContent {
    #[must_use]
    pub fn build<L: NameLookup>(report: &CreativeReport, session: SessionInputs<'_, L>) -> Self {
        let mut tabs: Vec<TabList> = Vec::with_capacity(report.tabs().len());
        let mut search_list: Vec<ItemStack> = Vec::new();
        for tab in report.tabs() {
            let mut stacks = build_list(report, tab, ListKind::TabList, &session);
            same_stacks::remove_same_stacks(&mut stacks);
            let search = build_list(report, tab, ListKind::SearchList, &session);
            search_list.extend_from_slice(&search);
            tabs.push(TabList {
                tab: tab.id().to_buf(),
                stacks,
                search,
            });
        }
        same_stacks::remove_same_stacks(&mut search_list);
        Self { tabs, search_list }
    }

    #[must_use]
    pub fn tab_list(&self, tab: ResourceLocation<'_>) -> &[ItemStack] {
        self.tabs
            .iter()
            .find(|list| list.tab == tab)
            .map(|list| list.stacks.as_slice())
            .unwrap_or_default()
    }

    #[must_use]
    pub fn tab_search_list(&self, tab: ResourceLocation<'_>) -> &[ItemStack] {
        self.tabs
            .iter()
            .find(|list| list.tab == tab)
            .map(|list| list.search.as_slice())
            .unwrap_or_default()
    }

    #[must_use]
    pub fn search_list(&self) -> &[ItemStack] {
        &self.search_list
    }
}

fn build_list<L: NameLookup>(
    report: &CreativeReport,
    tab: &ItemsTab,
    kind: ListKind,
    session: &SessionInputs<'_, L>,
) -> Vec<ItemStack> {
    let mut stacks: Vec<ItemStack> = Vec::new();
    if tab.needs_operator() && session.operator == OperatorCondition::NotMet {
        return stacks;
    }
    let list = tab.list(kind);
    let saved = list.stacks();
    stacks.reserve(saved.len());
    let mut next = 0;
    for span in list.spans() {
        let fixed = saved.get(next..span.stacks.start).unwrap_or_default();
        stacks.extend(
            fixed
                .iter()
                .filter_map(|stack| read_saved(stack, session.names).ok()),
        );
        group_stacks::push_group_stacks(span.group, session, &mut stacks);
        next = span.stacks.end;
    }
    let rest = saved.get(next..).unwrap_or_default();
    stacks.extend(
        rest.iter()
            .filter_map(|stack| read_saved(stack, session.names).ok()),
    );
    stacks.retain(|stack| is_enabled(report, stack, session));
    stacks
}

fn is_enabled<L: NameLookup>(
    report: &CreativeReport,
    stack: &ItemStack,
    session: &SessionInputs<'_, L>,
) -> bool {
    let Some(item) = session.names.name(NameTable::Item, stack.kind.0) else {
        return false;
    };
    report.item_features(item).all(|flag| {
        session
            .enabled_features
            .iter()
            .any(|enabled| *enabled == flag)
    })
}
