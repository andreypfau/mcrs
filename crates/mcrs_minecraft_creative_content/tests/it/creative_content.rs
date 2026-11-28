use std::fs;
use std::ops::RangeInclusive;
use std::sync::LazyLock;

use mcrs_minecraft_creative_content::CreativeContent;
use mcrs_minecraft_creative_content::OperatorCondition;
use mcrs_minecraft_creative_content::SessionInputs;
use mcrs_minecraft_creative_report::CreativeReport;
use mcrs_minecraft_creative_report::Group;
use mcrs_minecraft_creative_report::ListKind;
use mcrs_minecraft_engine_registries::RegistryNames;
use mcrs_minecraft_engine_registries::test_item_table;
use mcrs_minecraft_game_registries::registry_load::load_registries;
use mcrs_minecraft_game_registries::registry_load::test_additions;
use mcrs_minecraft_game_registries::registry_load::test_packs;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::read_saved;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;

const STORED_REPORT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/mcrs/reports/creative_tabs.json"
);

const REPORT: &str = r#"{
  "item_features": { "minecraft:cyan_wool": ["minecraft:vanilla", "minecraft:trade_rebalance"] },
  "tabs": {
    "minecraft:building_blocks": {
      "needs_operator": false,
      "tab_list": [
        { "id": "minecraft:stone" },
        { "id": "minecraft:stone", "count": 2 },
        { "id": "minecraft:not_an_item" },
        { "id": "minecraft:dirt" }
      ],
      "search_list": [{ "id": "minecraft:stone" }]
    },
    "minecraft:colored_blocks": {
      "needs_operator": false,
      "tab_list": [{ "id": "minecraft:white_wool" }, { "id": "minecraft:cyan_wool" }],
      "search_list": []
    },
    "minecraft:natural_blocks": { "needs_operator": false, "tab_list": [], "search_list": [] },
    "minecraft:functional_blocks": {
      "needs_operator": false,
      "tab_list": [
        { "id": "minecraft:oak_log" },
        { "id": "minecraft:painting", "components": { "minecraft:painting/variant": "minecraft:alban" } },
        { "id": "minecraft:oak_wood" }
      ],
      "search_list": [{ "id": "minecraft:stone" }, { "id": "minecraft:oak_log" }]
    },
    "minecraft:redstone_blocks": { "needs_operator": false, "tab_list": [], "search_list": [] },
    "minecraft:tools_and_utilities": {
      "needs_operator": false,
      "tab_list": [
        { "id": "minecraft:goat_horn" },
        { "id": "minecraft:goat_horn", "components": { "minecraft:instrument": "minecraft:sing_goat_horn" } }
      ],
      "search_list": []
    },
    "minecraft:combat": {
      "needs_operator": false,
      "tab_list": [
        { "id": "minecraft:tipped_arrow", "components": { "minecraft:potion_contents": { "potion": "minecraft:water" } } }
      ],
      "search_list": []
    },
    "minecraft:food_and_drinks": {
      "needs_operator": false,
      "tab_list": [
        { "id": "minecraft:potion", "components": { "minecraft:potion_contents": { "potion": "minecraft:water" } } },
        { "id": "minecraft:splash_potion", "components": { "minecraft:potion_contents": { "potion": "minecraft:water" } } },
        { "id": "minecraft:lingering_potion", "components": { "minecraft:potion_contents": { "potion": "minecraft:water" } } }
      ],
      "search_list": []
    },
    "minecraft:ingredients": {
      "needs_operator": false,
      "tab_list": [
        { "id": "minecraft:enchanted_book", "components": { "minecraft:stored_enchantments": { "minecraft:aqua_affinity": 1 } } }
      ],
      "search_list": [
        { "id": "minecraft:enchanted_book", "components": { "minecraft:stored_enchantments": { "minecraft:aqua_affinity": 1 } } }
      ]
    },
    "minecraft:spawn_eggs": { "needs_operator": false, "tab_list": [], "search_list": [] },
    "minecraft:op_blocks": {
      "needs_operator": true,
      "tab_list": [
        { "id": "minecraft:command_block" },
        { "id": "minecraft:painting", "components": { "minecraft:painting/variant": "minecraft:earth" } }
      ],
      "search_list": [
        { "id": "minecraft:command_block" },
        { "id": "minecraft:painting", "components": { "minecraft:painting/variant": "minecraft:earth" } }
      ]
    }
  }
}"#;

