use crate::data_pack::{
    check_registry_assets_ready, request_data_pack_assets, start_loading_data_pack,
};
use crate::{registries, resolvers};
use bevy_app::{App, Plugin, PostStartup, Update};
use bevy_asset::{AssetServer, UntypedHandle};
use bevy_ecs::prelude::*;
use bevy_state::prelude::*;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_worldgen::tables::build_worldgen_tables;

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

        registries::share_registries(app.world_mut());

        app.add_systems(PostStartup, start_loading_data_pack)
            .add_systems(OnEnter(AppState::LoadingDataPack), request_data_pack_assets)
            .add_systems(
                Update,
                check_registry_assets_ready.run_if(in_state(AppState::LoadingDataPack)),
            )
            .add_systems(
                OnEnter(AppState::WorldgenFreeze),
                (build_worldgen_tables, transition_to_playing),
            );
    }

    fn finish(&self, app: &mut App) {
        {
            let asset_server = app.world().resource::<AssetServer>().clone();
            let source = asset_server
                .get_source(bevy_asset::io::AssetSourceId::Default)
                .expect("default AssetSource missing");
            let path = std::path::Path::new("minecraft/version.json");
            let bytes = bevy_tasks::block_on(mcrs_minecraft_assets::asset::read_whole(
                source.reader(),
                path,
            ))
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            mcrs_minecraft_core::check_corpus_version(&bytes)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        }
        let (block_registry, registries) = {
            let asset_server = app.world().resource::<AssetServer>().clone();
            let statics = registries::static_registries()
                .unwrap_or_else(|report| registries::refuse(&report));
            tracing::info!(
                count = statics.tables().count(),
                "built the static registries"
            );
            let started = bevy_platform::time::Instant::now();
            let registries = registries::load_registries(&asset_server, statics)
                .unwrap_or_else(|report| registries::refuse(&report));
            tracing::info!(
                registries = registries.tables().count(),
                entries = registries.tables().map(|table| table.len()).sum::<usize>(),
                elapsed = ?started.elapsed(),
                "loaded registries"
            );
            resolvers::run_resolvers(app.world_mut(), &registries)
                .unwrap_or_else(|report| registries::refuse(&report));
            let block_registry = registries
                .registry::<Block>()
                .expect("the static registries hold minecraft:block");
            registries::insert_registry_resources(app.world_mut(), &registries);
            (block_registry, registries)
        };
        mcrs_minecraft_worldgen::bevy::register_worldgen_loaders(app, &registries);
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
        }
    }
}

pub fn transition_to_playing(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::Playing);
    tracing::info!("entering Playing state");
}
