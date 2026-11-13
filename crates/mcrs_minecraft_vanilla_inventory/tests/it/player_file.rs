use std::collections::BTreeMap;

use bevy_ecs::hierarchy::Children;
use mcrs_minecraft_engine::persisted_registry::CaptureCause;
use mcrs_minecraft_engine::persisted_registry::PersistedSnapshot;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_protocol_game::item::ItemStackValue;
use mcrs_minecraft_protocol_game::item::ItemStackWithSlot;
use mcrs_minecraft_registries::Named;

use crate::fixture::DIRT;
use crate::fixture::STONE;
use crate::fixture::app;
use crate::fixture::capture_file;
use crate::fixture::holders;
use crate::fixture::restore;
use crate::fixture::saved;
use crate::fixture::stacks;

/// INV-SAV-001, INV-SAV-002: a load puts each stack of the file in its slot with no filter and no
/// slot limit, and an autosave writes the same file back, with the keys of `equipment` that hold
/// no slot.
#[test]
fn inv_sav_001_002_a_file_loads_into_its_slots_and_an_autosave_writes_it_back() {
    let inventory = vec![
        ItemStackWithSlot {
            slot: 0,
            stack: saved("stone", 5),
        },
        ItemStackWithSlot {
            slot: 35,
            stack: saved("dirt", 99),
        },
    ];
    let equipment = BTreeMap::from([
        ("feet".to_owned(), saved("stone", 3)),
        ("offhand".to_owned(), saved("dirt", 1)),
        ("body".to_owned(), saved("stone", 2)),
    ]);
    let cases = [(4, 4), (8, 8), (9, 0), (-1, 0)];
    for (file_selected, selected) in cases {
        let mut app = app();
        let world = app.world_mut();
        let mut file = PersistedSnapshot::new();
        file.put(&["Inventory"], &inventory).expect("the inventory");
        file.put(&["SelectedItemSlot"], &file_selected)
            .expect("the selected slot");
        file.put(&["equipment"], &equipment).expect("the equipment");
        let player = restore(world, &file);

        assert_eq!(
            stacks(world, player),
            [(0, STONE, 5), (35, DIRT, 99), (36, STONE, 3), (40, DIRT, 1)],
            "the stacks of the file, past the maximum stack size and the armour filter"
        );
        assert_eq!(
            world.get::<Selected>(player),
            Some(&Selected(selected)),
            "the file gives the selected slot {file_selected}"
        );
        assert_eq!(
            world.get::<Children>(player).map(|children| children.len()),
            Some(3),
            "the spawn and the file give the player one set of holders"
        );
        let holders = holders(world, player);
        for holder in [holders.cursor, holders.craft_grid, holders.craft_result] {
            assert!(
                stacks(world, holder).is_empty(),
                "a load fills no other holder"
            );
        }

        let written = capture_file(world, player, CaptureCause::Autosave);
        assert_eq!(
            written
                .get_as::<Vec<ItemStackWithSlot<Named>>>(&["Inventory"])
                .ok()
                .flatten(),
            Some(inventory.clone()),
            "the main slots"
        );
        assert_eq!(
            written
                .get_as::<BTreeMap<String, ItemStackValue<Named>>>(&["equipment"])
                .ok()
                .flatten(),
            Some(equipment.clone()),
            "the armour, the off hand, the other keys"
        );
        assert_eq!(
            written.get_as::<i32>(&["SelectedItemSlot"]).ok().flatten(),
            Some(i32::from(selected)),
            "the selected slot"
        );
    }
}

/// INV-SLT-002: a player with no file gets four empty holders and the selected slot 0.
#[test]
fn inv_slt_002_a_player_without_a_file_gets_four_empty_holders() {
    let mut app = app();
    let world = app.world_mut();
    let player = restore(world, &PersistedSnapshot::new());

    let holders = holders(world, player);
    let sizes = [
        player,
        holders.cursor,
        holders.craft_grid,
        holders.craft_result,
    ]
    .map(|holder| {
        let slots = world.get::<Slots>(holder);
        slots.map(|slots| (slots.len(), slots.occupied().count()))
    });
    assert_eq!(
        sizes,
        [Some((41, 0)), Some((1, 0)), Some((4, 0)), Some((1, 0))],
        "41 slots, the cursor, the craft grid and the craft result, all empty"
    );
    assert_eq!(world.get::<Selected>(player), Some(&Selected(0)));
}
