use mcrs_minecraft_engine::persisted_registry::CaptureCause;
use mcrs_minecraft_engine::persisted_registry::PersistedSnapshot;
use mcrs_minecraft_protocol_game::item::ItemStackWithSlot;
use mcrs_minecraft_registries::Named;

use crate::fixture::DIRT;
use crate::fixture::STONE;
use crate::fixture::app;
use crate::fixture::capture;
use crate::fixture::capture_file;
use crate::fixture::dropped;
use crate::fixture::holders;
use crate::fixture::put;
use crate::fixture::restore;
use crate::fixture::saved;
use crate::fixture::stacks;

/// INV-CLS-003: a leave and a dimension change put the cursor stack, then the craft grid, into the
/// player inventory by the insert rule before the write. Each rest that finds no room becomes one
/// item entity with the pickup delay 40 (IEN-SPN-001).
#[test]
fn inv_cls_003_a_leave_folds_the_cursor_and_the_craft_grid_into_the_inventory() {
    for reason in [CaptureCause::Leave, CaptureCause::DimensionChange] {
        let mut app = app();
        let world = app.world_mut();
        let mut inventory = vec![ItemStackWithSlot {
            slot: 0,
            stack: saved("stone", 60),
        }];
        inventory.extend((1..36).map(|slot| ItemStackWithSlot {
            slot,
            stack: saved("dirt", 64),
        }));
        let mut file = PersistedSnapshot::new();
        file.put(&["Inventory"], &inventory).expect("the inventory");
        let player = restore(world, &file);
        let holders = holders(world, player);
        put(world, holders.cursor, 0, STONE, 10);
        put(world, holders.craft_grid, 1, STONE, 1);
        put(world, holders.craft_grid, 2, DIRT, 5);

        let written = capture(world, player, reason);

        let inventory = written
            .get_as::<Vec<ItemStackWithSlot<Named>>>(&["Inventory"])
            .expect("the inventory decodes")
            .expect("the inventory");
        assert_eq!(
            inventory.first(),
            Some(&ItemStackWithSlot {
                slot: 0,
                stack: saved("stone", 64),
            }),
            "{reason:?}: the cursor fills the alike stack of the slot 0"
        );
        assert!(
            written.get("mcrs:cursor").is_none() && written.get("mcrs:craft_grid").is_none(),
            "{reason:?}: the close leaves nothing to carry"
        );
        assert_eq!(
            dropped(world),
            [(STONE, 1, 40), (STONE, 6, 40), (DIRT, 5, 40)],
            "{reason:?}: one item entity for the rest of each source stack"
        );
        assert_eq!(
            stacks(world, player).first(),
            Some(&(0, STONE, 64)),
            "{reason:?}: the live inventory holds what the file holds"
        );
        for holder in [holders.cursor, holders.craft_grid] {
            assert!(
                stacks(world, holder).is_empty(),
                "{reason:?}: the close empties it"
            );
        }
    }
}

/// INV-SLT-002: a cell crossing carries the cursor and the craft grid to the restore in the other
/// world, and leaves the player inventory of the file as it is. A later leave ends the carry.
#[test]
fn inv_slt_002_a_cell_crossing_carries_the_cursor_and_the_craft_grid() {
    let mut from = app();
    let from = from.world_mut();
    let player = restore(from, &PersistedSnapshot::new());
    let left = holders(from, player);
    put(from, left.cursor, 0, STONE, 7);
    put(from, left.craft_grid, 3, DIRT, 2);

    let carried = capture(from, player, CaptureCause::CellCrossing);

    let file = capture_file(from, player, CaptureCause::CellCrossing);
    assert_eq!(
        file.get_as::<Vec<ItemStackWithSlot<Named>>>(&["Inventory"])
            .ok()
            .flatten(),
        Some(Vec::new()),
        "the cursor stays out of the file"
    );
    assert!(
        file.get("mcrs:cursor").is_none() && file.get("mcrs:craft_grid").is_none(),
        "the file holds no carried value"
    );
    assert!(dropped(from).is_empty(), "a crossing drops nothing");

    let mut to = app();
    let to = to.world_mut();
    let arrived = restore(to, &carried);
    let entered = holders(to, arrived);
    assert_eq!(stacks(to, entered.cursor), [(0, STONE, 7)]);
    assert_eq!(stacks(to, entered.craft_grid), [(3, DIRT, 2)]);

    let mut written = capture(to, arrived, CaptureCause::Leave);
    assert_eq!(
        stacks(to, arrived),
        [(0, STONE, 7), (1, DIRT, 2)],
        "the leave folds the carried stacks into the inventory"
    );
    written.remove("Inventory");
    let joined = restore(to, &written);
    let joined = holders(to, joined);
    assert!(
        stacks(to, joined.cursor).is_empty() && stacks(to, joined.craft_grid).is_empty(),
        "the carry ends with the leave"
    );
}
