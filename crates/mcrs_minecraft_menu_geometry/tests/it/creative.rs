use mcrs_minecraft_client_widget::GuiPoint;
use mcrs_minecraft_keys::creative_mode_tab;
use mcrs_minecraft_menu_geometry::CREATIVE_TABS;
use mcrs_minecraft_menu_geometry::CreativeGeometry;
use mcrs_minecraft_menu_geometry::CreativeHit;
use mcrs_minecraft_menu_geometry::CreativeTabIndex;
use mcrs_minecraft_menu_geometry::CreativeTabKind;
use mcrs_minecraft_menu_geometry::VisibleTabs;

const BUILDING_BLOCKS: CreativeTabIndex = tab(0);
const COLORED_BLOCKS: CreativeTabIndex = tab(1);
const SEARCH: CreativeTabIndex = tab(6);
const INVENTORY: CreativeTabIndex = tab(13);

#[expect(
    clippy::panic,
    reason = "a constant evaluation fails the build for an index out of the tab table"
)]
const fn tab(index: usize) -> CreativeTabIndex {
    match CreativeTabIndex::new(index) {
        Some(tab) => tab,
        None => panic!("the creative tabs hold the index"),
    }
}

#[test]
fn the_tabs_follow_the_registry_order() {
    let ids: Vec<_> = CREATIVE_TABS.iter().map(|tab| tab.id).collect();
    assert_eq!(ids, creative_mode_tab::ENTRIES);
}

#[test]
fn the_tab_columns_give_the_x_positions() {
    let positions = [0, 27, 54, 81, 108, 142, 169];
    for tab in &CREATIVE_TABS {
        assert_eq!(tab.x(), positions[usize::from(tab.column)], "{}", tab.id);
    }
}

#[test]
fn the_tab_row_and_column_name_the_sprites() {
    let cases = [
        (
            BUILDING_BLOCKS,
            "tab_top_selected_1",
            "tab_top_unselected_1",
        ),
        (SEARCH, "tab_top_selected_7", "tab_top_unselected_7"),
        (
            INVENTORY,
            "tab_bottom_selected_7",
            "tab_bottom_unselected_7",
        ),
    ];
    for (index, selected, unselected) in cases {
        let tab = index.tab();
        assert_eq!(
            tab.selected_sprite().path(),
            format!("container/creative_inventory/{selected}"),
        );
        assert_eq!(
            tab.unselected_sprite().path(),
            format!("container/creative_inventory/{unselected}"),
        );
    }
}

#[test]
fn the_slots_have_their_item_positions() {
    let cases = [
        (CreativeGeometry::grid_slot(0), Some((9, 18))),
        (CreativeGeometry::grid_slot(8), Some((153, 18))),
        (CreativeGeometry::grid_slot(44), Some((153, 90))),
        (CreativeGeometry::grid_slot(45), None),
        (CreativeGeometry::hotbar_slot(8), Some((153, 112))),
        (CreativeGeometry::hotbar_slot(9), None),
        (CreativeGeometry::inventory_entry(4), None),
        (CreativeGeometry::inventory_entry(5), Some((54, 6))),
        (CreativeGeometry::inventory_entry(8), Some((108, 33))),
        (CreativeGeometry::inventory_entry(9), Some((9, 54))),
        (CreativeGeometry::inventory_entry(35), Some((153, 90))),
        (CreativeGeometry::inventory_entry(36), Some((9, 112))),
        (CreativeGeometry::inventory_entry(44), Some((153, 112))),
        (CreativeGeometry::inventory_entry(45), Some((35, 20))),
        (CreativeGeometry::inventory_entry(46), None),
    ];
    for (index, (corner, expected)) in cases.into_iter().enumerate() {
        assert_eq!(
            corner,
            expected.map(|(x, y)| GuiPoint::new(x, y)),
            "case {index}"
        );
    }
}