const STONE: &str = r#"{ "id": "minecraft:stone" }"#;
const DIRT: &str = r#"{ "id": "minecraft:dirt" }"#;
const OAK_LOG: &str = r#"{ "id": "minecraft:oak_log" }"#;
const OAK_WOOD: &str = r#"{ "id": "minecraft:oak_wood" }"#;
const WHITE_WOOL: &str = r#"{ "id": "minecraft:white_wool" }"#;
const CYAN_WOOL: &str = r#"{ "id": "minecraft:cyan_wool" }"#;
const COMMAND_BLOCK: &str = r#"{ "id": "minecraft:command_block" }"#;
const GOAT_HORN: &str = r#"{ "id": "minecraft:goat_horn" }"#;

const VANILLA: &[&str] = &["minecraft:vanilla"];
const REBALANCE: &[&str] = &["minecraft:vanilla", "minecraft:trade_rebalance"];

static ITEMS: LazyLock<ItemTable> = LazyLock::new(test_item_table);

static HAND_MADE: LazyLock<CreativeReport> = LazyLock::new(|| {
    CreativeReport::read(REPORT.as_bytes()).expect("the hand-made report is valid")
});

const HUGE_LEVEL_PACK: &str = "huge_level";
const HUGE_LEVEL: &str = "minecraft:zz_huge_level";
const HUGE_LEVEL_FILE: &str = r##"{
  "anvil_cost": 1,
  "description": "huge level",
  "max_cost": { "base": 1, "per_level_above_first": 0 },
  "max_level": 300,
  "min_cost": { "base": 1, "per_level_above_first": 0 },
  "slots": ["any"],
  "supported_items": "#minecraft:enchantable/head_armor",
  "weight": 1
}"##;

static HUGE_LEVEL_REGISTRIES: LazyLock<Registries> = LazyLock::new(|| {
    let mut additions = test_additions();
    additions.files.insert(
        HUGE_LEVEL_PACK,
        "data/minecraft/enchantment/zz_huge_level.json",
        HUGE_LEVEL_FILE,
    );
    additions.embedded_packs.push(HUGE_LEVEL_PACK.to_owned());
    load_registries(&test_packs(), additions)
        .expect("the registries load with the pack of the huge level")
        .0
});

static STORED: LazyLock<CreativeReport> = LazyLock::new(|| {
    let bytes = fs::read(STORED_REPORT).expect("the creative report is in the assets");
    CreativeReport::read(&bytes).expect("the stored creative report is valid")
});

#[derive(Clone, Copy)]
enum Target {
    Tab(&'static str),
    Search,
}

#[derive(Clone, Copy)]
enum Session {
    Vanilla,
    HugeLevel,
}

#[derive(Clone)]
enum Expected {
    Stack(&'static str),
    StoredSpan(Group, ListKind),
    Books {
        enchantment: &'static str,
        levels: RangeInclusive<i32>,
    },
}

struct Row {
    name: &'static str,
    features: &'static [&'static str],
    operator: OperatorCondition,
    session: Session,
    target: Target,
    expected: &'static [Expected],
}

const fn tab(
    name: &'static str,
    features: &'static [&'static str],
    id: &'static str,
    expected: &'static [Expected],
) -> Row {
    Row {
        name,
        features,
        operator: OperatorCondition::Met,
        session: Session::Vanilla,
        target: Target::Tab(id),
        expected,
    }
}

