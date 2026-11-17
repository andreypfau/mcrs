use std::collections::HashSet;

use mcrs_minecraft_inventory_menu::MenuEntry;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_inventory_menu::MenuOrder;
use mcrs_minecraft_inventory_menu::MenuRange;
use mcrs_minecraft_inventory_menu::Route;
use mcrs_minecraft_inventory_menu::RouteWhen;
use mcrs_minecraft_inventory_menu::menu_keys::MenuType;
use mcrs_minecraft_inventory_player::ArmourSlot;
use mcrs_minecraft_inventory_player::EquipSlot;

const PLAYER: u8 = 0;
const CRAFT_GRID: u8 = 1;
const CRAFT_RESULT: u8 = 2;

fn entry(part: u8, index: u16) -> MenuEntry {
    MenuEntry { part, index }
}

fn inventory(index: u16) -> Option<MenuEntry> {
    Some(entry(PLAYER, index))
}

fn quick_move_destinations(
    source: u16,
    equip: Option<EquipSlot>,
    empty: &[u16],
) -> Option<Vec<u16>> {
    let route = MenuLayout::INVENTORY
        .routes(source)
        .find(|route| route.when.holds(equip, |target| empty.contains(&target)))?;
    Some(route.to.iter().collect())
}

#[test]
fn inv_mnu_001_entries_refer_to_the_slots_of_the_table() {
    let cases = [
        (0, Some(entry(CRAFT_RESULT, 0))),
        (1, Some(entry(CRAFT_GRID, 0))),
        (4, Some(entry(CRAFT_GRID, 3))),
        (5, inventory(39)),
        (6, inventory(38)),
        (7, inventory(37)),
        (8, inventory(36)),
        (9, inventory(9)),
        (35, inventory(35)),
        (36, inventory(0)),
        (44, inventory(8)),
        (45, inventory(40)),
        (46, None),
        (u16::MAX, None),
    ];
    for (index, slot) in cases {
        assert_eq!(
            MenuLayout::INVENTORY.entry(index),
            slot,
            "menu index {index}"
        );
    }
}

#[test]
fn inv_mnu_001_each_slot_of_the_holders_has_exactly_one_entry() {
    let layout = MenuLayout::INVENTORY;
    assert_eq!(layout.len(), 46, "the inventory menu has 46 entries");
    let mut seen = HashSet::new();
    for index in 0..layout.len() {
        let slot = layout.entry(index).unwrap();
        assert!(seen.insert(slot), "{slot:?} has two entries");
        assert_eq!(layout.index_of(slot), Some(index), "{slot:?}");
    }
    let outside = [
        entry(CRAFT_RESULT, 1),
        entry(CRAFT_GRID, 4),
        entry(PLAYER, 41),
    ];
    for slot in outside {
        assert_eq!(layout.index_of(slot), None, "{slot:?}");
    }
}

#[test]
fn inv_mnu_004_quick_move_destinations_follow_the_first_true_row() {
    let up = |start: u16, end: u16| (start..end).collect::<Vec<u16>>();
    let down = |start: u16, end: u16| (start..end).rev().collect::<Vec<u16>>();
    let none: &[u16] = &[];
    let all_empty: &[u16] = &[5, 6, 7, 8, 45];
    let cases: [(u16, Option<EquipSlot>, &[u16], Vec<u16>); 14] = [
        (0, None, none, down(9, 45)),
        (
            0,
            Some(EquipSlot::Armour(ArmourSlot::Head)),
            all_empty,
            down(9, 45),
        ),
        (1, None, none, up(9, 45)),
        (
            4,
            Some(EquipSlot::Armour(ArmourSlot::Feet)),
            all_empty,
            up(9, 45),
        ),
        (
            20,
            Some(EquipSlot::Armour(ArmourSlot::Head)),
            all_empty,
            vec![5],
        ),
        (
            20,
            Some(EquipSlot::Armour(ArmourSlot::Chest)),
            all_empty,
            vec![6],
        ),
        (
            40,
            Some(EquipSlot::Armour(ArmourSlot::Legs)),
            all_empty,
            vec![7],
        ),
        (
            45,
            Some(EquipSlot::Armour(ArmourSlot::Feet)),
            all_empty,
            vec![8],
        ),
        (36, Some(EquipSlot::OffHand), all_empty, vec![45]),
        (
            20,
            Some(EquipSlot::Armour(ArmourSlot::Head)),
            &[6, 7, 8, 45],
            up(36, 45),
        ),
        (9, None, none, up(36, 45)),
        (44, None, none, up(9, 36)),
        (45, Some(EquipSlot::OffHand), none, up(9, 45)),
        (
            6,
            Some(EquipSlot::Armour(ArmourSlot::Head)),
            all_empty,
            up(9, 45),
        ),
    ];
    for (source, equip, empty, expected) in cases {
        let destinations = quick_move_destinations(source, equip, empty);
        assert_eq!(destinations, Some(expected), "source {source}, {equip:?}");
    }
    assert_eq!(
        quick_move_destinations(46, None, &[]),
        None,
        "46 is not an entry"
    );
}

#[test]
fn inv_mnu_005_pick_up_all_skips_the_craft_result_and_drag_permits_each_entry() {
    let cases = [
        (0, false, true),
        (1, true, true),
        (45, true, true),
        (46, false, false),
    ];
    for (index, pick_up_all, drag) in cases {
        assert_eq!(
            MenuLayout::INVENTORY.permits_pick_up_all(index),
            pick_up_all,
            "entry {index}"
        );
        assert_eq!(
            MenuLayout::INVENTORY.permits_drag(index),
            drag,
            "entry {index}"
        );
    }
}

#[test]
fn a_new_layout_refuses_too_many_entries_and_routes_outside_its_entries() {
    let up = |start: u16, end: u16| MenuRange::new(start, end, MenuOrder::Ascending);
    let route = |from: MenuRange, to: MenuRange| Route {
        from,
        when: RouteWhen::Always,
        to,
    };
    let wears = |target: u16| Route {
        from: up(0, 27),
        when: RouteWhen::Wears {
            slot: EquipSlot::OffHand,
            target,
        },
        to: up(27, 63),
    };
    let chest = route(up(0, 27), up(27, 63));
    let cases: [(&str, u16, Vec<Route>, Option<u16>); 7] = [
        (
            "27 + 36 entries",
            63,
            vec![chest, route(up(27, 63), up(0, 27))],
            Some(63),
        ),
        ("128 entries", 128, vec![], Some(128)),
        ("129 entries", 129, vec![], None),
        (
            "from ends past the entries",
            63,
            vec![route(up(27, 64), up(0, 27))],
            None,
        ),
        (
            "to starts after it ends",
            63,
            vec![route(up(0, 27), up(40, 30))],
            None,
        ),
        ("Wears names the last entry", 63, vec![wears(62)], Some(63)),
        ("Wears names no entry", 63, vec![wears(63)], None),
    ];
    for (name, len, routes, expected) in cases {
        let entries = (0..len).map(|index| entry(PLAYER, index)).collect();
        let layout = MenuLayout::new(MenuType::Generic9x3, entries, routes);
        assert_eq!(layout.as_ref().map(MenuLayout::len), expected, "{name}");
        if let Some(layout) = layout {
            assert_eq!(layout.kind(), Some(MenuType::Generic9x3), "{name}");
            let last = layout.len() - 1;
            assert!(layout.permits_pick_up_all(last), "{name}: the last entry");
            assert!(layout.permits_drag(last), "{name}: the last entry");
            assert!(
                !layout.permits_drag(layout.len()),
                "{name}: past the last entry"
            );
        }
    }
}
