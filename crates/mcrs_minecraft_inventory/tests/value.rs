mod common;

use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_inventory::value::spawn_stack;
use mcrs_minecraft_inventory::{Op, TransactionError};
use mcrs_minecraft_item::held::SlotTable;
use mcrs_minecraft_item::{
    Held, ItemStack, StackError, same_item_same_components, stack_to_slot, stack_to_value,
};
use mcrs_minecraft_protocol::item::{ComponentPatch, ItemComponentKind, ItemStackValue};

use common::{apply, items, place, set_count, world};

fn apply_value(world: &mut World, stack: Entity, value: &ItemStackValue) -> Result<(), StackError> {
    match apply(
        world,
        vec![Op::Apply {
            stack,
            value: value.clone(),
        }],
    ) {
        Ok(()) => Ok(()),
        Err(TransactionError::Stack(error)) => Err(error),
        Err(other) => panic!("{other}"),
    }
}

fn remove(world: &mut World, stack: Entity, kind: ItemComponentKind) {
    apply(world, vec![Op::Remove { stack, kind }]).unwrap();
}

fn parse(json: &str) -> ItemStackValue {
    serde_json::from_str(json).unwrap()
}

const CHESTPLATE: &str = r#"{"id": "minecraft:leather_chestplate", "count": 1, "components": {
    "minecraft:dyed_color": 16711680,
    "minecraft:trim": {"material": "minecraft:gold", "pattern": "minecraft:sentry"}}}"#;
const BOOK: &str = r#"{"id": "minecraft:enchanted_book", "count": 1, "components": {
    "minecraft:stored_enchantments": {"minecraft:sharpness": 3, "minecraft:unbreaking": 1}}}"#;
const POTION: &str = r#"{"id": "minecraft:potion", "count": 3, "components": {
    "minecraft:potion_contents": {"custom_effects": [{"id": "minecraft:speed", "duration": 600, "amplifier": 1}]}}}"#;
const SHULKER: &str = r#"{"id": "minecraft:shulker_box", "count": 1, "components": {
    "minecraft:container": [
        {"slot": 0, "item": {"id": "minecraft:stone", "count": 64}},
        {"slot": 5, "item": {"id": "minecraft:diamond_pickaxe", "components": {"minecraft:damage": 12}}},
        {"slot": 26, "item": {"id": "minecraft:bundle", "components": {"minecraft:bundle_contents": [
            {"id": "minecraft:apple", "count": 7}, "minecraft:stick"]}}}]}}"#;
const BUNDLE: &str = r#"{"id": "minecraft:bundle", "components": {"minecraft:bundle_contents": [
    {"id": "minecraft:apple", "count": 7}, {"id": "minecraft:stick", "count": 2}]}}"#;
const NO_LORE: &str =
    r#"{"id": "minecraft:stone", "count": 9, "components": {"!minecraft:lore": {}}}"#;

#[test]
fn spawn_then_read_is_the_identity() {
    let mut world = world();
    for json in [CHESTPLATE, BOOK, POTION, SHULKER, BUNDLE, NO_LORE] {
        let value = parse(json);
        let stack = spawn_stack(&mut world, &value, items()).unwrap();
        assert_eq!(stack_to_value(&world, stack, items()), value, "{json}");
        let slot = stack_to_slot(&world, stack, items());
        assert_eq!(slot.count, value.count.0);
        assert_eq!(slot.components, value.components);
        assert_eq!(
            items().get(slot.id).unwrap().identifier,
            *value.item.location()
        );
    }
}

