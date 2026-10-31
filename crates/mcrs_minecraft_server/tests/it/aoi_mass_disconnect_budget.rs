//! Per-tick disconnect budget — four scenarios bounding orphan-state
//! accumulation under mass-disconnect pressure.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::system::RunSystemOnce;
use mcrs_minecraft_engine::bus::{
    InboundPlayerDespawn, OutboundPlayerAttached, OutboundPlayerDisconnect,
};
use mcrs_minecraft_engine::channel_types::FromDim;
use mcrs_minecraft_engine::channel_types::{DimChannelsResource, ToDim};
use mcrs_minecraft_engine::disconnect::{
    DisconnectProtocolPlugin, Disconnects, PendingDisconnectQueue, drain_pending_disconnects,
    filter_inflight_for_disconnect,
};
use mcrs_minecraft_engine::session::SessionBundle;
use mcrs_minecraft_engine_world::session::{
    Place, PlayerSessionCounter, Session, SessionPlacement,
};
use mcrs_minecraft_engine_world::world::channels::{
    FROM_DIM_CAPACITY, TO_DIM_CAPACITY, TO_DIM_CONTROL_CAPACITY,
};

fn build_app() -> App {
    let mut app = App::new();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.add_message::<InboundPlayerDespawn>();
    app.init_resource::<PlayerSessionCounter>();
    app.init_resource::<DimChannelsResource>();
    app.add_plugins(DisconnectProtocolPlugin);
    app
}

fn register_dim_channel(app: &mut App, dim: Entity) -> flume::Receiver<ToDim> {
    let (srv_tx, _srv_rx) = flume::bounded::<ToDim>(TO_DIM_CAPACITY);
    let (ctl_tx, ctl_rx) = flume::bounded::<ToDim>(TO_DIM_CONTROL_CAPACITY);
    let (_from_tx, from_rx) = flume::bounded::<FromDim>(FROM_DIM_CAPACITY);
    app.world_mut()
        .resource_mut::<DimChannelsResource>()
        .insert(dim, srv_tx, ctl_tx, from_rx);
    ctl_rx
}

fn insert_player(app: &mut App, host_anchor: Entity, dim: Entity) {
    let session = app
        .world_mut()
        .resource_mut::<PlayerSessionCounter>()
        .next();
    app.world_mut()
        .entity_mut(host_anchor)
        .insert(SessionBundle::placed(
            session,
            SessionPlacement::new(Place::InDim(dim), 0),
        ));
}

fn session_count(app: &mut App) -> usize {
    app.world_mut()
        .query::<&Session>()
        .iter(app.world())
        .count()
}

fn spawn_anchors(app: &mut App, count: usize, dim: Entity) -> Vec<Entity> {
    let mut anchors = Vec::with_capacity(count);
    for _ in 0..count {
        let e = app.world_mut().spawn_empty().id();
        insert_player(app, e, dim);
        anchors.push(e);
    }
    anchors
}

fn fire_disconnect(app: &mut App, anchors: &[Entity]) {
    let anchors = anchors.to_vec();
    app.world_mut()
        .run_system_once(move |mut disconnects: Disconnects| {
            for &host_anchor in &anchors {
                disconnects.disconnect(host_anchor);
            }
        })
        .expect("disconnect batch runs");
}

fn tick_first_schedule(app: &mut App) {
    app.world_mut()
        .run_system_once(drain_pending_disconnects)
        .expect("First-schedule drain");
}

fn tick_update_schedule(app: &mut App) {
    app.world_mut()
        .run_system_once(filter_inflight_for_disconnect)
        .expect("Update-schedule filter");
}

fn dim_despawn_count(ctl_rx: &flume::Receiver<ToDim>) -> usize {
    ctl_rx
        .try_iter()
        .filter(|msg| matches!(msg, ToDim::Despawn(..)))
        .count()
}

#[test]
fn e4_1_100_simultaneous_disconnects_process_32_per_tick() {
    let mut app = build_app();
    let dim = Entity::from_raw_u32(900).unwrap();
    let ctl_rx = register_dim_channel(&mut app, dim);
    let anchors = spawn_anchors(&mut app, 100, dim);

    fire_disconnect(&mut app, &anchors);
    tick_update_schedule(&mut app);

    let processed = dim_despawn_count(&ctl_rx);
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
    tick_first_schedule(&mut app);
    tick_update_schedule(&mut app);
    let processed_2 = processed + dim_despawn_count(&ctl_rx);
    assert_eq!(processed_2, 64, "32 + 32 processed");
    assert_eq!(
        app.world()
            .resource::<PendingDisconnectQueue>()
            .entries
            .len(),
        36,
    );

    // Tick 3: another 32.
    tick_first_schedule(&mut app);
    tick_update_schedule(&mut app);
    let processed_3 = processed_2 + dim_despawn_count(&ctl_rx);
    assert_eq!(processed_3, 96);
    assert_eq!(
        app.world()
            .resource::<PendingDisconnectQueue>()
            .entries
            .len(),
        4,
    );

    // Tick 4: remaining 4 drained; queue empty.
    tick_first_schedule(&mut app);
    tick_update_schedule(&mut app);
    let processed_4 = processed_3 + dim_despawn_count(&ctl_rx);
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
