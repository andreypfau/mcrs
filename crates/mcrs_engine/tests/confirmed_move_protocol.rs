//! Engine-level primitives the confirmed-move protocol is built on: the
//! `InTransit` hide marker (keep-until-confirm / un-hide-on-rollback) and the
//! source-allocated move id. The end-to-end four-message protocol is covered as
//! an integration test in mcrs_minecraft (`confirmed_move_roundtrip`).

use bevy_ecs::prelude::*;
use mcrs_engine::entity::InTransit;
use mcrs_engine::world::in_flight::alloc_move_id;

fn without_in_transit_contains(world: &mut World, target: Entity) -> bool {
    let mut q = world.query_filtered::<Entity, Without<InTransit>>();
    q.iter(world).any(|e| e == target)
}

#[test]
fn in_transit_marker_hides_then_un_hides_the_same_entity() {
    let mut world = World::new();
    let e = world.spawn_empty().id();

    // Before move-out the entity is visible to every source-dim system, which all
    // filter `Without<InTransit>`.
    assert!(without_in_transit_contains(&mut world, e));

    // Hide-on-move-out: stamping the marker excludes it from those systems while
    // the entity stays live (it is NOT despawned).
    let id = alloc_move_id();
    world.entity_mut(e).insert(InTransit { move_id: id });
    assert!(!without_in_transit_contains(&mut world, e), "hidden in transit");

    // Un-hide-on-rollback: removing the marker brings the same entity back into
    // view — never a despawn, so it reappears exactly where it was.
    world.entity_mut(e).remove::<InTransit>();
    assert!(
        without_in_transit_contains(&mut world, e),
        "entity is visible again after un-hide"
    );
}

#[test]
fn in_transit_carries_the_move_id_for_confirm_rollback_matching() {
    let mut world = World::new();
    let id = alloc_move_id();
    let e = world.spawn(InTransit { move_id: id }).id();
    // The source resolves confirm/rollback by matching this id against the id the
    // host echoes back, so it must round-trip exactly.
    assert_eq!(world.get::<InTransit>(e).unwrap().move_id, id);
}
