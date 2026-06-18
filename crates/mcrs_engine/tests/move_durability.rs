//! Engine-level durability primitive: the host-resident in-flight move table and
//! its deterministic, tick-based timeout. The full source→host→source rollback is
//! covered as an integration test in mcrs_minecraft (`confirmed_move_roundtrip`);
//! here we pin the engine mechanism the rollback depends on.

use bevy_ecs::entity::Entity;
use mcrs_engine::session::PlayerSession;
use mcrs_engine::world::in_flight::{alloc_move_id, InFlightEntry, InFlightMoves};

fn entry() -> InFlightEntry {
    InFlightEntry {
        source_dim: Entity::PLACEHOLDER,
        hidden_entity: Entity::PLACEHOLDER,
        session: Some(PlayerSession(1)),
        ticks_elapsed: 0,
    }
}

#[test]
fn tick_all_times_out_exactly_at_threshold() {
    let mut moves = InFlightMoves::default();
    moves.timeout_ticks = 3;
    let id = alloc_move_id();
    moves.insert(id, entry());

    assert!(moves.tick_all().is_empty(), "ticks=1, below threshold");
    assert!(moves.tick_all().is_empty(), "ticks=2, below threshold");
    assert_eq!(moves.tick_all(), vec![id], "ticks=3, at threshold");

    // tick_all reports but does not remove; the caller drives the rollback then removes.
    assert!(moves.remove(id).is_some(), "entry present until removed");
    assert!(moves.get(id).is_none());
    assert!(moves.tick_all().is_empty(), "removed entry never fires again");
}

#[test]
fn entry_removed_before_timeout_never_fires() {
    let mut moves = InFlightMoves::default();
    moves.timeout_ticks = 5;
    let id = alloc_move_id();
    moves.insert(id, entry());
    moves.tick_all(); // ticks=1

    // A Spawned ack removes the entry before the timeout — it must never time out.
    assert!(moves.remove(id).is_some());
    for _ in 0..10 {
        assert!(moves.tick_all().is_empty());
    }
}

#[test]
fn multiple_in_flight_moves_time_out_independently() {
    let mut moves = InFlightMoves::default();
    moves.timeout_ticks = 2;
    let a = alloc_move_id();
    let b = alloc_move_id();
    assert_ne!(a, b, "source-allocated ids are unique");
    moves.insert(a, entry());
    moves.tick_all(); // a: ticks=1
    moves.insert(b, entry()); // b: ticks=0
    let timed_out = moves.tick_all(); // a: ticks=2 (fires), b: ticks=1
    assert_eq!(timed_out, vec![a], "only the older move has reached the threshold");
}