#[test]
fn prototype_values_and_absent_removals_normalise_away() {
    let mut world = world();
    let value = parse(
        r#"{"id": "minecraft:stone", "components": {"minecraft:max_stack_size": 64, "!minecraft:food": {}}}"#,
    );
    let stack = spawn_stack(&mut world, &value, items()).unwrap();
    assert_eq!(
        stack_to_value(&world, stack, items()).components,
        ComponentPatch::EMPTY
    );
    let shulker = spawn_stack(
        &mut world,
        &parse(r#"{"id": "minecraft:shulker_box", "components": {"minecraft:container": []}}"#),
        items(),
    )
    .unwrap();
    assert_eq!(
        stack_to_value(&world, shulker, items()).components,
        ComponentPatch::EMPTY
    );
}

#[test]
fn nested_stacks_are_child_entities() {
    let mut world = world();
    let shulker = spawn_stack(&mut world, &parse(SHULKER), items()).unwrap();
    let table = world.get::<SlotTable>(shulker).unwrap();
    assert_eq!(table.len(), 27);
    let cells: Vec<u16> = table.iter().map(|(index, _)| index).collect();
    assert_eq!(cells, [0, 5, 26]);
    let bundle = table.get(26).unwrap();
    assert_eq!(
        world.get::<Held>(bundle),
        Some(&Held {
            holder: shulker,
            index: 26
        })
    );
    let inner = world.get::<SlotTable>(bundle).unwrap();
    assert!(inner.is_growable());
    assert_eq!(inner.len(), 2);
    assert_eq!(
        world
            .get::<ItemStack>(inner.get(0).unwrap())
            .unwrap()
            .count(),
        7
    );
    assert_eq!(world.query::<&ItemStack>().iter(&world).count(), 6);
}

#[test]
fn apply_value_reconciles_an_existing_subtree() {
    let mut world = world();
    let shulker = spawn_stack(&mut world, &parse(SHULKER), items()).unwrap();
    let before: Vec<Option<bevy_ecs::entity::Entity>> = (0..27)
        .map(|i| world.get::<SlotTable>(shulker).unwrap().get(i))
        .collect();
    let next = parse(
        r#"{"id": "minecraft:shulker_box", "components": {"minecraft:container": [
            {"slot": 0, "item": {"id": "minecraft:stone", "count": 3}},
            {"slot": 5, "item": {"id": "minecraft:iron_pickaxe"}}]}}"#,
    );
    apply_value(&mut world, shulker, &next).unwrap();
    let after: Vec<Option<bevy_ecs::entity::Entity>> = (0..27)
        .map(|i| world.get::<SlotTable>(shulker).unwrap().get(i))
        .collect();
    assert_eq!(after[0], before[0], "the kept child keeps its entity");
    assert_eq!(
        world.get::<ItemStack>(after[0].unwrap()).unwrap().count(),
        3
    );
    assert_ne!(after[5], before[5], "a different item replaces the child");
    assert!(world.get_entity(before[5].unwrap()).is_err());
    assert!(
        world.get_entity(before[26].unwrap()).is_err(),
        "the removed bundle and its children despawn"
    );
    assert_eq!(after[26], None);
    assert_eq!(stack_to_value(&world, shulker, items()), next);
    assert_eq!(world.query::<&ItemStack>().iter(&world).count(), 3);
}

