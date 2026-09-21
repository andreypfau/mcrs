mod common;

use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_inventory::{Op, Slot, TransactionError};
use mcrs_minecraft_item::{
    DroppedItem, Held, ItemStack, SlotTable, same_item_same_components, stack_to_value,
};
use mcrs_minecraft_protocol::item::{
    ComponentPatch, Damage, ItemComponentKind, Lore, MaxStackSize, Unbreakable,
};

use common::{apply, holder, items, place, revision, set_count, spawn, world};

fn insert<K: mcrs_minecraft_protocol::item::ItemDataComponent>(
    world: &mut World,
    stack: Entity,
    value: K,
) {
    apply(
        world,
        vec![Op::Insert {
            stack,
            component: value.into_value(),
        }],
    )
    .unwrap();
}

fn remove(world: &mut World, stack: Entity, kind: ItemComponentKind) {
    apply(world, vec![Op::Remove { stack, kind }]).unwrap();
}

fn count(world: &World, stack: Entity) -> u8 {
    world.get::<ItemStack>(stack).unwrap().count
}

#[test]
fn a_stack_carries_its_effective_components() {
    let mut world = world();
    let stone = spawn(&mut world, "stone", 1);
    assert_eq!(
        world.get::<MaxStackSize>(stone),
        Some(&MaxStackSize(Bounded(64)))
    );
    assert_eq!(world.get::<Lore>(stone), Some(&Lore::default()));
    assert_eq!(world.get::<Damage>(stone), None);
    assert_eq!(
        stack_to_value(&world, stone, items()).components,
        ComponentPatch::EMPTY
    );
}

#[test]
fn insert_equal_to_the_prototype_leaves_no_patch() {
    let mut world = world();
    let stone = spawn(&mut world, "stone", 1);
    insert(&mut world, stone, MaxStackSize(Bounded(16)));
    assert_eq!(
        world.get::<MaxStackSize>(stone),
        Some(&MaxStackSize(Bounded(16)))
    );
    let patch = stack_to_value(&world, stone, items()).components;
    assert_eq!(patch.added, [MaxStackSize(Bounded(16)).into()]);
    assert!(patch.removed.is_empty());
    insert(&mut world, stone, MaxStackSize(Bounded(64)));
    assert_eq!(
        stack_to_value(&world, stone, items()).components,
        ComponentPatch::EMPTY
    );
    assert_eq!(revision(&world, stone), 3);
}

#[test]
fn remove_tombstones_a_prototype_value_and_clears_the_rest() {
    let mut world = world();
    let stone = spawn(&mut world, "stone", 1);
    remove(&mut world, stone, ItemComponentKind::Lore);
    assert_eq!(world.get::<Lore>(stone), None);
    let patch = stack_to_value(&world, stone, items()).components;
    assert!(patch.added.is_empty());
    assert_eq!(patch.removed, [ItemComponentKind::Lore]);
    insert(&mut world, stone, Unbreakable);
    assert_eq!(world.get::<Unbreakable>(stone), Some(&Unbreakable));
    assert_eq!(
        stack_to_value(&world, stone, items()).components.added,
        [Unbreakable.into()]
    );
    remove(&mut world, stone, ItemComponentKind::Unbreakable);
    assert_eq!(world.get::<Unbreakable>(stone), None);
    assert_eq!(
        stack_to_value(&world, stone, items()).components.removed,
        [ItemComponentKind::Lore]
    );
}

#[test]
fn a_child_kind_is_never_a_component() {
    let mut world = world();
    let shulker = spawn(&mut world, "shulker_box", 1);
    let refused = apply(
        &mut world,
        vec![Op::Remove {
            stack: shulker,
            kind: ItemComponentKind::Container,
        }],
    );
    assert!(matches!(
        refused,
        Err(TransactionError::ChildKind(ItemComponentKind::Container))
    ));
}

#[test]
fn set_count_zero_despawns_the_subtree() {
    let mut world = world();
    let shulker = spawn(&mut world, "shulker_box", 1);
    let inner = spawn(&mut world, "stone", 5);
    place(&mut world, inner, shulker, 3).unwrap();
    set_count(&mut world, inner, 2);
    assert_eq!(count(&world, inner), 2);
    set_count(&mut world, shulker, 0);
    assert!(world.get_entity(shulker).is_err());
    assert!(world.get_entity(inner).is_err());
}

/// A chest holding a shulker holding a pickaxe; returns (chest, shulker, pickaxe).
fn chest_of_shulker(world: &mut World, chest_cell: u16) -> (Entity, Entity, Entity) {
    let chest = holder(world, 27);
    let shulker = spawn(world, "shulker_box", 1);
    let pickaxe = spawn(world, "diamond_pickaxe", 1);
    place(world, shulker, chest, chest_cell).unwrap();
    place(world, pickaxe, shulker, 4).unwrap();
    (chest, shulker, pickaxe)
}

