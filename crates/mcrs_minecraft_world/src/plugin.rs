use crate::data_pack::{
    check_registry_assets_ready, request_data_pack_assets, start_loading_data_pack,
};
use crate::registries;
use crate::registry_load::{self, AssetReaderPacks, RegistryLoadError};
use bevy_app::{App, Plugin, PostStartup, Update};
use bevy_asset::{AssetServer, UntypedHandle};
use bevy_ecs::prelude::*;
use bevy_state::prelude::*;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_assets::packs::PACK_FILES;
use mcrs_minecraft_block::keys::Block;

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
        let asset_server = app.world().resource::<AssetServer>().clone();
        let pack_files = asset_server
            .get_source(PACK_FILES)
            .unwrap_or_else(|error| {
                registries::refuse(&RegistryLoadError::Read(format!(
                    "asset source {PACK_FILES}: {error}"
                )))
            })
            .reader();
        {
            let path = std::path::Path::new("minecraft/version.json");
            let bytes =
                bevy_tasks::block_on(mcrs_minecraft_assets::asset::read_whole(pack_files, path))
                    .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            mcrs_minecraft_core::check_corpus_version(&bytes)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        }
        let (block_registry, registries, registries_arc) = {
            let started = bevy_platform::time::Instant::now();
            let loaded = registry_load::load_registries(&AssetReaderPacks(pack_files))
                .unwrap_or_else(|error| registries::refuse(&error));
            registry_load::log_diagnostics(&loaded.diagnostics);
            let registries = loaded.set;
            tracing::info!(
                statics = registries.statics().count(),
                registries = registries.tables().count(),
                entries = registries.tables().map(|table| table.len()).sum::<usize>(),
                elapsed = ?started.elapsed(),
                "loaded registries"
            );
            let block_registry = registries
                .registry::<Block>()
                .expect("the static registries hold minecraft:block");
            app.insert_resource(registries.clone());
            (block_registry, registries, loaded.registries)
        };
        {
            let asset_server = app.world().resource::<AssetServer>().clone();
            let (definitions, report) = mcrs_minecraft_block::definition::load_block_definitions(
                &asset_server,
                &block_registry,
            )
            .expect("the block definition corpus loads");
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
                .expect("the data pack registries hold minecraft:block_sound_set");
            let sound_sets = mcrs_minecraft_block::definition::BlockSoundSets::resolve(
                &definitions,
                &sound_set_registry,
            )
            .unwrap_or_else(|error| registries::refuse(&error));
            let definitions = std::sync::Arc::new(definitions);
            let items = crate::item::definitions::load_item_definitions(
                &asset_server,
                &registries,
                &definitions,
            )
            .expect("the item definition corpus loads");
            tracing::info!(items = items.len(), "loaded item definitions");
            app.insert_resource(mcrs_minecraft_item::Items(std::sync::Arc::new(items)));
            app.insert_resource(mcrs_minecraft_block::definition::Blocks(definitions));
            app.insert_resource(sound_sets);
        }
        crate::registry_snapshot::install_registries(app.world_mut(), registries_arc);
    }
}

pub fn transition_to_playing(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::Playing);
    tracing::info!("entering Playing state");
}
