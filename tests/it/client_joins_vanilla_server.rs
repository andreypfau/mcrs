use bevy_app::App;
use mcrs_minecraft_client::columns::{ColumnCachePlugin, ColumnStore};
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::{ClientNetworkPlugin, JoinedGame, PendingTeleports};
use mcrs_minecraft_registry::RegistrySet;
use std::net::SocketAddr;

use crate::support::{
    JOIN_TIMEOUT, assert_inventory_resolves_through_session, assert_session_registries,
    drive_client_until_joined, insert_block_catalog, insert_inventory, insert_session_inputs,
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
    insert_session_inputs(&mut client);
    insert_inventory(&mut client);

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

    assert_session_registries(&client, connection);
    assert_inventory_resolves_through_session(&mut client, connection);

    let world = client.world();
    let registries = world.resource::<RegistrySet>();
    println!("registries received: {}", registries.tables().count());
    println!("synced registries: {}", registries.synced().count());
    println!("columns stored: {}", world.resource::<ColumnStore>().len());
}