#[test]
fn a_change_deep_in_a_chest_bumps_the_chain() {
    let mut world = world();
    let (_chest, shulker, pickaxe) = chest_of_shulker(&mut world, 7);
    let (before_pickaxe, before_shulker) = (revision(&world, pickaxe), revision(&world, shulker));
    insert(&mut world, pickaxe, Damage(Bounded(1)));
    assert_eq!(revision(&world, pickaxe), before_pickaxe + 1);
    assert_eq!(revision(&world, shulker), before_shulker + 1);
}

#[test]
fn moving_between_two_shulkers_bumps_both_parents() {
    let mut world = world();
    let (_chest_a, shulker_a, pickaxe) = chest_of_shulker(&mut world, 1);
    let (_chest_b, shulker_b, _) = chest_of_shulker(&mut world, 2);
    let (before_a, before_b) = (revision(&world, shulker_a), revision(&world, shulker_b));
    apply(
        &mut world,
        vec![Op::Transfer {
            from: Slot::new(shulker_a, 4),
            to: Slot::new(shulker_b, 9),
            count: 1,
        }],
    )
    .unwrap();
    assert_eq!(
        world.get::<Held>(pickaxe),
        Some(&Held {
            holder: shulker_b,
            index: 9
        })
    );
    assert_eq!(revision(&world, shulker_a), before_a + 1);
    assert_eq!(revision(&world, shulker_b), before_b + 1);
    assert_eq!(world.get::<SlotTable>(shulker_a).unwrap().get(4), None);
    assert_eq!(
        world.get::<SlotTable>(shulker_b).unwrap().get(9),
        Some(pickaxe)
    );
}

#[test]
fn move_errors_are_returned_not_panicked() {
    let mut world = world();
    let chest = holder(&mut world, 3);
    let a = spawn(&mut world, "stone", 1);
    let b = spawn(&mut world, "stone", 1);
    place(&mut world, a, chest, 0).unwrap();
    assert!(matches!(
        place(&mut world, b, chest, 0),
        Err(TransactionError::Occupied { holder, index: 0, by }) if holder == chest && by == a
    ));
    assert!(matches!(
        place(&mut world, b, chest, 3),
        Err(TransactionError::OutOfRange { index: 3, .. })
    ));
    assert!(
        place(&mut world, a, chest, 0).is_ok(),
        "re-placing a stack in its own cell is a no-op"
    );
    let (_, shulker, pickaxe) = chest_of_shulker(&mut world, 1);
    world.entity_mut(pickaxe).insert(SlotTable::fixed(1));
    assert!(matches!(
        place(&mut world, shulker, pickaxe, 0),
        Err(TransactionError::Cycle(stack)) if stack == shulker
    ));
    let gone = world.spawn_empty().id();
    world.despawn(gone);
    assert!(matches!(
        place(&mut world, b, gone, 0),
        Err(TransactionError::HolderMissing(holder)) if holder == gone
    ));
    let no_table = world.spawn_empty().id();
    assert!(matches!(
        place(&mut world, b, no_table, 0),
        Err(TransactionError::HolderMissing(_))
    ));
}

#[test]
fn a_failed_op_stops_the_rest_of_the_transaction() {
    let mut world = world();
    let chest = holder(&mut world, 3);
    let a = spawn(&mut world, "stone", 1);
    let b = spawn(&mut world, "dirt", 1);
    place(&mut world, a, chest, 0).unwrap();
    let refused = apply(
        &mut world,
        vec![
            Op::Place {
                stack: b,
                to: Slot::new(chest, 0),
            },
            Op::Place {
                stack: b,
                to: Slot::new(chest, 1),
            },
        ],
    );
    assert!(matches!(refused, Err(TransactionError::Occupied { .. })));
    assert_eq!(world.get::<Held>(b), None);
}

#[test]
fn transfer_splits_merges_and_moves_counts() {
    let mut world = world();
    let chest = holder(&mut world, 3);
    let stone = spawn(&mut world, "stone", 40);
    insert(&mut world, stone, Lore::default());
    place(&mut world, stone, chest, 0).unwrap();
    let (from, to) = (Slot::new(chest, 0), Slot::new(chest, 1));
    apply(
        &mut world,
        vec![Op::Transfer {
            from,
            to,
            count: 15,
        }],
    )
    .unwrap();
    let half = world.get::<SlotTable>(chest).unwrap().get(1).unwrap();
    assert_eq!(count(&world, half), 15);
    assert_eq!(count(&world, stone), 25);
    assert!(same_item_same_components(&world, stone, half, items()));
    apply(
        &mut world,
        vec![Op::Transfer {
            from: to,
            to: from,
            count: 5,
        }],
    )
    .unwrap();
    assert_eq!(count(&world, stone), 30);
    assert_eq!(count(&world, half), 10);
    apply(
        &mut world,
        vec![Op::Transfer {
            from: to,
            to: from,
            count: 64,
        }],
    )
    .unwrap();
    assert!(world.get_entity(half).is_err());
    assert_eq!(count(&world, stone), 40);
    apply(
        &mut world,
        vec![Op::Transfer {
            from,
            to,
            count: 99,
        }],
    )
    .unwrap();
    assert_eq!(
        world.get::<Held>(stone),
        Some(&Held {
            holder: chest,
            index: 1
        }),
        "moving everything keeps the entity"
    );
    assert_eq!(count(&world, stone), 40);
    let dirt = spawn(&mut world, "dirt", 1);
    place(&mut world, dirt, chest, 2).unwrap();
    assert!(matches!(
        apply(&mut world, vec![Op::Transfer { from: to, to: Slot::new(chest, 2), count: 1 }]),
        Err(TransactionError::Different(slot)) if slot == Slot::new(chest, 2)
    ));
}

