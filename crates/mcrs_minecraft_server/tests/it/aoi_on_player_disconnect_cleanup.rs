//! Mid-transit disconnect cleanup — five scenarios covering each tick of
//! the cross-dim transfer choreography. Each scenario stages the
//! relevant state then drives the disconnect protocol directly via
//! `run_system_once` and `Disconnects`.
//!
//! Constructing a real `ServerSideConnection` requires a `RawConnection`
//! socket that integration tests cannot reach, so the tests exercise the
//! protocol pipeline (resources + helpers + filter system) rather than
//! the `On<Remove, ServerSideConnection>` trigger itself. The trigger
//! path is the thin shim documented in `session_lifecycle.rs`.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::system::RunSystemOnce;
use mcrs_minecraft_engine::bus::{
    InboundPlayerDespawn, InboundPlayerSpawn, OutboundPlayerAttached, OutboundPlayerDisconnect,
};
use mcrs_minecraft_engine::channel_types::ToDim;
use mcrs_minecraft_engine::dim_inputs::DimInputs;
use mcrs_minecraft_engine::disconnect::{
    DisconnectProtocolPlugin, Disconnects, filter_inflight_for_disconnect,
};
use mcrs_minecraft_engine::session::SessionBundle;
use mcrs_minecraft_engine_world::session::{
    Place, PlayerSession, PlayerSessionCounter, SessionPlacement,
};

fn build_disconnect_app() -> App {
    let mut app = App::new();
    app.add_message::<InboundPlayerSpawn>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.add_message::<InboundPlayerDespawn>();
    app.init_resource::<PlayerSessionCounter>();
    app.init_resource::<DimInputs>();
    app.add_plugins(DisconnectProtocolPlugin);
    app
}

/// Lets the app's `DimInputs` take input for `dim`, so tests can assert on its `ToDim::Despawn`
/// messages.
fn register_dim_channel(app: &mut App, dim: Entity) {
    app.world_mut().resource_mut::<DimInputs>().insert(dim);
}

fn count_despawns(app: &App, dim: Entity) -> usize {
    app.world()
        .resource::<DimInputs>()
        .control(dim)
        .iter()
        .filter(|m| matches!(m, ToDim::Despawn(..)))
        .count()
}

fn insert_joining(app: &mut App, host_anchor: Entity, dim: Entity) -> PlayerSession {
    let session = app
        .world_mut()
        .resource_mut::<PlayerSessionCounter>()
        .next();
    app.world_mut()
        .entity_mut(host_anchor)
        .insert(SessionBundle::placed(
            session,
            SessionPlacement::new(Place::Joining(dim), 0),
        ));
    session
}

fn synthetic_disconnect(app: &mut App, host_anchor: Entity) {
    app.world_mut()
        .run_system_once(move |mut disconnects: Disconnects| disconnects.disconnect(host_anchor))
        .expect("disconnect helper runs");
}

fn run_filter(app: &mut App) {
    app.world_mut()
        .run_system_once(filter_inflight_for_disconnect)
        .expect("filter runs");
}

#[test]
fn disconnect_at_tick_n_e1_4_attached_pending_filter() {
    let mut app = build_disconnect_app();

    let host_anchor = app.world_mut().spawn_empty().id();
    let dim = Entity::from_raw_u32(402).unwrap();
    register_dim_channel(&mut app, dim);
    let session = insert_joining(&mut app, host_anchor, dim);

    app.world_mut()
        .resource_mut::<Messages<OutboundPlayerAttached>>()
        .write(OutboundPlayerAttached { session });

    synthetic_disconnect(&mut app, host_anchor);
    run_filter(&mut app);

    let mut attached_msgs = app
        .world_mut()
        .resource_mut::<Messages<OutboundPlayerAttached>>();
    let remaining: Vec<_> = attached_msgs.drain().collect();
    assert!(
        remaining.is_empty(),
        "OutboundPlayerAttached filtered, got {}",
        remaining.len()
    );

    assert_eq!(count_despawns(&app, dim), 1);

    assert!(
        app.world().get_entity(host_anchor).is_err(),
        "the session is despawned"
    );
}
