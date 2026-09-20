mod common;

use bevy_ecs::prelude::With;
use bevy_ecs::world::World;
use mcrs_minecraft_item::dropped::spawn_dropped;
use mcrs_minecraft_item::{DroppedItem, Held, SlotTable, Thrower, mutate};

use common::{drain, holder, items, spawn, world};

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
    mutate::move_stack(&mut world, stone, player, 36).unwrap();
    drain(&mut world);

    spawn_dropped(&mut world, stone, 40, Some(player));
    assert!(world.get::<Held>(stone).is_none());
    assert_eq!(
        world.get::<DroppedItem>(stone),
        Some(&DroppedItem { age: 0, pickup_delay: 40, health: 5 })
    );
    assert_eq!(world.get::<Thrower>(stone), Some(&Thrower(player)));
    assert_eq!(world.get::<SlotTable>(player).unwrap().get(36), None);
    let dirty = drain(&mut world);
    assert_eq!(dirty.cells, [(player, 36)]);
    assert_eq!(dirty.roots, [stone]);
    assert_eq!(held_and_dropped(&mut world), 0);

    let half = mutate::split(&mut world, stone, 8, items()).unwrap();
    spawn_dropped(&mut world, half, 40, None);
    assert_eq!(held_and_dropped(&mut world), 0);

    assert_eq!(mutate::merge_into(&mut world, half, stone, 64), 8);
    assert!(world.get_entity(half).is_err());
    assert_eq!(held_and_dropped(&mut world), 0);

    world.entity_mut(stone).remove::<DroppedItem>();
    mutate::move_stack(&mut world, stone, player, 0).unwrap();
    assert_eq!(held_and_dropped(&mut world), 0);
    assert_eq!(world.get::<SlotTable>(player).unwrap().get(0), Some(stone));
}

#[test]
#[cfg_attr(not(debug_assertions), ignore)]
#[should_panic(expected = "held and dropped")]
fn inserting_dropped_on_a_held_stack_panics() {
    let mut world = world();
    let chest = holder(&mut world, 1);
    let stone = spawn(&mut world, "stone", 1);
    mutate::move_stack(&mut world, stone, chest, 0).unwrap();
    world.entity_mut(stone).insert(DroppedItem {
        age: 0,
        pickup_delay: 0,
        health: 5,
    });
}
