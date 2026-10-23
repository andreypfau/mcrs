use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::io::AssetSourceId;
use bevy_asset::{AssetApp, AssetPlugin};
use bevy_ecs::entity::Entity;
use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_client::columns::{BlockSource, ColumnStore};
use mcrs_minecraft_client::player::Player;
use mcrs_minecraft_client::registries::ClientRegistriesPlugin;
use mcrs_minecraft_core::{Id, RegistryValue};
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::{
    ChunkCacheCenter, ChunkCacheRadius, CurrentDimension, JoinedGame, PendingTeleports,
};
use mcrs_minecraft_registries::CurrentRegistries;
use std::time::{Duration, Instant};

pub const JOIN_TIMEOUT: Duration = Duration::from_secs(120);

/// The client's registry plugin over the repository's assets. It loads the block states a column
/// is decoded with, reads the server's registries at the end of configuration and holds the
/// vanilla pack to fill the entries the server leaves out.
pub fn add_client_registries(client: &mut App) {
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

pub fn insert_player(client: &mut App) {
    client.world_mut().spawn(Player);
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
