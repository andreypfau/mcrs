use mcrs_minecraft_keys::creative_mode_tab;
use mcrs_minecraft_resource::rl;

use crate::CreativeTab;
use crate::CreativeTabAlignment;
use crate::CreativeTabKind;
use crate::CreativeTabRow;
use crate::TabSprites;

pub const CREATIVE_TABS: [CreativeTab; 14] = [
    CreativeTab {
        id: creative_mode_tab::BUILDING_BLOCKS,
        kind: CreativeTabKind::Items,
        row: CreativeTabRow::Top,
        column: 0,
        alignment: CreativeTabAlignment::Left,
        title_key: "itemGroup.buildingBlocks",
        icon: rl!("minecraft:bricks"),
        sprites: TabSprites {
            selected: rl!("minecraft:container/creative_inventory/tab_top_selected_1"),
            unselected: rl!("minecraft:container/creative_inventory/tab_top_unselected_1"),
        },
    },
    CreativeTab {
        id: creative_mode_tab::COLORED_BLOCKS,
        kind: CreativeTabKind::Items,
        row: CreativeTabRow::Top,
        column: 1,
        alignment: CreativeTabAlignment::Left,
        title_key: "itemGroup.coloredBlocks",
        icon: rl!("minecraft:cyan_wool"),
        sprites: TabSprites {
            selected: rl!("minecraft:container/creative_inventory/tab_top_selected_2"),
            unselected: rl!("minecraft:container/creative_inventory/tab_top_unselected_2"),
        },
    },
    CreativeTab {
        id: creative_mode_tab::NATURAL_BLOCKS,
        kind: CreativeTabKind::Items,
        row: CreativeTabRow::Top,
        column: 2,
        alignment: CreativeTabAlignment::Left,
        title_key: "itemGroup.natural",
        icon: rl!("minecraft:grass_block"),
        sprites: TabSprites {
            selected: rl!("minecraft:container/creative_inventory/tab_top_selected_3"),
            unselected: rl!("minecraft:container/creative_inventory/tab_top_unselected_3"),
        },
    },
    CreativeTab {
        id: creative_mode_tab::FUNCTIONAL_BLOCKS,
        kind: CreativeTabKind::Items,
        row: CreativeTabRow::Top,
        column: 3,
        alignment: CreativeTabAlignment::Left,
        title_key: "itemGroup.functional",
        icon: rl!("minecraft:oak_sign"),
        sprites: TabSprites {
            selected: rl!("minecraft:container/creative_inventory/tab_top_selected_4"),
            unselected: rl!("minecraft:container/creative_inventory/tab_top_unselected_4"),
        },
    },
    CreativeTab {
        id: creative_mode_tab::REDSTONE_BLOCKS,
        kind: CreativeTabKind::Items,
        row: CreativeTabRow::Top,
        column: 4,
        alignment: CreativeTabAlignment::Left,
        title_key: "itemGroup.redstone",
        icon: rl!("minecraft:redstone"),
        sprites: TabSprites {
            selected: rl!("minecraft:container/creative_inventory/tab_top_selected_5"),
            unselected: rl!("minecraft:container/creative_inventory/tab_top_unselected_5"),
        },
    },
    CreativeTab {
        id: creative_mode_tab::HOTBAR,
        kind: CreativeTabKind::SavedHotbars,
        row: CreativeTabRow::Top,
        column: 5,
        alignment: CreativeTabAlignment::Right,
        title_key: "itemGroup.hotbar",
        icon: rl!("minecraft:bookshelf"),
        sprites: TabSprites {
            selected: rl!("minecraft:container/creative_inventory/tab_top_selected_6"),
            unselected: rl!("minecraft:container/creative_inventory/tab_top_unselected_6"),
        },
    },
    CreativeTab {
        id: creative_mode_tab::SEARCH,
        kind: CreativeTabKind::Search,
        row: CreativeTabRow::Top,
        column: 6,
        alignment: CreativeTabAlignment::Right,
        title_key: "itemGroup.search",
        icon: rl!("minecraft:compass"),
        sprites: TabSprites {
            selected: rl!("minecraft:container/creative_inventory/tab_top_selected_7"),
            unselected: rl!("minecraft:container/creative_inventory/tab_top_unselected_7"),
        },
    },
    CreativeTab {
        id: creative_mode_tab::TOOLS_AND_UTILITIES,
        kind: CreativeTabKind::Items,
        row: CreativeTabRow::Bottom,
        column: 0,
        alignment: CreativeTabAlignment::Left,
        title_key: "itemGroup.tools",
        icon: rl!("minecraft:diamond_pickaxe"),
        sprites: TabSprites {
            selected: rl!("minecraft:container/creative_inventory/tab_bottom_selected_1"),
            unselected: rl!("minecraft:container/creative_inventory/tab_bottom_unselected_1"),
        },
    },
    CreativeTab {
        id: creative_mode_tab::COMBAT,
        kind: CreativeTabKind::Items,
        row: CreativeTabRow::Bottom,
        column: 1,
        alignment: CreativeTabAlignment::Left,
        title_key: "itemGroup.combat",
        icon: rl!("minecraft:netherite_sword"),
        sprites: TabSprites {
            selected: rl!("minecraft:container/creative_inventory/tab_bottom_selected_2"),
            unselected: rl!("minecraft:container/creative_inventory/tab_bottom_unselected_2"),
        },
    },
    CreativeTab {
        id: creative_mode_tab::FOOD_AND_DRINKS,
        kind: CreativeTabKind::Items,
        row: CreativeTabRow::Bottom,
        column: 2,
        alignment: CreativeTabAlignment::Left,
        title_key: "itemGroup.foodAndDrink",
        icon: rl!("minecraft:golden_apple"),
        sprites: TabSprites {
            selected: rl!("minecraft:container/creative_inventory/tab_bottom_selected_3"),
            unselected: rl!("minecraft:container/creative_inventory/tab_bottom_unselected_3"),
        },
    },
    CreativeTab {
        id: creative_mode_tab::INGREDIENTS,
        kind: CreativeTabKind::Items,
        row: CreativeTabRow::Bottom,
        column: 3,
        alignment: CreativeTabAlignment::Left,
        title_key: "itemGroup.ingredients",
        icon: rl!("minecraft:iron_ingot"),
        sprites: TabSprites {
            selected: rl!("minecraft:container/creative_inventory/tab_bottom_selected_4"),
            unselected: rl!("minecraft:container/creative_inventory/tab_bottom_unselected_4"),
        },
    },
    CreativeTab {
        id: creative_mode_tab::SPAWN_EGGS,
        kind: CreativeTabKind::Items,
        row: CreativeTabRow::Bottom,
        column: 4,
        alignment: CreativeTabAlignment::Left,
        title_key: "itemGroup.spawnEggs",
        icon: rl!("minecraft:creeper_spawn_egg"),
        sprites: TabSprites {
            selected: rl!("minecraft:container/creative_inventory/tab_bottom_selected_5"),
            unselected: rl!("minecraft:container/creative_inventory/tab_bottom_unselected_5"),
        },
    },
    CreativeTab {
        id: creative_mode_tab::OP_BLOCKS,
        kind: CreativeTabKind::Items,
        row: CreativeTabRow::Bottom,
        column: 5,
        alignment: CreativeTabAlignment::Right,
        title_key: "itemGroup.op",
        icon: rl!("minecraft:command_block"),
        sprites: TabSprites {
            selected: rl!("minecraft:container/creative_inventory/tab_bottom_selected_6"),
            unselected: rl!("minecraft:container/creative_inventory/tab_bottom_unselected_6"),
        },
    },
    CreativeTab {
        id: creative_mode_tab::INVENTORY,
        kind: CreativeTabKind::SurvivalInventory,
        row: CreativeTabRow::Bottom,
        column: 6,
        alignment: CreativeTabAlignment::Right,
        title_key: "itemGroup.inventory",
        icon: rl!("minecraft:chest"),
        sprites: TabSprites {
            selected: rl!("minecraft:container/creative_inventory/tab_bottom_selected_7"),
            unselected: rl!("minecraft:container/creative_inventory/tab_bottom_unselected_7"),
        },
    },
];
