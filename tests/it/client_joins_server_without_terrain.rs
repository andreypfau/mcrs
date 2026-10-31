use bevy_app::{App, PluginGroup};
use mcrs_minecraft_light_plugin::LightingPlugin;
use mcrs_minecraft_server::{BoundAddress, MinecraftServerPlugins, WorldGenPlugin};

use crate::support::{JOIN_TIMEOUT, drive_client_until_joined, joining_client, start_server};

#[test]
fn a_client_joins_a_server_without_world_generation_and_lighting() {
    let mut server = App::new();
    server.add_plugins(
        MinecraftServerPlugins::embedded()
            .build()
            .disable::<WorldGenPlugin>()
            .disable::<LightingPlugin>(),
    );
    let address = server.world().resource::<BoundAddress>().0;
    let stop_server = start_server(server);

    let mut client = joining_client(address, 2);
    let outcome = drive_client_until_joined(&mut client, false);
    stop_server();

    assert!(
        outcome.is_some(),
        "the client never reached the play state within {JOIN_TIMEOUT:?}"
    );
}