const ROWS: &[Row] = &[
    tab(
        "a stack that is the same stack as an earlier one and a stack that does not decode are removed",
        VANILLA,
        "minecraft:building_blocks",
        &[Expected::Stack(STONE), Expected::Stack(DIRT)],
    ),
    tab(
        "a stack whose item needs a flag that is not enabled is removed",
        VANILLA,
        "minecraft:colored_blocks",
        &[Expected::Stack(WHITE_WOOL)],
    ),
    tab(
        "a stack shows when each flag of its item is enabled",
        REBALANCE,
        "minecraft:colored_blocks",
        &[Expected::Stack(WHITE_WOOL), Expected::Stack(CYAN_WOOL)],
    ),
    tab(
        "an items tab whose stacks the enabled features all disable is empty",
        &[],
        "minecraft:colored_blocks",
        &[],
    ),
    tab(
        "an items tab with no stack in the report is empty",
        VANILLA,
        "minecraft:natural_blocks",
        &[],
    ),
    tab(
        "the placeable paintings are at the place of their span",
        VANILLA,
        "minecraft:functional_blocks",
        &[
            Expected::Stack(OAK_LOG),
            Expected::StoredSpan(Group::PlaceablePaintings, ListKind::TabList),
            Expected::Stack(OAK_WOOD),
        ],
    ),
    tab(
        "the goat horns are the tag in its order, and the horn with the default instrument is the plain horn",
        VANILLA,
        "minecraft:tools_and_utilities",
        &[
            Expected::Stack(GOAT_HORN),
            Expected::StoredSpan(Group::GoatHorns, ListKind::TabList),
        ],
    ),
    tab(
        "the tipped arrows are each potion",
        VANILLA,
        "minecraft:combat",
        &[Expected::StoredSpan(Group::TippedArrows, ListKind::TabList)],
    ),
    tab(
        "the three potion groups are each potion",
        VANILLA,
        "minecraft:food_and_drinks",
        &[
            Expected::StoredSpan(Group::Potions, ListKind::TabList),
            Expected::StoredSpan(Group::SplashPotions, ListKind::TabList),
            Expected::StoredSpan(Group::LingeringPotions, ListKind::TabList),
        ],
    ),
    tab(
        "the tab list holds the enchanted book of the maximum level of each enchantment",
        VANILLA,
        "minecraft:ingredients",
        &[Expected::StoredSpan(
            Group::MaxLevelEnchantedBooks,
            ListKind::TabList,
        )],
    ),
    tab(
        "the operator tab holds its stacks when the operator condition is met",
        VANILLA,
        "minecraft:op_blocks",
        &[
            Expected::Stack(COMMAND_BLOCK),
            Expected::StoredSpan(Group::UnplaceablePaintings, ListKind::TabList),
        ],
    ),
    Row {
        name: "the operator tab is empty when the operator condition is not met",
        features: VANILLA,
        operator: OperatorCondition::NotMet,
        session: Session::Vanilla,
        target: Target::Tab("minecraft:op_blocks"),
        expected: &[],
    },
    tab(
        "a tab that is not an items tab has no tab list",
        VANILLA,
        "minecraft:search",
        &[],
    ),
    Row {
        name: "the search list joins the search lists of the items tabs without a stack two times",
        features: VANILLA,
        operator: OperatorCondition::Met,
        session: Session::Vanilla,
        target: Target::Search,
        expected: &[
            Expected::Stack(STONE),
            Expected::Stack(OAK_LOG),
            Expected::StoredSpan(Group::EnchantedBooks, ListKind::SearchList),
            Expected::Stack(COMMAND_BLOCK),
            Expected::StoredSpan(Group::UnplaceablePaintings, ListKind::SearchList),
        ],
    },
    Row {
        name: "the search list holds no stack of the operator tab when the operator condition is not met",
        features: VANILLA,
        operator: OperatorCondition::NotMet,
        session: Session::Vanilla,
        target: Target::Search,
        expected: &[
            Expected::Stack(STONE),
            Expected::Stack(OAK_LOG),
            Expected::StoredSpan(Group::EnchantedBooks, ListKind::SearchList),
        ],
    },
    Row {
        name: "an enchantment above the level 255 has no book in the tab list",
        features: VANILLA,
        operator: OperatorCondition::Met,
        session: Session::HugeLevel,
        target: Target::Tab("minecraft:ingredients"),
        expected: &[Expected::StoredSpan(
            Group::MaxLevelEnchantedBooks,
            ListKind::TabList,
        )],
    },
    Row {
        name: "an enchantment above the level 255 has the books of the levels 1 to 255 in the search list",
        features: VANILLA,
        operator: OperatorCondition::NotMet,
        session: Session::HugeLevel,
        target: Target::Search,
        expected: &[
            Expected::Stack(STONE),
            Expected::Stack(OAK_LOG),
            Expected::StoredSpan(Group::EnchantedBooks, ListKind::SearchList),
            Expected::Books {
                enchantment: HUGE_LEVEL,
                levels: 1..=255,
            },
        ],
    },
];

