use bevy_app::{App, AppExit, TaskPoolPlugin, Update};
use bevy_asset::io::AssetSourceId;
use bevy_asset::{AssetApp, AssetPlugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageWriter;
use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_client::columns::{BlockSource, ColumnCachePlugin, ColumnStore};
use mcrs_minecraft_client::player::Player;
use mcrs_minecraft_client::registries::ClientRegistriesPlugin;
use mcrs_minecraft_core::{Id, RegistryValue};
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::{
    ChunkCacheCenter, ChunkCacheRadius, ClientNetworkPlugin, CurrentDimension, JoinedGame,
    PendingTeleports,
};
use mcrs_minecraft_registries::CurrentRegistries;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub const JOIN_TIMEOUT: Duration = Duration::from_secs(120);

/// Runs `server` on its own thread. The returned closure asks it to exit and waits until it has.
pub fn start_server(mut server: App) -> impl FnOnce() {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = stop.clone();
    server.add_systems(Update, move |mut exit: MessageWriter<AppExit>| {
        if stop_flag.load(Ordering::Relaxed) {
            exit.write(AppExit::Success);
        }
    });
    let (finished_send, finished_recv) = mpsc::channel();
    let server_thread = mcrs_minecraft_server::spawn_server_thread(server, move |app| {
        mcrs_minecraft_server::run_server_loop(app);
        finished_send.send(()).ok();
    });
    move || {
        stop.store(true, Ordering::Relaxed);
        finished_recv
            .recv_timeout(Duration::from_secs(30))
            .expect("the embedded server did not stop after AppExit");
        server_thread.join().expect("server thread joined");
    }
}

/// A client as `mcrs_test`, with a player and a column cache, ready to connect to `server`.
pub fn joining_client(server: SocketAddr, view_distance: u8) -> App {
    let mut client = App::new();
    client.add_plugins(ClientNetworkPlugin {
        server,
        username: "mcrs_test".to_owned(),
        profile_id: None,
        view_distance,
    });
    client.add_plugins(ColumnCachePlugin);
    add_client_registries(&mut client);
    client.world_mut().spawn(Player);
    client.finish();
    client.cleanup();
    client
}

/// The client's registry plugin over the repository's assets. It loads the block states a column
/// is decoded with, reads the server's registries at the end of configuration and holds the
/// vanilla pack to fill the entries the server leaves out.
fn add_client_registries(client: &mut App) {
    let assets = concat!(env!("CARGO_MANIFEST_DIR"), "/assets").to_owned();
    client
        .add_plugins(TaskPoolPlugin::default())
        .register_asset_source(
            AssetSourceId::Default,
            mcrs_minecraft_client::asset_source(&mcrs_minecraft_assets::packs::PackRoots {
                assets: assets.clone(),
                vanilla: mcrs_minecraft_fixtures::vanilla_dir().to_owned(),
            }),
        )
        .add_plugins(AssetPlugin {
            file_path: assets,
            watch_for_changes_override: Some(false),
            ..Default::default()
        })
        .add_plugins(ClientRegistriesPlugin {
            local_corpus: Some(mcrs_minecraft_world::registry_load::test_packs()),
        });
}

/// Returns the connection entity once every play-state packet the flow promises
/// has arrived, or `None` if the deadline passes first. A server that generates no
/// terrain sends no column, so `awaits_columns` is clear for it.
pub fn drive_client_until_joined(
    client: &mut App,
    awaits_columns: bool,
) -> Option<bevy_ecs::entity::Entity> {
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
        let chunks = !awaits_columns || !world.resource::<ColumnStore>().is_empty();
        if joined && positioned && centred && radius && chunks {
            return Some(entity);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}

pub fn assert_session_registries(client: &App, connection: Entity) {
    let world = client.world();
    let registries = world.resource::<CurrentRegistries>();

    let biomes = registries
        .registry::<BiomeFile>()
        .expect("the session holds the biome registry");
    assert!(!biomes.is_empty(), "the biome registry arrived empty");
    assert!(
        biomes
            .values()
            .iter()
            .any(|biome| biome.effects.water_color.is_some()),
        "no biome carries a water colour"
    );
    for (registry, tagged) in [
        (
            "minecraft:block",
            registries.tags::<Block>().map(|tags| tags.iter().count()),
        ),
        (
            "minecraft:worldgen/biome",
            registries
                .tags::<<BiomeFile as RegistryValue>::Registry>()
                .map(|tags| tags.iter().count()),
        ),
    ] {
        assert!(
            tagged.is_some_and(|count| count > 0),
            "the tags of {registry} arrived empty"
        );
    }

    let current = world
        .get::<CurrentDimension>(connection)
        .expect("the login named the dimension the player is in");
    let dimension_types = registries
        .registry::<DimensionTypeFile>()
        .expect("the session holds the dimension types");
    let dimension_type = Id::checked(current.dimension_type.number(), dimension_types.len())
        .and_then(|id| dimension_types.get(id))
        .expect("the login names a dimension type of the session");
    let extent = world
        .resource::<ColumnStore>()
        .extent()
        .expect("the store holds the extent of the dimension");
    assert_eq!(
        u32::try_from(extent.sections).unwrap() * 16,
        dimension_type.height,
        "the column height is the dimension type's"
    );
    assert_eq!(
        extent.min_section_y * 16,
        dimension_type.min_y,
        "the column floor is the dimension type's"
    );
}
