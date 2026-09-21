mod common;

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::With;
use bevy_ecs::world::World;
use mcrs_minecraft_inventory::{Op, Slot};
use mcrs_minecraft_item::{DroppedItem, Held, ItemStack, SlotTable, Thrower};

use common::{apply, holder, place, spawn, world};

fn held_and_dropped(world: &mut World) -> usize {
    world
        .query_filtered::<(), (With<Held>, With<DroppedItem>)>()
        .iter(world)
        .count()
}

#[test]
fn never_held_and_dropped() {
    let mut world = world();
    let player = holder(&mut world, 47);
    let stone = spawn(&mut world, "stone", 20);
    place(&mut world, stone, player, 36).unwrap();

    apply(
        &mut world,
        vec![Op::Drop {
            from: Slot::new(player, 36),
            count: 20,
            thrower: player,
        }],
    )
    .unwrap();
    assert!(world.get::<Held>(stone).is_none());
    assert_eq!(
        world.get::<DroppedItem>(stone),
        Some(&DroppedItem {
            age: 0,
            pickup_delay: 40,
            health: 5
        })
    );
    assert_eq!(world.get::<Thrower>(stone), Some(&Thrower(player)));
    assert_eq!(world.get::<SlotTable>(player).unwrap().get(36), None);
    assert_eq!(held_and_dropped(&mut world), 0);

    let loose = world.spawn_empty().id();
    apply(
        &mut world,
        vec![Op::SpawnDropped {
            entity: loose,
            value: common::value("stone", 8, Default::default()),
            pickup_delay: 40,
            thrower: None,
        }],
    )
    .unwrap();
    assert_eq!(held_and_dropped(&mut world), 0);

    apply(
        &mut world,
        vec![Op::MergeDropped {
            from: loose,
            into: stone,
        }],
    )
    .unwrap();
    assert!(world.get_entity(loose).is_err());
    assert_eq!(world.get::<ItemStack>(stone).unwrap().count, 28);
    assert_eq!(held_and_dropped(&mut world), 0);

    apply(
        &mut world,
        vec![Op::Pickup {
            item: stone,
            to: Slot::new(player, 0),
            count: 28,
        }],
    )
    .unwrap();
    assert!(world.get_entity(stone).is_err());
    let picked: Entity = world.get::<SlotTable>(player).unwrap().get(0).unwrap();
    assert_eq!(world.get::<ItemStack>(picked).unwrap().count, 28);
    assert_eq!(held_and_dropped(&mut world), 0);
}

#[test]
fn merging_dropped_items_keeps_the_longer_delay_and_the_younger_age() {
    let mut world = world();
    let drop = |world: &mut World, count: i32, delay: i16, age: i16| {
        let entity = world.spawn_empty().id();
        apply(
            world,
            vec![Op::SpawnDropped {
                entity,
                value: common::value("stone", count, Default::default()),
                pickup_delay: delay,
                thrower: None,
            }],
        )
        .unwrap();
        world.get_mut::<DroppedItem>(entity).unwrap().age = age;
        entity
    };
    let a = drop(&mut world, 3, 10, 50);
    let b = drop(&mut world, 7, 30, 20);
    apply(&mut world, vec![Op::MergeDropped { from: a, into: b }]).unwrap();
    let merged = world.get::<DroppedItem>(b).unwrap();
    assert_eq!((merged.pickup_delay, merged.age), (30, 20));
    assert_eq!(world.get::<ItemStack>(b).unwrap().count, 10);
    let full = drop(&mut world, 60, 0, 0);
    apply(
        &mut world,
        vec![Op::MergeDropped {
            from: b,
            into: full,
        }],
    )
    .unwrap();
    assert_eq!(
        world.get::<ItemStack>(b).unwrap().count,
        10,
        "a merge past the max is refused"
    );
}

#[test]
#[cfg_attr(not(debug_assertions), ignore)]
#[should_panic(expected = "held and dropped")]
fn inserting_dropped_on_a_held_stack_panics() {
    let mut world = world();
    let chest = holder(&mut world, 1);
    let stone = spawn(&mut world, "stone", 1);
    place(&mut world, stone, chest, 0).unwrap();
    world.entity_mut(stone).insert(DroppedItem {
        age: 0,
        pickup_delay: 0,
        health: 5,
    });
}