#[test]
fn the_pointer_hits_one_part_of_the_screen() {
    use CreativeHit::*;
    use CreativeTabKind::*;
    let all = CreativeTabIndex::all().fold(VisibleTabs::always(), VisibleTabs::with);
    let without_colored = CreativeTabIndex::all()
        .filter(|&tab| tab != COLORED_BLOCKS)
        .fold(VisibleTabs::NONE, VisibleTabs::with);
    let window = CreativeGeometry::window(427, 240);
    assert_eq!(window, GuiPoint::new(116, 52));
    let cases = [
        (0, 0, Items, all, Tab(BUILDING_BLOCKS)),
        (0, 0, Items, VisibleTabs::always(), Empty),
        (26, -32, Items, all, Tab(BUILDING_BLOCKS)),
        (27, -32, Items, all, Tab(COLORED_BLOCKS)),
        (27, -32, Items, without_colored, Outside),
        (0, -33, Items, all, Outside),
        (195, 0, Items, all, Tab(SEARCH)),
        (196, 0, Items, all, Outside),
        (169, 136, Items, all, Tab(INVENTORY)),
        (195, 168, Items, all, Tab(INVENTORY)),
        (169, 169, Items, all, Outside),
        (-1, 60, Items, all, Outside),
        (195, 60, Items, all, Outside),
        (9, 18, Items, all, GridSlot(0)),
        (24, 33, Items, all, GridSlot(0)),
        (25, 18, Items, all, Empty),
        (168, 105, Items, all, GridSlot(44)),
        (9, 112, SavedHotbars, all, HotbarSlot(0)),
        (168, 127, Items, all, HotbarSlot(8)),
        (175, 18, Items, all, ScrollBar),
        (188, 129, Items, all, ScrollBar),
        (189, 18, Items, all, Empty),
        (82, 6, Search, all, SearchBox),
        (161, 14, Search, all, SearchBox),
        (82, 6, Items, all, Empty),
        (54, 6, SurvivalInventory, all, InventoryEntry(5)),
        (9, 54, SurvivalInventory, all, InventoryEntry(9)),
        (9, 112, SurvivalInventory, all, InventoryEntry(36)),
        (35, 20, SurvivalInventory, all, InventoryEntry(45)),
        (188, 127, SurvivalInventory, all, DestroySlot),
        (9, 18, SurvivalInventory, all, Empty),
        (175, 18, SurvivalInventory, all, Empty),
    ];
    for (x, y, selected, visible, hit) in cases {
        let pointer = GuiPoint::new(window.x + x, window.y + y);
        assert_eq!(
            CreativeGeometry::hit(window, pointer, selected, visible),
            hit,
            "(left + {x}, top + {y}) in {selected:?}"
        );
    }
}

#[test]
fn the_hover_rectangle_gives_the_tab_title() {
    let all = CreativeTabIndex::all().fold(VisibleTabs::always(), VisibleTabs::with);
    let window = GuiPoint::new(10, 40);
    let cases = [
        (2, -30, all, Some(BUILDING_BLOCKS)),
        (24, -2, all, Some(BUILDING_BLOCKS)),
        (1, -30, all, None),
        (25, -30, all, None),
        (2, -31, all, None),
        (2, -1, all, None),
        (2, -30, VisibleTabs::always(), None),
        (171, 138, all, Some(INVENTORY)),
        (171, 167, all, None),
    ];
    for (x, y, visible, tab) in cases {
        let pointer = GuiPoint::new(window.x + x, window.y + y);
        assert_eq!(
            CreativeGeometry::tab_under(window, pointer, visible),
            tab,
            "(left + {x}, top + {y})"
        );
    }
}

#[test]
fn the_scroll_offset_selects_the_first_row_and_the_thumb() {
    assert_eq!(CreativeGeometry::row_count(45), 0);
    assert_eq!(CreativeGeometry::row_count(46), 1);
    assert_eq!(CreativeGeometry::first_row(0.5, 3), 2);
    assert_eq!(CreativeGeometry::first_row(0.49, 3), 1);
    assert_eq!(CreativeGeometry::first_row(1.0, -5), 0);
    assert_eq!(CreativeGeometry::thumb(1.0).y, 113);
    assert_eq!(CreativeGeometry::scroll_from_drag(52, 52.0 + 25.5), 0.0);
    assert_eq!(CreativeGeometry::scroll_from_drag(52, 52.0 + 122.5), 1.0);
    assert_eq!(CreativeGeometry::scroll_from_drag(52, 52.0 + 74.0), 0.5);
    assert_eq!(
        CreativeGeometry::thumb_sprite(CreativeTabKind::Items, 46),
        Some(CreativeGeometry::SCROLLER)
    );
    assert_eq!(
        CreativeGeometry::thumb_sprite(CreativeTabKind::Search, 45),
        Some(CreativeGeometry::SCROLLER_DISABLED)
    );
    assert_eq!(
        CreativeGeometry::thumb_sprite(CreativeTabKind::SurvivalInventory, 100),
        None
    );
}
