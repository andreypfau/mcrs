use bevy_app::App;
use mcrs_minecraft_client::columns::ColumnStore;
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::{
    ChunkCacheCenter, ChunkCacheRadius, JoinedGame, PendingTeleports,
};
use std::time::{Duration, Instant};

pub const JOIN_TIMEOUT: Duration = Duration::from_secs(120);

/// A column cannot be decoded without the number of block states: it fixes the
/// width a section's states are packed at once the section drops its palette.
pub fn insert_block_catalog(client: &mut App) {
    client.insert_resource(mcrs_minecraft_worldgen_generator::tests::blocks().clone());
}

/// What the client reads the server's registries with once configuration ends, the vanilla
/// pack's entries included, as the client's registry plugin holds them.
pub fn insert_session_inputs(client: &mut App) {
    client.insert_resource(mcrs_minecraft_client::registries::session_inputs(Some(
        &mcrs_minecraft_client::asset_corpus(),
    )));
}

/// Returns the connection entity once every play-state packet the flow promises
/// has arrived, or `None` if the deadline passes first.
pub fn drive_client_until_joined(client: &mut App) -> Option<bevy_ecs::entity::Entity> {
    let deadline = Instant::now() + JOIN_TIMEOUT;
    let mut seen_configuration = false;

    while Instant::now() < deadline {
        client.update();

        let world = client.world_mut();
        let mut connections = world.query::<(bevy_ecs::entity::Entity, &ConnectionState)>();
        let Some((entity, state)) = connections.iter(world).next() else {
            std::thread::sleep(Duration::from_millis(5));
            continue;
        };
        seen_configuration |= *state == ConnectionState::Configuration;
        if *state != ConnectionState::Game {
            std::thread::sleep(Duration::from_millis(5));
            continue;
        }

        assert!(
            seen_configuration,
            "the connection reached play without passing through configuration"
        );
        let joined = world.get::<JoinedGame>(entity).is_some();
        let positioned = world
            .get::<PendingTeleports>(entity)
            .is_some_and(|teleports| !teleports.0.is_empty());
        let centred = world.get::<ChunkCacheCenter>(entity).is_some();
        let radius = world.get::<ChunkCacheRadius>(entity).is_some();
        let chunks = !world.resource::<ColumnStore>().is_empty();
        if joined && positioned && centred && radius && chunks {
            return Some(entity);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}
