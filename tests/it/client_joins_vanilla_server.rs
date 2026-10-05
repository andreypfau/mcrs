use bevy_app::App;
use mcrs_minecraft_client::columns::{ColumnCachePlugin, ColumnStore};
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::{
    ClientNetworkPlugin, JoinedGame, PendingTeleports, ReceivedRegistries,
};
use std::net::SocketAddr;

use crate::support::{
    JOIN_TIMEOUT, drive_client_until_joined, insert_block_catalog, insert_local_registries,
};

#[test]
#[ignore = "needs a running vanilla server; tools/vanilla-server/run.sh starts one and runs this"]
fn the_client_logs_in_configures_and_joins_a_vanilla_server() {
    let address: SocketAddr = std::env::var("MCRS_VANILLA_SERVER")
        .expect("MCRS_VANILLA_SERVER names the vanilla server to join, as <host>:<port>")
        .parse()
        .expect("MCRS_VANILLA_SERVER is not a <host>:<port> socket address");

    let mut client = App::new();
    client.add_plugins(ClientNetworkPlugin {
        server: address,
        username: "mcrs_test".to_owned(),
        profile_id: None,
        view_distance: 6,
    });
    client.add_plugins(ColumnCachePlugin);
    insert_block_catalog(&mut client);
    insert_local_registries(&mut client);

    let Some(connection) = drive_client_until_joined(&mut client) else {
        panic!("the client never reached the play state within {JOIN_TIMEOUT:?}");
    };

    let world = client.world();
    assert_eq!(
        *world.get::<ConnectionState>(connection).unwrap(),
        ConnectionState::Game
    );
    assert!(world.get::<JoinedGame>(connection).is_some());
    assert!(
        world
            .get::<PendingTeleports>(connection)
            .is_some_and(|teleports| !teleports.0.is_empty()),
        "no teleport arrived for the client to confirm"
    );
    let store = world.resource::<ColumnStore>();
    assert!(!store.is_empty(), "no column reached the store");

    let registries = world.get::<ReceivedRegistries>(connection).unwrap();
    println!("registries received: {}", registries.0.len());
    println!("columns stored: {}", store.len());
}
