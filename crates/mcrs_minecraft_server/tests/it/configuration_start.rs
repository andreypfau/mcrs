use crate::mock_connection;

use bevy_app::{App, Update};
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::prelude::NextState;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_network::{ConnectionState, ServerSideConnection};
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
