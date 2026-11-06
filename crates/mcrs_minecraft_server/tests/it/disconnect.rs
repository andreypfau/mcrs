use bevy_app::App;
use bevy_ecs::entity::Entity;
use mcrs_minecraft_engine::bus::{
    InboundPlayerDespawn, InboundPlayerSpawn, OutboundPlayerAttached,
};
use mcrs_minecraft_engine::channel_types::ToDim;
use mcrs_minecraft_engine::muxer::dim_inputs::DimInputs;
use mcrs_minecraft_engine::muxer::disconnect::DisconnectProtocolPlugin;
use mcrs_minecraft_engine::muxer::session::{HostAnchorRef, SessionBundle};
use mcrs_minecraft_engine_world::session::{
    Place, PlayerSessionCounter, Session, SessionPlacement,
};
use mcrs_minecraft_engine_world::world::sub_app::DimDespawnQueue;
use mcrs_minecraft_network::ServerSideConnection;

use crate::mock_connection;

fn build_app(dim: Entity) -> App {
    let mut app = App::new();
    app.add_message::<InboundPlayerSpawn>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<InboundPlayerDespawn>();
    app.init_resource::<PlayerSessionCounter>();
    app.init_resource::<DimInputs>();
    app.init_resource::<DimDespawnQueue>();
    app.add_plugins(DisconnectProtocolPlugin);
    app.world_mut().resource_mut::<DimInputs>().insert(dim);
    app
}

/// Spawns a player at `place` and its connection; returns the connection.
fn connect(app: &mut App, place: Place) -> Entity {
    let session = app
        .world_mut()
        .resource_mut::<PlayerSessionCounter>()
        .next();
    let host_anchor = app
        .world_mut()
        .spawn(SessionBundle::placed(
            session,
            SessionPlacement::new(place, 0),
        ))
        .id();
    let (raw, _outgoing) = mock_connection::make_mock_raw_connection();
    app.world_mut()
        .spawn((
            ServerSideConnection { raw: Box::new(raw) },
            HostAnchorRef(host_anchor),
        ))
        .id()
}

fn close(app: &mut App, connection: Entity) {
    app.world_mut()
        .entity_mut(connection)
        .remove::<ServerSideConnection>();
}

fn despawn_count(app: &App, dim: Entity) -> usize {
    app.world()
        .resource::<DimInputs>()
        .control(dim)
        .iter()
        .filter(|message| matches!(message, ToDim::Despawn(..)))
        .count()
}

fn session_count(app: &mut App) -> usize {
    app.world_mut()
        .query::<&Session>()
        .iter(app.world())
        .count()
}

#[test]
fn every_disconnect_of_a_mass_disconnect_is_cleaned_up_at_once() {
    let dim = Entity::from_raw_u32(900).unwrap();
    let mut app = build_app(dim);
    let connections: Vec<Entity> = (0..100)
        .map(|_| connect(&mut app, Place::InDim(dim)))
        .collect();

    for connection in connections {
        close(&mut app, connection);
    }

    assert_eq!(despawn_count(&app, dim), 100);
    assert_eq!(session_count(&mut app), 0, "every session is despawned");
}

#[test]
fn a_joining_player_that_disconnects_sends_its_dimension_a_despawn_and_loses_its_session() {
    let dim = Entity::from_raw_u32(402).unwrap();
    let mut app = build_app(dim);
    let connection = connect(&mut app, Place::Joining(dim));

    close(&mut app, connection);

    assert_eq!(despawn_count(&app, dim), 1);
    assert_eq!(session_count(&mut app), 0, "the session is despawned");
}
