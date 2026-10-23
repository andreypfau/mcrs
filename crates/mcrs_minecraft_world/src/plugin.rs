use crate::data_pack::{
    check_registry_assets_ready, request_data_pack_assets, start_loading_data_pack,
};
use crate::registry_load::{self, AssetReaderPacks};
use bevy_app::{App, AppExit, Plugin, PostStartup, Update};
use bevy_asset::{AssetServer, UntypedHandle};
use bevy_ecs::prelude::*;
use bevy_state::prelude::*;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_assets::packs::PACK_FILES;
use mcrs_minecraft_registries::NetworkSnapshot;

#[derive(Resource, Default)]
pub struct LoadedRegistryAssets {
    handles: Vec<UntypedHandle>,
}

impl LoadedRegistryAssets {
    pub fn push(&mut self, handle: UntypedHandle) {
        self.handles.push(handle);
    }

    /// True once every handle and everything it pulls in has either finished
    /// loading successfully or failed to load. A tag file's nested `#tag`
    /// references are dependencies of it, so waiting on the tag file alone
    /// resolves it against a half-loaded tree. Missing or malformed files do not
    /// stall the gate; they are logged once `WorldgenFreeze` proceeds.
    ///
    /// A recursive state turns `Failed` as soon as one dependency fails, while
    /// its siblings may still be in flight, so leaf assets (templates) are
    /// requested directly and gate on their own load state.
    pub fn all_handles_settled(&self, asset_server: &AssetServer) -> bool {
        use bevy_asset::RecursiveDependencyLoadState;
        self.handles.iter().all(|h| {
            matches!(
                asset_server.recursive_dependency_load_state(h.id()),
                RecursiveDependencyLoadState::Loaded | RecursiveDependencyLoadState::Failed(_)
            )
        })
    }
}

/// What the configuration phase sends of the installed registries, encoded once per load.
#[derive(Resource)]
pub struct SyncedRegistries(pub NetworkSnapshot);

pub struct MinecraftWorldPlugin;

impl Plugin for MinecraftWorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(mcrs_minecraft_environment::world_clock::WorldClockPlugin);
        app.add_plugins(mcrs_minecraft_worldgen::bevy::WorldgenAssetsPlugin);
        app.init_resource::<LoadedRegistryAssets>();

        app.add_systems(PostStartup, start_loading_data_pack)
            .add_systems(OnEnter(AppState::LoadingDataPack), request_data_pack_assets)
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

    let started = bevy_platform::time::Instant::now();
    let (registries, diagnostics) =
        registry_load::load_registries(&AssetReaderPacks(pack_files), Vec::new())
            .map_err(|error| error.to_string())?;
    registry_load::log_diagnostics(&diagnostics);
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
            mcrs_minecraft_core::Id::checked(id.number(), sound_set_registry.len())
        })
        .map_err(|key| format!("the block sound set {key} is not loaded"))?;
    let definitions = std::sync::Arc::new(definitions);
    let items =
        crate::item::definitions::load_item_definitions(&asset_server, &registries, &definitions)
            .map_err(|error| error.to_string())?;
    tracing::info!(items = items.len(), "loaded item definitions");
    world.insert_resource(sound_sets);
    world.insert_resource(SyncedRegistries(synced));
    crate::registry_snapshot::install_registries(
        world,
        registries,
        mcrs_minecraft_block::definition::Blocks(definitions),
        mcrs_minecraft_item::Items(std::sync::Arc::new(items)),
    );
    Ok(())
}

pub fn transition_to_playing(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::Playing);
    tracing::info!("entering Playing state");
}
