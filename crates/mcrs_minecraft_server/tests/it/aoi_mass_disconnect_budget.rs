//! Per-tick disconnect budget — four scenarios bounding orphan-state
//! accumulation under mass-disconnect pressure.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use mcrs_minecraft_engine::muxer::disconnect::PendingDisconnectQueue;
use mcrs_minecraft_engine_world::session::{Place, Session};

use crate::disconnect_support::{
    build_app, despawn_count, disconnect, insert_player, register_dim_channel, run_drain,
    run_filter,
};

fn session_count(app: &mut App) -> usize {
    app.world_mut()
        .query::<&Session>()
        .iter(app.world())
        .count()
}

fn spawn_anchors(app: &mut App, count: usize, dim: Entity) -> Vec<Entity> {
    let mut anchors = Vec::with_capacity(count);
    for _ in 0..count {
        let host_anchor = app.world_mut().spawn_empty().id();
        insert_player(app, host_anchor, Place::InDim(dim));
        anchors.push(host_anchor);
    }
    anchors
}

#[test]
fn e4_1_100_simultaneous_disconnects_process_32_per_tick() {
    let mut app = build_app();
    let dim = Entity::from_raw_u32(900).unwrap();
    register_dim_channel(&mut app, dim);
    let anchors = spawn_anchors(&mut app, 100, dim);

    disconnect(&mut app, &anchors);
    run_filter(&mut app);

    let processed = despawn_count(&app, dim);
    assert_eq!(
        processed, 32,
        "exactly budget-count (32) processed in the first tick",
    );
    assert_eq!(
        app.world()
            .resource::<PendingDisconnectQueue>()
            .entries
            .len(),
        68,
        "remaining 68 anchors queued",
    );

    // Tick 2: drain refills budget and processes 32 more.
    run_drain(&mut app);
    run_filter(&mut app);
    let processed_2 = despawn_count(&app, dim);
    assert_eq!(processed_2, 64, "32 + 32 processed");
    assert_eq!(
        app.world()
            .resource::<PendingDisconnectQueue>()
            .entries
            .len(),
        36,
    );

    // Tick 3: another 32.
    run_drain(&mut app);
    run_filter(&mut app);
    let processed_3 = despawn_count(&app, dim);
    assert_eq!(processed_3, 96);
    assert_eq!(
        app.world()
            .resource::<PendingDisconnectQueue>()
            .entries
            .len(),
        4,
    );

    // Tick 4: remaining 4 drained; queue empty.
    run_drain(&mut app);
    run_filter(&mut app);
    let processed_4 = despawn_count(&app, dim);
    assert_eq!(processed_4, 100);
    assert!(
        app.world()
            .resource::<PendingDisconnectQueue>()
            .entries
            .is_empty(),
        "queue drained after 4 ticks",
    );

    assert!(session_count(&mut app) == 0, "every session cleared",);
}
