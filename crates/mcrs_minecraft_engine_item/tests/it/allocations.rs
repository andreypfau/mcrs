use bevy_ecs::entity::Entity;
use bevy_ecs::system::SystemState;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::StackRows;
use mcrs_test_alloc::allocations;

use crate::fixture::LORED;
use crate::fixture::SWORD;
use crate::fixture::TABLE;

fn spawn_item(world: &mut World) -> Entity {
    let stack = world.commands().spawn_item(LORED).id();
    world.flush();
    stack
}

#[test]
fn an_unpatched_spawn_and_a_merge_check_allocate_nothing() {
    let mut world = World::new();
    world.insert_resource(*TABLE);
    let mut state = SystemState::<StackRows>::new(&mut world);
    let sword = world.commands().spawn_item(SWORD).id();
    world.flush();
    for _ in 0..8 {
        let stack = spawn_item(&mut world);
        world.despawn(stack);
    }

    let before = allocations();
    let left = spawn_item(&mut world);
    let spawned = allocations() - before;
    let right = spawn_item(&mut world);
    assert!(state.get(&world).unwrap().mergeable(left, right));
    assert!(!state.get(&world).unwrap().mergeable(left, sword));

    let before = allocations();
    let mergeable = state.get(&world).unwrap().mergeable(left, right);
    let merge_checked = allocations() - before;

    assert!(mergeable);
    assert_eq!((spawned, merge_checked), (0, 0));
}
