mod common;

use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_item::{Held, ItemStack, MoveError, Patch, StackRevision, mutate};
use mcrs_minecraft_protocol::item::{Damage, Lore, MaxStackSize, Unbreakable};

use common::{drain, holder, items, spawn, world};

fn revision(world: &bevy_ecs::world::World, stack: bevy_ecs::entity::Entity) -> u32 {
    world.get::<StackRevision>(stack).unwrap().0
}

#[test]
fn set_equal_to_the_prototype_clears_the_patch() {
    let mut world = world();
    let stone = spawn(&mut world, "stone", 1);
    mutate::set(&mut world, stone, MaxStackSize(Bounded(16)), items());
    assert_eq!(
        world.get::<Patch<MaxStackSize>>(stone),
        Some(&Patch(Some(MaxStackSize(Bounded(16)))))
    );
    mutate::set(&mut world, stone, MaxStackSize(Bounded(64)), items());
    assert_eq!(world.get::<Patch<MaxStackSize>>(stone), None);
    assert_eq!(revision(&world, stone), 3);
}

#[test]
fn remove_tombstones_a_prototype_value_and_clears_the_rest() {
    let mut world = world();
    let stone = spawn(&mut world, "stone", 1);
    mutate::remove::<Lore>(&mut world, stone, items());
    assert_eq!(world.get::<Patch<Lore>>(stone), Some(&Patch(None)));
    mutate::set(&mut world, stone, Unbreakable, items());
    assert!(world.get::<Patch<Unbreakable>>(stone).is_some());
    mutate::remove::<Unbreakable>(&mut world, stone, items());
    assert_eq!(world.get::<Patch<Unbreakable>>(stone), None);
}

#[test]
fn set_count_zero_despawns_the_subtree() {
    let mut world = world();
    let shulker = spawn(&mut world, "shulker_box", 1);
    let inner = spawn(&mut world, "stone", 5);
    world
        .entity_mut(shulker)
        .insert(mcrs_minecraft_item::SlotTable::fixed(27));
    mutate::move_stack(&mut world, inner, shulker, 3).unwrap();
    mutate::set_count(&mut world, inner, 2);
    assert_eq!(world.get::<ItemStack>(inner).unwrap().count(), 2);
    mutate::set_count(&mut world, shulker, 0);
    assert!(world.get_entity(shulker).is_err());
    assert!(world.get_entity(inner).is_err());
}

/// A chest holding a shulker holding a pickaxe; returns (chest, shulker, pickaxe).
fn chest_of_shulker(
    world: &mut bevy_ecs::world::World,
    chest_cell: u16,
) -> (bevy_ecs::entity::Entity, bevy_ecs::entity::Entity, bevy_ecs::entity::Entity) {
    let chest = holder(world, 27);
    let shulker = spawn(world, "shulker_box", 1);
    world
        .entity_mut(shulker)
        .insert(mcrs_minecraft_item::SlotTable::fixed(27));
    let pickaxe = spawn(world, "diamond_pickaxe", 1);
    mutate::move_stack(world, shulker, chest, chest_cell).unwrap();
    mutate::move_stack(world, pickaxe, shulker, 4).unwrap();
    (chest, shulker, pickaxe)
}

#[test]
fn a_change_deep_in_a_chest_bumps_the_chain_and_queues_the_cell() {
    let mut world = world();
    let (chest, shulker, pickaxe) = chest_of_shulker(&mut world, 7);
    drain(&mut world);
    let (before_pickaxe, before_shulker) = (revision(&world, pickaxe), revision(&world, shulker));
    mutate::set(&mut world, pickaxe, Damage(Bounded(1)), items());
    assert_eq!(revision(&world, pickaxe), before_pickaxe + 1);
    assert_eq!(revision(&world, shulker), before_shulker + 1);
    let dirty = drain(&mut world);
    assert_eq!(dirty.cells, [(chest, 7)]);
    assert!(dirty.roots.is_empty());
}

#[test]
fn moving_between_two_shulkers_bumps_both_parents() {
    let mut world = world();
    let (chest_a, shulker_a, pickaxe) = chest_of_shulker(&mut world, 1);
    let (chest_b, shulker_b, _) = chest_of_shulker(&mut world, 2);
    drain(&mut world);
    let (before_a, before_b) = (revision(&world, shulker_a), revision(&world, shulker_b));
    mutate::move_stack(&mut world, pickaxe, shulker_b, 9).unwrap();
    assert_eq!(world.get::<Held>(pickaxe), Some(&Held { holder: shulker_b, index: 9 }));
    assert_eq!(revision(&world, shulker_a), before_a + 1);
    assert_eq!(revision(&world, shulker_b), before_b + 1);
    let cells = drain(&mut world).cells;
    assert_eq!(cells.len(), 2);
    assert!(cells.contains(&(chest_a, 1)) && cells.contains(&(chest_b, 2)), "{cells:?}");
    assert_eq!(world.get::<mcrs_minecraft_item::SlotTable>(shulker_a).unwrap().get(4), None);
    assert_eq!(world.get::<mcrs_minecraft_item::SlotTable>(shulker_b).unwrap().get(9), Some(pickaxe));
}