#[test]
fn swap_exchanges_two_cells_either_of_which_may_be_empty() {
    let mut world = world();
    let chest = holder(&mut world, 3);
    let a = spawn(&mut world, "stone", 1);
    let b = spawn(&mut world, "dirt", 1);
    place(&mut world, a, chest, 0).unwrap();
    place(&mut world, b, chest, 1).unwrap();
    let swap = |a, b| Op::Swap {
        a: Slot::new(chest, a),
        b: Slot::new(chest, b),
    };
    apply(&mut world, vec![swap(0, 1)]).unwrap();
    assert_eq!(world.get::<Held>(a).unwrap().index, 1);
    assert_eq!(world.get::<Held>(b).unwrap().index, 0);
    apply(&mut world, vec![swap(1, 2)]).unwrap();
    assert_eq!(world.get::<Held>(a).unwrap().index, 2);
    assert_eq!(world.get::<SlotTable>(chest).unwrap().get(1), None);
}

#[test]
fn drop_takes_a_stack_out_of_its_cell_and_pickup_puts_it_back() {
    let mut world = world();
    let player = holder(&mut world, 47);
    let stone = spawn(&mut world, "stone", 20);
    place(&mut world, stone, player, 36).unwrap();
    apply(
        &mut world,
        vec![Op::Drop {
            from: Slot::new(player, 36),
            count: 8,
            thrower: player,
        }],
    )
    .unwrap();
    assert_eq!(count(&world, stone), 12);
    let thrown = world
        .query_filtered::<Entity, bevy_ecs::prelude::With<DroppedItem>>()
        .single(&world)
        .unwrap();
    assert_eq!(count(&world, thrown), 8);
    assert_eq!(world.get::<DroppedItem>(thrown).unwrap().pickup_delay, 40);
    assert_eq!(world.get::<Held>(thrown), None);
    apply(
        &mut world,
        vec![Op::Pickup {
            item: thrown,
            to: Slot::new(player, 36),
            count: 8,
        }],
    )
    .unwrap();
    assert_eq!(count(&world, stone), 20);
    assert!(world.get_entity(thrown).is_err());
}

#[test]
fn only_the_transaction_writes_stack_truth() {
    let src = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
    let mut offenders = Vec::new();
    for entry in walk(std::path::Path::new(src)) {
        let name = entry.strip_prefix(src).unwrap().display().to_string();
        let text = std::fs::read_to_string(&entry).unwrap();
        let writer = name.ends_with("transaction.rs") || name.ends_with("value.rs");
        if !writer && text.contains(".count = ") {
            offenders.push(format!("{name}: assigns a count"));
        }
        if !writer && text.contains("Held {") {
            offenders.push(format!("{name}: builds a Held"));
        }
        if !writer && text.contains("&mut World") {
            offenders.push(format!("{name}: takes the world mutably"));
        }
    }
    assert!(offenders.is_empty(), "{offenders:#?}");
}

fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(walk(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            files.push(path);
        }
    }
    files
}

#[test]
fn placing_a_non_stack_or_dropped_stack_is_an_error() {
    let mut world = world();
    let chest = holder(&mut world, 2);
    let gone = spawn(&mut world, "stone", 1);
    world.despawn(gone);
    assert!(
        matches!(place(&mut world, gone, chest, 0), Err(TransactionError::NotAStack(e)) if e == gone)
    );
    let bare = world.spawn_empty().id();
    assert!(
        matches!(place(&mut world, bare, chest, 0), Err(TransactionError::NotAStack(e)) if e == bare)
    );
    let player = holder(&mut world, 47);
    let dropped = spawn(&mut world, "stone", 1);
    place(&mut world, dropped, player, 0).unwrap();
    apply(
        &mut world,
        vec![Op::Drop {
            from: Slot::new(player, 0),
            count: 1,
            thrower: player,
        }],
    )
    .unwrap();
    assert!(
        matches!(place(&mut world, dropped, chest, 0), Err(TransactionError::Dropped(e)) if e == dropped)
    );
    assert_eq!(world.get::<SlotTable>(chest).unwrap().iter().count(), 0);
}