#[test]
fn same_item_same_components_compares_subtrees() {
    let mut world = world();
    let a = spawn_stack(&mut world, &parse(SHULKER), items()).unwrap();
    let b = spawn_stack(&mut world, &parse(SHULKER), items()).unwrap();
    assert!(same_item_same_components(&world, a, b, items()));
    let stone = world.get::<SlotTable>(b).unwrap().get(0).unwrap();
    set_count(&mut world, stone, 63);
    assert!(!same_item_same_components(&world, a, b, items()));
    let other = spawn_stack(&mut world, &parse(BUNDLE), items()).unwrap();
    assert!(!same_item_same_components(&world, a, other, items()));
    let plain = spawn_stack(&mut world, &parse(NO_LORE), items()).unwrap();
    let with_lore =
        spawn_stack(&mut world, &parse(r#"{"id": "minecraft:stone"}"#), items()).unwrap();
    assert!(!same_item_same_components(
        &world,
        plain,
        with_lore,
        items()
    ));
    remove(&mut world, with_lore, ItemComponentKind::Lore);
    assert!(same_item_same_components(&world, plain, with_lore, items()));
}

#[test]
fn a_child_kind_on_an_item_without_one_is_refused() {
    let mut world = world();
    let value = parse(
        r#"{"id": "minecraft:stone", "components": {"minecraft:bundle_contents": ["minecraft:stick"]}}"#,
    );
    let error = spawn_stack(&mut world, &value, items()).unwrap_err();
    assert!(matches!(
        error,
        mcrs_minecraft_item::StackError::UnsupportedChildKind {
            kind: ItemComponentKind::BundleContents,
            ..
        }
    ));
}

#[test]
fn a_refused_value_leaves_the_world_untouched() {
    let mut world = world();
    let chest = world.spawn(SlotTable::fixed(1)).id();
    let shulker = spawn_stack(&mut world, &parse(SHULKER), items()).unwrap();
    place(&mut world, shulker, chest, 0).unwrap();
    let before = stack_to_value(&world, shulker, items());
    let stacks =
        |world: &mut bevy_ecs::world::World| world.query::<&ItemStack>().iter(world).count();
    assert_eq!(stacks(&mut world), 6);
    for (json, expected) in [
        (
            r#"{"id": "minecraft:shulker_box", "components": {"minecraft:container": [
                {"slot": 0, "item": {"id": "minecraft:stone"}}, {"slot": 40, "item": {"id": "minecraft:apple"}}]}}"#,
            "ContainerOverflow",
        ),
        (
            r#"{"id": "minecraft:shulker_box", "components": {"minecraft:container": [
                {"slot": 0, "item": {"id": "minecraft:stone"}}, {"slot": 1, "item": {"id": "minecraft:not_an_item"}}]}}"#,
            "UnknownItem",
        ),
        (
            r#"{"id": "minecraft:shulker_box", "components": {"minecraft:container": [
                {"slot": 0, "item": {"id": "minecraft:stone", "components": {"minecraft:bundle_contents": ["minecraft:stick"]}}}]}}"#,
            "UnsupportedChildKind",
        ),
    ] {
        let value = parse(json);
        let error = spawn_stack(&mut world, &value, items()).unwrap_err();
        assert!(format!("{error:?}").starts_with(expected), "{error:?}");
        let error = apply_value(&mut world, shulker, &value).unwrap_err();
        assert!(format!("{error:?}").starts_with(expected), "{error:?}");
        assert_eq!(stacks(&mut world), 6, "{json}");
        assert_eq!(stack_to_value(&world, shulker, items()), before, "{json}");
    }
    assert_eq!(world.query::<&Held>().iter(&world).count(), 6);
}

#[test]
fn child_kind_tombstones_and_empty_foreign_kinds_survive() {
    let mut world = world();
    for json in [
        r#"{"id": "minecraft:shulker_box", "count": 1, "components": {"!minecraft:container": {}}}"#,
        r#"{"id": "minecraft:stone", "count": 1, "components": {"minecraft:bundle_contents": []}}"#,
    ] {
        let value = parse(json);
        let stack = spawn_stack(&mut world, &value, items()).unwrap();
        assert_eq!(stack_to_value(&world, stack, items()), value, "{json}");
    }
    let shulker = spawn_stack(&mut world, &parse(SHULKER), items()).unwrap();
    let emptied = parse(
        r#"{"id": "minecraft:shulker_box", "count": 1, "components": {"!minecraft:container": {}}}"#,
    );
    apply_value(&mut world, shulker, &emptied).unwrap();
    assert_eq!(stack_to_value(&world, shulker, items()), emptied);
    assert_eq!(world.query::<&ItemStack>().iter(&world).count(), 3);
    apply_value(&mut world, shulker, &parse(SHULKER)).unwrap();
    assert_eq!(stack_to_value(&world, shulker, items()), parse(SHULKER));
}

