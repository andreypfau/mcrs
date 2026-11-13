use std::collections::HashSet;

use mcrs_minecraft_menu::InventoryMenu;
use mcrs_minecraft_menu::MenuSlot;
use mcrs_minecraft_player_slots::ArmourSlot;
use mcrs_minecraft_player_slots::EquipSlot;
use mcrs_minecraft_player_slots::Holder;

fn inventory(index: u16) -> Option<MenuSlot> {
    Some(MenuSlot::new(Holder::Inventory, index))
}

#[test]
fn inv_mnu_001_entries_refer_to_the_slots_of_the_table() {
    let cases = [
        (0, Some(MenuSlot::new(Holder::CraftResult, 0))),
        (1, Some(MenuSlot::new(Holder::CraftGrid, 0))),
        (4, Some(MenuSlot::new(Holder::CraftGrid, 3))),
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
        assert_eq!(InventoryMenu::slot(index), slot, "menu index {index}");
    }
}

#[test]
fn inv_mnu_001_each_slot_of_the_holders_has_exactly_one_entry() {
    let mut seen = HashSet::new();
    for index in 0..InventoryMenu::LEN {
        let slot = InventoryMenu::slot(index).unwrap();
        assert!(seen.insert(slot), "{slot:?} has two entries");
        assert_eq!(InventoryMenu::index(slot), Some(index), "{slot:?}");
    }
    let outside = [
        MenuSlot::new(Holder::CraftResult, 1),
        MenuSlot::new(Holder::CraftGrid, 4),
        MenuSlot::new(Holder::Inventory, 41),
    ];
    for slot in outside {
        assert_eq!(InventoryMenu::index(slot), None, "{slot:?}");
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
        let ranges =
            InventoryMenu::quick_move_destinations(source, equip, |index| empty.contains(&index))
                .unwrap();
        let destinations: Vec<u16> = ranges.iter().flat_map(|range| range.iter()).collect();
        assert_eq!(destinations, expected, "source {source}, {equip:?}");
    }
    assert_eq!(
        InventoryMenu::quick_move_destinations(46, None, |_| true),
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
            InventoryMenu::permits_pick_up_all(index),
            pick_up_all,
            "entry {index}"
        );
        assert_eq!(InventoryMenu::permits_drag(index), drag, "entry {index}");
    }
}
