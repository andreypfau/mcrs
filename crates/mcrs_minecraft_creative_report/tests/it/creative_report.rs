use std::fs;
use std::sync::LazyLock;

use mcrs_minecraft_creative_report::CreativeReport;
use mcrs_minecraft_creative_report::Group;
use mcrs_minecraft_creative_report::ListKind;
use mcrs_minecraft_creative_report::ReportError;
use mcrs_minecraft_engine_registries::RegistryNames;
use mcrs_minecraft_engine_registries::test_item_table;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_item_stack::read_saved;
use mcrs_minecraft_resource::ResourceLocation;

const REPORT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/mcrs/reports/creative_tabs.json"
);

const ITEMS_TABS: [&str; 11] = [
    "minecraft:building_blocks",
    "minecraft:colored_blocks",
    "minecraft:natural_blocks",
    "minecraft:functional_blocks",
    "minecraft:redstone_blocks",
    "minecraft:tools_and_utilities",
    "minecraft:combat",
    "minecraft:food_and_drinks",
    "minecraft:ingredients",
    "minecraft:spawn_eggs",
    "minecraft:op_blocks",
];

const LISTS: [ListKind; 2] = [ListKind::TabList, ListKind::SearchList];

static BYTES: LazyLock<Vec<u8>> =
    LazyLock::new(|| fs::read(REPORT).expect("the creative report is in the assets"));

static STORED: LazyLock<CreativeReport> =
    LazyLock::new(|| CreativeReport::read(&BYTES).expect("the stored creative report is valid"));

fn location(text: &str) -> ResourceLocation<'_> {
    ResourceLocation::read(text).expect("the test names a valid id")
}

#[test]
fn each_stack_of_the_report_decodes_with_the_items_and_registries_of_the_pinned_version() {
    let items = test_item_table();
    let names = RegistryNames::new(test_registries(), &items);
    let mut decoded = 0;
    for tab in STORED.tabs() {
        for list in LISTS {
            for saved in tab.list(list).stacks() {
                read_saved(saved, &names).unwrap_or_else(|error| {
                    panic!("{} {list}: {saved:?} does not decode: {error:?}", tab.id())
                });
                decoded += 1;
            }
        }
    }
    assert!(decoded > 4000, "the report lost stacks: {decoded}");
}

#[test]
fn the_report_holds_the_items_tabs_and_only_the_operator_tab_needs_an_operator() {
    let ids: Vec<String> = STORED
        .tabs()
        .iter()
        .map(|tab| tab.id().to_string())
        .collect();
    assert_eq!(ids, ITEMS_TABS);
    let operator: Vec<String> = STORED
        .tabs()
        .iter()
        .filter(|tab| tab.needs_operator())
        .map(|tab| tab.id().to_string())
        .collect();
    assert_eq!(operator, ["minecraft:op_blocks"]);
}

#[test]
fn each_group_has_one_span_in_each_list_that_the_specification_gives_it() {
    let expected: [(Group, &str, &[ListKind]); 9] = [
        (
            Group::PlaceablePaintings,
            "minecraft:functional_blocks",
            &LISTS,
        ),
        (Group::GoatHorns, "minecraft:tools_and_utilities", &LISTS),
        (Group::TippedArrows, "minecraft:combat", &LISTS),
        (Group::Potions, "minecraft:food_and_drinks", &LISTS),
        (Group::SplashPotions, "minecraft:food_and_drinks", &LISTS),
        (Group::LingeringPotions, "minecraft:food_and_drinks", &LISTS),
        (
            Group::MaxLevelEnchantedBooks,
            "minecraft:ingredients",
            &[ListKind::TabList],
        ),
        (
            Group::EnchantedBooks,
            "minecraft:ingredients",
            &[ListKind::SearchList],
        ),
        (Group::UnplaceablePaintings, "minecraft:op_blocks", &LISTS),
    ];
    for tab in STORED.tabs() {
        for list in LISTS {
            let located: Vec<Group> = tab
                .list(list)
                .spans()
                .iter()
                .map(|span| span.group)
                .collect();
            let wanted: Vec<Group> = expected
                .iter()
                .filter(|(_, id, lists)| tab.id() == location(id) && lists.contains(&list))
                .map(|(group, _, _)| *group)
                .collect();
            assert_eq!(located, wanted, "{} {list}", tab.id());
        }
    }
    let books = STORED
        .tab(location("minecraft:ingredients"))
        .expect("the report has the ingredients tab");
    let length = |list: ListKind| -> usize {
        books
            .list(list)
            .spans()
            .iter()
            .map(|span| span.stacks.len())
            .sum()
    };
    assert!(
        length(ListKind::SearchList) > length(ListKind::TabList),
        "the search list holds each level of each enchantment, the tab list only the maximum"
    );
}