#[test]
fn a_tombstoned_child_kind_has_no_cells() {
    let mut world = world();
    let emptied =
        parse(r#"{"id": "minecraft:shulker_box", "components": {"!minecraft:container": {}}}"#);
    let shulker = spawn_stack(&mut world, &emptied, items()).unwrap();
    let stone = spawn_stack(&mut world, &parse(r#"{"id": "minecraft:stone"}"#), items()).unwrap();
    assert!(world.get::<SlotTable>(shulker).is_none());
    assert!(matches!(
        place(&mut world, stone, shulker, 0),
        Err(TransactionError::HolderMissing(holder)) if holder == shulker
    ));
    let full = spawn_stack(&mut world, &parse(SHULKER), items()).unwrap();
    apply_value(&mut world, full, &emptied).unwrap();
    assert!(world.get::<SlotTable>(full).is_none());
    assert_eq!(world.query::<&Held>().iter(&world).count(), 0);
    apply_value(&mut world, full, &parse(SHULKER)).unwrap();
    assert_eq!(world.get::<SlotTable>(full).unwrap().len(), 27);
    assert_eq!(stack_to_value(&world, full, items()), parse(SHULKER));
}

#[test]
fn a_count_outside_the_byte_is_refused() {
    let mut world = world();
    let stone = spawn_stack(
        &mut world,
        &parse(r#"{"id": "minecraft:stone", "count": 2}"#),
        items(),
    )
    .unwrap();
    for count in [0, -1, 256] {
        let mut value = parse(r#"{"id": "minecraft:stone"}"#);
        value.count = mcrs_minecraft_core::codec::Bounded(count);
        assert!(matches!(
            spawn_stack(&mut world, &value, items()),
            Err(mcrs_minecraft_item::StackError::BadCount { count: c, .. }) if c == count
        ));
        assert!(matches!(
            apply_value(&mut world, stone, &value),
            Err(mcrs_minecraft_item::StackError::BadCount { .. })
        ));
    }
    assert_eq!(world.get::<ItemStack>(stone).unwrap().count(), 2);
    assert_eq!(world.query::<&ItemStack>().iter(&world).count(), 1);
}

#[test]
fn a_crossbow_refuses_a_projectile_past_the_codec_bound() {
    let mut world = world();
    let crossbow = spawn_stack(
        &mut world,
        &parse(r#"{"id": "minecraft:crossbow"}"#),
        items(),
    )
    .unwrap();
    let arrow = spawn_stack(&mut world, &parse(r#"{"id": "minecraft:arrow"}"#), items()).unwrap();
    let bound = mcrs_minecraft_protocol::item::MAX_CHARGED_PROJECTILES as u16;
    assert!(matches!(
        place(&mut world, arrow, crossbow, bound),
        Err(TransactionError::OutOfRange { .. })
    ));
    place(&mut world, arrow, crossbow, bound - 1).unwrap();
    let value = stack_to_value(&world, crossbow, items());
    assert_eq!(value.components.added.len(), 1);
}

#[test]
fn reapplying_a_value_bumps_the_revision_once() {
    let mut world = world();
    let chest = world.spawn(SlotTable::fixed(1)).id();
    let shulker = spawn_stack(&mut world, &parse(SHULKER), items()).unwrap();
    place(&mut world, shulker, chest, 0).unwrap();
    let revision = common::revision(&world, shulker);
    apply_value(&mut world, shulker, &parse(SHULKER)).unwrap();
    assert_eq!(common::revision(&world, shulker), revision + 1);
}

#[test]
fn a_campfire_table_is_bounded_but_not_allocated() {
    let mut world = world();
    let campfire = spawn_stack(
        &mut world,
        &parse(r#"{"id": "minecraft:campfire"}"#),
        items(),
    )
    .unwrap();
    let table = world.get::<SlotTable>(campfire).unwrap();
    assert_eq!(table.len(), 256);
    assert!(!table.is_empty());
    assert!(table.iter().next().is_none());
}
