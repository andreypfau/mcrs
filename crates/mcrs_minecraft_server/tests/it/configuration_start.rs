use crate::mock_connection;

use bevy_app::{App, Update};
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::prelude::NextState;
use bytes::BytesMut;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_network::identity;
use mcrs_minecraft_network::{ConnectionState, ServerSideConnection};
use mcrs_minecraft_protocol::Decode;
use mcrs_minecraft_protocol::Packet;
use mcrs_minecraft_protocol::decode::PacketDecoder;
use mcrs_minecraft_protocol::packets::common::clientbound::Payload;
use mcrs_minecraft_protocol::packets::configuration::clientbound::{
    ClientboundCustomPayload, ClientboundSelectKnownPacks,
};
use mcrs_minecraft_server::configuration::{AwaitingKnownPacks, start_configuration};

#[test]
fn a_connection_that_reaches_configuration_while_loading_waits_for_the_registries() {
    let mut app = App::new();
    app.add_plugins(StatesPlugin)
        .init_state::<AppState>()
        .add_systems(Update, start_configuration());
    let (raw, _outgoing) = mock_connection::make_mock_raw_connection();
    let connection = app
        .world_mut()
        .spawn((
            ServerSideConnection { raw: Box::new(raw) },
            ConnectionState::Configuration,
        ))
        .id();

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::LoadingDataPack);
    app.update();
    app.update();
    assert!(
        !app.world()
            .entity(connection)
            .contains::<AwaitingKnownPacks>()
    );

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    app.update();
    assert!(
        app.world()
            .entity(connection)
            .contains::<AwaitingKnownPacks>()
    );
}

#[test]
fn the_first_configuration_frame_is_the_brand() {
    let mut app = App::new();
    app.add_plugins(StatesPlugin)
        .init_state::<AppState>()
        .add_systems(Update, start_configuration());
    let (raw, mut outgoing) = mock_connection::make_mock_raw_connection();
    let connection = app
        .world_mut()
        .spawn((
            ServerSideConnection { raw: Box::new(raw) },
            ConnectionState::Configuration,
        ))
        .id();
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    app.update();
    app.update();

    let mut con = app
        .world_mut()
        .get_mut::<ServerSideConnection>(connection)
        .unwrap();
    con.raw.flush().unwrap();
    let mut decoder = PacketDecoder::new();
    while let Ok(blob) = outgoing.try_recv() {
        decoder.queue_bytes(BytesMut::from(&blob[..]));
    }
    let frames: Vec<_> = std::iter::from_fn(|| decoder.try_next_packet().unwrap()).collect();

    let ids: Vec<i32> = frames.iter().map(|frame| frame.id).collect();
    assert_eq!(
        ids,
        [
            ClientboundCustomPayload::ID,
            ClientboundSelectKnownPacks::ID
        ]
    );
    let Payload::Brand(brand) = Payload::decode(&mut &frames[0].body[..]).unwrap() else {
        panic!("the first payload is not a brand");
    };
    assert_eq!(brand.brand, identity::BRAND);
    assert!(
        app.world()
            .entity(connection)
            .contains::<AwaitingKnownPacks>()
    );
}