const ARROW: &str = r#"{ "id": "minecraft:tipped_arrow", "components": { "minecraft:potion_contents": { "potion": "minecraft:water" } } }"#;

fn tab_json(id: &str, list: &str) -> String {
    format!(r#""{id}": {{ "needs_operator": false, "tab_list": {list}, "search_list": [] }}"#)
}

fn report_json(tabs: &[&str], extra_tab: Option<&str>, combat_tab_list: &str) -> String {
    let mut entries: Vec<String> = tabs
        .iter()
        .map(|id| {
            if *id == "minecraft:combat" {
                tab_json(id, combat_tab_list)
            } else {
                tab_json(id, "[]")
            }
        })
        .collect();
    if let Some(extra_tab) = extra_tab {
        entries.push(tab_json(extra_tab, "[]"));
    }
    format!(
        r#"{{ "item_features": {{ "minecraft:bundle": ["minecraft:bundle"] }}, "tabs": {{ {} }} }}"#,
        entries.join(", ")
    )
}

#[test]
fn a_valid_report_locates_a_group_and_gives_the_listed_flags_or_vanilla() {
    let joined = format!(r#"[{{ "id": "minecraft:arrow" }}, {ARROW}, {ARROW}]"#);
    let valid = CreativeReport::read(report_json(&ITEMS_TABS, None, &joined).as_bytes())
        .expect("a report with one span for each group is valid");
    let combat = valid
        .tab(location("minecraft:combat"))
        .expect("the report has the combat tab");
    let spans = combat.list(ListKind::TabList).spans();
    assert_eq!(spans.len(), 1, "the two arrows are one group");
    assert_eq!(spans[0].stacks, 1..3);
    let flags: Vec<String> = valid
        .item_features(location("minecraft:bundle"))
        .map(|flag| flag.to_string())
        .collect();
    assert_eq!(flags, ["minecraft:bundle"]);
    let flags: Vec<String> = valid
        .item_features(location("minecraft:stone"))
        .map(|flag| flag.to_string())
        .collect();
    assert_eq!(flags, ["minecraft:vanilla"]);
}

#[test]
fn a_report_that_breaks_the_shape_or_splits_a_group_is_rejected() {
    let split = format!(r#"[{ARROW}, {{ "id": "minecraft:arrow" }}, {ARROW}]"#);
    let all_but_combat: Vec<&str> = ITEMS_TABS
        .iter()
        .copied()
        .filter(|id| *id != "minecraft:combat")
        .collect();

    let cases: [(&str, String, fn(&ReportError) -> bool); 4] = [
        (
            "split group",
            report_json(&ITEMS_TABS, None, &split),
            |error| {
                matches!(
                    error,
                    ReportError::SplitGroup {
                        list: ListKind::TabList,
                        group: Group::TippedArrows,
                        ..
                    }
                )
            },
        ),
        (
            "missing tab",
            report_json(&all_but_combat, None, "[]"),
            |error| matches!(error, ReportError::MissingTab(tab) if *tab == "minecraft:combat"),
        ),
        (
            "tab that is not an items tab",
            report_json(&ITEMS_TABS, Some("minecraft:search"), "[]"),
            |error| matches!(error, ReportError::UnknownTab(tab) if *tab == "minecraft:search"),
        ),
        (
            "unknown field",
            report_json(&ITEMS_TABS, None, "[]").replacen('{', r#"{ "groups": [],"#, 1),
            |error| matches!(error, ReportError::Malformed(_)),
        ),
    ];
    for (name, json, expected) in cases {
        let error = CreativeReport::read(json.as_bytes()).expect_err(name);
        assert!(expected(&error), "{name}: {error}");
    }
}
