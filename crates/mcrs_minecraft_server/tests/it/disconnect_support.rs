use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::system::RunSystemOnce;
use mcrs_minecraft_engine::bus::{
    InboundPlayerDespawn, InboundPlayerSpawn, OutboundPlayerAttached, OutboundPlayerDisconnect,
};
use mcrs_minecraft_engine::channel_types::ToDim;
use mcrs_minecraft_engine::dim_inputs::DimInputs;
use mcrs_minecraft_engine::disconnect::{
    DisconnectProtocolPlugin, Disconnects, drain_pending_disconnects,
    filter_inflight_for_disconnect,
};
use mcrs_minecraft_engine::session::SessionBundle;
use mcrs_minecraft_engine_world::session::{
    Place, PlayerSession, PlayerSessionCounter, SessionPlacement,
};
use mcrs_minecraft_engine_world::world::sub_app::DimDespawnQueue;

/// The disconnect protocol alone, with the bus messages and resources it reads and writes.
pub fn build_app() -> App {
    let mut app = App::new();
    app.add_message::<InboundPlayerSpawn>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.add_message::<InboundPlayerDespawn>();
    app.init_resource::<PlayerSessionCounter>();
    app.init_resource::<DimInputs>();
    app.init_resource::<DimDespawnQueue>();
    app.add_plugins(DisconnectProtocolPlugin);
    app
}

/// Lets the app's `DimInputs` take input for `dim`, so a test can count its despawns.
pub fn register_dim_channel(app: &mut App, dim: Entity) {
    app.world_mut().resource_mut::<DimInputs>().insert(dim);
}

pub fn insert_player(app: &mut App, host_anchor: Entity, place: Place) -> PlayerSession {
    let session = app
        .world_mut()
        .resource_mut::<PlayerSessionCounter>()
        .next();
    app.world_mut()
        .entity_mut(host_anchor)
        .insert(SessionBundle::placed(
            session,
            SessionPlacement::new(place, 0),
        ));
    session
}

pub fn disconnect(app: &mut App, host_anchors: &[Entity]) {
    let host_anchors = host_anchors.to_vec();
    app.world_mut()
        .run_system_once(move |mut disconnects: Disconnects| {
            for &host_anchor in &host_anchors {
                disconnects.disconnect(host_anchor);
            }
        })
        .expect("disconnect batch runs");
}

pub fn run_drain(app: &mut App) {
    app.world_mut()
        .run_system_once(drain_pending_disconnects)
        .expect("drain runs");
}

pub fn run_filter(app: &mut App) {
    app.world_mut()
        .run_system_once(filter_inflight_for_disconnect)
        .expect("filter runs");
}

/// The despawns that `dim` holds for its next tick, which never comes here.
pub fn despawn_count(app: &App, dim: Entity) -> usize {
    app.world()
        .resource::<DimInputs>()
        .control(dim)
        .iter()
        .filter(|message| matches!(message, ToDim::Despawn(..)))
        .count()
}
