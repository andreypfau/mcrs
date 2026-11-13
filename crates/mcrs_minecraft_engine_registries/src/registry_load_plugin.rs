use crate::asset_reader_packs::AssetReaderPacks;
use crate::item_definitions::load_item_definitions;
use crate::loading_state::{check_registry_assets_ready, start_loading_data_pack};
use crate::log_diagnostics::log_diagnostics;
use crate::registry_names::RegistryNames;
use crate::registry_snapshot::install_registries;
use bevy_app::{App, AppExit, Plugin, PostStartup, Update};
use bevy_asset::AssetServer;
use bevy_asset::io::AssetSourceId;
use bevy_ecs::prelude::*;
use bevy_state::prelude::*;
use mcrs_minecraft_assets::packs::PACK_FILES;
use mcrs_minecraft_assets::{AppState, LoadedRegistryAssets};
use mcrs_minecraft_game_registries::registry_load::load_registries;
use mcrs_minecraft_item_table::DefinitionFile;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_item_table::bake;
use mcrs_minecraft_registries::NetworkSnapshot;
use mcrs_minecraft_registries::Registries;

/// What the configuration phase sends of the installed registries, encoded once per load.
#[derive(Resource)]
pub struct SyncedRegistries(pub NetworkSnapshot);

/// Loads the registries, the block and item definitions and the block sound sets when the app
/// finishes, and walks the app through loading the data pack to playing.
pub struct RegistryLoadPlugin;

impl Plugin for RegistryLoadPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(mcrs_minecraft_environment::WorldClockPlugin);
        app.init_resource::<LoadedRegistryAssets>();

        app.add_systems(PostStartup, start_loading_data_pack)
            .add_systems(
                Update,
                check_registry_assets_ready.run_if(in_state(AppState::LoadingDataPack)),
            )
            .add_systems(OnEnter(AppState::WorldgenFreeze), transition_to_playing);
    }

    fn finish(&self, app: &mut App) {
        if let Err(error) = install(app.world_mut()) {
            tracing::error!("the registries are unusable:\n{error}");
            app.world_mut().write_message(AppExit::error());
        }
    }
}

/// Loads the registries, the block and item definitions and the block sound sets into `world`.
fn install(world: &mut World) -> Result<(), String> {
    let asset_server = world
        .get_resource::<AssetServer>()
        .cloned()
        .ok_or_else(|| "the app has no asset server".to_owned())?;
    let pack_files = asset_server
        .get_source(PACK_FILES)
        .map_err(|error| format!("asset source {PACK_FILES}: {error}"))?
        .reader();
    let path = std::path::Path::new("minecraft/version.json");
    bevy_tasks::block_on(mcrs_minecraft_assets::asset::read_whole(pack_files, path))
        .map_err(|error| error.to_string())
        .and_then(|bytes| mcrs_minecraft_core::check_corpus_version(&bytes))
        .map_err(|error| format!("{}: {error}", path.display()))?;

    let additions = world
        .remove_resource::<mcrs_minecraft_registries::RegistryAdditions>()
        .unwrap_or_default();
    let started = bevy_platform::time::Instant::now();
    let (registries, diagnostics) = load_registries(&AssetReaderPacks(pack_files), additions)
        .map_err(|error| error.to_string())?;
    log_diagnostics(&diagnostics);
    tracing::info!(elapsed = ?started.elapsed(), "loaded registries");
    let synced = NetworkSnapshot::new(&registries).map_err(|error| error.to_string())?;

    let (definitions, report) =
        mcrs_minecraft_block::definition::load_block_definitions(&asset_server)
            .map_err(|error| error.to_string())?;
    tracing::info!(
        blocks = definitions.blocks().len(),
        states = report.states,
        permutations = report.permutations,
        shapes = report.shapes,
        bytes = report.table_bytes,
        elapsed = ?report.elapsed,
        "loaded block definitions"
    );
    let sound_set_registry = registries
        .registry::<mcrs_minecraft_sound::BlockSoundSet>()
        .ok_or_else(|| "the data pack registries hold no minecraft:block_sound_set".to_owned())?;
    let sound_sets =
        mcrs_minecraft_block::definition::BlockSoundSets::resolve(&definitions, |key| {
            let id = sound_set_registry.by_name(key.location())?;
            mcrs_minecraft_resource::Id::checked(id.number(), sound_set_registry.len())
        })
        .map_err(|key| format!("the block sound set {key} is not loaded"))?;
    let definitions = std::sync::Arc::new(definitions);
    let items = load_item_definitions(&asset_server, &registries, &definitions)
        .map_err(|error| error.to_string())?;
    tracing::info!(items = items.len(), "loaded item definitions");
    let table = bake_item_table(&asset_server, &registries)?;
    world.insert_resource(table);
    world.insert_resource(sound_sets);
    world.insert_resource(SyncedRegistries(synced));
    install_registries(
        world,
        registries,
        mcrs_minecraft_block::definition::Blocks(definitions),
        mcrs_minecraft_item::Items(std::sync::Arc::new(items)),
    );
    Ok(())
}

/// The item table of the stacks as entities, from the item definitions of the default asset
/// source and the names of `registries`.
fn bake_item_table(
    asset_server: &AssetServer,
    registries: &Registries,
) -> Result<ItemTable, String> {
    let source = asset_server
        .get_source(AssetSourceId::Default)
        .map_err(|error| format!("the default asset source: {error}"))?;
    let files = mcrs_minecraft_assets::asset::read_json_corpus(
        source.reader(),
        mcrs_minecraft_item::definition::CORPUS_DIRECTORY,
    )
    .map_err(|error| error.to_string())?;
    let baked = bake(
        files
            .iter()
            .map(|(name, bytes)| DefinitionFile { name, bytes }),
        &RegistryNames::new(registries),
    )
    .map_err(|error| format!("the item table: {error}"))?;
    let skipped: usize = baked.skipped.iter().map(|skipped| skipped.prototypes).sum();
    tracing::info!(
        types = baked.skipped.len(),
        components = skipped,
        "baked the item table without the component types it has no type for"
    );
    Ok(baked.table)
}

/// Enters `AppState::Playing`.
pub fn transition_to_playing(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::Playing);
    tracing::info!("entering Playing state");
}