#[test]
fn move_errors_are_returned_not_panicked() {
    let mut world = world();
    let chest = holder(&mut world, 3);
    let a = spawn(&mut world, "stone", 1);
    let b = spawn(&mut world, "stone", 1);
    mutate::move_stack(&mut world, a, chest, 0).unwrap();
    assert!(matches!(
        mutate::move_stack(&mut world, b, chest, 0),
        Err(MoveError::Occupied { holder, index: 0, by }) if holder == chest && by == a
    ));
    assert!(matches!(
        mutate::move_stack(&mut world, b, chest, 3),
        Err(MoveError::OutOfRange { index: 3, .. })
    ));
    assert!(mutate::move_stack(&mut world, a, chest, 0).is_ok(), "re-placing a stack in its own cell is a no-op");
    let (_, shulker, pickaxe) = chest_of_shulker(&mut world, 1);
    world
        .entity_mut(pickaxe)
        .insert(mcrs_minecraft_item::SlotTable::fixed(1));
    assert!(matches!(
        mutate::move_stack(&mut world, shulker, pickaxe, 0),
        Err(MoveError::Cycle(stack)) if stack == shulker
    ));
    let gone = world.spawn_empty().id();
    world.despawn(gone);
    assert!(matches!(
        mutate::move_stack(&mut world, b, gone, 0),
        Err(MoveError::HolderMissing(holder)) if holder == gone
    ));
    let no_table = world.spawn_empty().id();
    assert!(matches!(
        mutate::move_stack(&mut world, b, no_table, 0),
        Err(MoveError::HolderMissing(_))
    ));
}

#[test]
fn split_and_merge_move_counts() {
    let mut world = world();
    let chest = holder(&mut world, 2);
    let stone = spawn(&mut world, "stone", 40);
    mutate::set(&mut world, stone, Lore::default(), items());
    mutate::move_stack(&mut world, stone, chest, 0).unwrap();
    let half = mutate::split(&mut world, stone, 15, items()).unwrap();
    assert_eq!(world.get::<ItemStack>(half).unwrap().count(), 15);
    assert_eq!(world.get::<ItemStack>(stone).unwrap().count(), 25);
    assert!(world.get::<Held>(half).is_none());
    assert!(mcrs_minecraft_item::same_item_same_components(&world, stone, half, items()));
    assert_eq!(mutate::merge_into(&mut world, half, stone, 30), 5);
    assert_eq!(world.get::<ItemStack>(stone).unwrap().count(), 30);
    assert_eq!(world.get::<ItemStack>(half).unwrap().count(), 10);
    assert_eq!(mutate::merge_into(&mut world, half, stone, 64), 10);
    assert!(world.get_entity(half).is_err());
    let rest = mutate::split(&mut world, stone, 99, items()).unwrap();
    assert!(world.get_entity(stone).is_err(), "splitting everything despawns the source");
    assert_eq!(world.get::<ItemStack>(rest).unwrap().count(), 40);
}

#[test]
fn only_mutate_writes_stack_truth() {
    let src = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
    let mut offenders = Vec::new();
    for entry in walk(std::path::Path::new(src)) {
        let name = entry.strip_prefix(src).unwrap().display().to_string();
        let text = std::fs::read_to_string(&entry).unwrap();
        let allowed = |writer: &str| name.ends_with(writer) || name.ends_with("value.rs");
        if !allowed("mutate.rs") && text.contains(".count = ") {
            offenders.push(format!("{name}: assigns a count"));
        }
        if !allowed("mutate.rs") && !name.ends_with("patch.rs") && text.contains("Patch(") {
            offenders.push(format!("{name}: builds a Patch"));
        }
        if !name.ends_with("mutate.rs") && !name.ends_with("held.rs") && text.contains("Held {") {
            offenders.push(format!("{name}: builds a Held"));
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
fn moving_a_non_stack_or_dropped_stack_is_an_error() {
    let mut world = world();
    let chest = holder(&mut world, 2);
    let gone = spawn(&mut world, "stone", 1);
    world.despawn(gone);
    assert!(matches!(mutate::move_stack(&mut world, gone, chest, 0), Err(MoveError::NotAStack(e)) if e == gone));
    let bare = world.spawn_empty().id();
    assert!(matches!(mutate::move_stack(&mut world, bare, chest, 0), Err(MoveError::NotAStack(e)) if e == bare));
    let dropped = spawn(&mut world, "stone", 1);
    mcrs_minecraft_item::dropped::spawn_dropped(&mut world, dropped, 0, None);
    assert!(matches!(mutate::move_stack(&mut world, dropped, chest, 0), Err(MoveError::Dropped(e)) if e == dropped));
    assert_eq!(world.get::<mcrs_minecraft_item::SlotTable>(chest).unwrap().iter().count(), 0);
}

#[test]
fn spawning_and_splitting_queue_no_roots() {
    let mut world = world();
    let chest = holder(&mut world, 1);
    let stone = spawn(&mut world, "stone", 10);
    assert!(mutate::split(&mut world, stone, 0, items()).is_none());
    assert_eq!(world.get::<ItemStack>(stone).unwrap().count(), 10);
    assert_eq!(mutate::merge_into(&mut world, stone, stone, 64), 0);
    assert_eq!(world.get::<ItemStack>(stone).unwrap().count(), 10);
    mutate::move_stack(&mut world, stone, chest, 0).unwrap();
    let half = mutate::split(&mut world, stone, 5, items()).unwrap();
    let dirty = drain(&mut world);
    assert!(dirty.roots.is_empty(), "{dirty:?}");
    assert_eq!(dirty.cells, [(chest, 0), (chest, 0)]);
    assert_eq!(world.get::<ItemStack>(half).unwrap().count(), 5);
}
