use mcrs_minecraft_client::columns::ColumnStore;
use mcrs_minecraft_client::joined_game::JoinedGame;
use mcrs_minecraft_network::client::PendingTeleports;
use std::net::SocketAddr;

use crate::support::{
    JOIN_TIMEOUT, assert_session_registries, drive_client_until_joined, joining_client,
};

#[test]
fn the_client_logs_in_configures_and_joins_a_vanilla_server() {
    let address: SocketAddr = std::env::var("MCRS_VANILLA_SERVER")
        .expect("MCRS_VANILLA_SERVER names the vanilla server to join, as <host>:<port>")
        .parse()
        .expect("MCRS_VANILLA_SERVER is not a <host>:<port> socket address");

    let mut client = joining_client(address, 6);

    let Some(connection) = drive_client_until_joined(&mut client, true) else {
        panic!("the client never reached the play state within {JOIN_TIMEOUT:?}");
    };

    let world = client.world();
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

    println!(
        "columns stored: {}",
        client.world().resource::<ColumnStore>().len()
    );
}