fn location(text: &str) -> ResourceLocation<'_> {
    ResourceLocation::read(text).expect("the test names a valid id")
}

fn expected_stacks(expected: &[Expected], names: &RegistryNames<'_>) -> Vec<ItemStack> {
    let mut stacks = Vec::new();
    for entry in expected {
        match entry.clone() {
            Expected::Stack(json) => {
                let saved: NbtCompound = serde_json::from_str(json).expect("the test stack is NBT");
                stacks.push(read_saved(&saved, names).expect("the test stack decodes"));
            }
            Expected::StoredSpan(group, list) => {
                let stored = STORED
                    .tab(group.tab())
                    .expect("the stored report has each items tab")
                    .list(list);
                let span = stored
                    .spans()
                    .iter()
                    .find(|span| span.group == group)
                    .expect("the stored report has a span for each group of its lists");
                let saved = stored
                    .stacks()
                    .get(span.stacks.clone())
                    .expect("a span is inside its list");
                stacks.extend(
                    saved
                        .iter()
                        .map(|saved| read_saved(saved, names).expect("a stored stack decodes")),
                );
            }
            Expected::Books {
                enchantment,
                levels,
            } => {
                stacks.extend(levels.map(|level| {
                    read_saved(&book(enchantment, level), names).expect("a test book decodes")
                }));
            }
        }
    }
    stacks
}

fn book(enchantment: &str, level: i32) -> NbtCompound {
    let levels = NbtCompound {
        child_tags: vec![(enchantment.to_owned(), NbtTag::Int(level))],
    };
    let components = NbtCompound {
        child_tags: vec![(
            "minecraft:stored_enchantments".to_owned(),
            NbtTag::Compound(levels),
        )],
    };
    NbtCompound {
        child_tags: vec![
            (
                "id".to_owned(),
                NbtTag::String("minecraft:enchanted_book".to_owned()),
            ),
            ("components".to_owned(), NbtTag::Compound(components)),
        ],
    }
}

#[test]
fn the_tab_lists_and_the_search_list_come_from_the_report_and_the_session() {
    for row in ROWS {
        let registries: &Registries = match row.session {
            Session::Vanilla => test_registries(),
            Session::HugeLevel => &HUGE_LEVEL_REGISTRIES,
        };
        let names = RegistryNames::new(registries, &ITEMS);
        let features: Vec<ResourceLocationBuf> = row
            .features
            .iter()
            .map(|flag| location(flag).to_buf())
            .collect();
        let content = CreativeContent::build(
            &HAND_MADE,
            SessionInputs {
                names: &names,
                items: &ITEMS,
                registries,
                enabled_features: &features,
                operator: row.operator,
            },
        );
        let built = match row.target {
            Target::Tab(id) => content.tab_list(location(id)),
            Target::Search => content.search_list(),
        };
        let expected = expected_stacks(row.expected, &names);
        assert_eq!(built.len(), expected.len(), "{}: the length", row.name);
        assert_eq!(built, expected.as_slice(), "{}", row.name);
    }
}
