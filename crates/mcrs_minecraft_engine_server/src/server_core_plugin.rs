use bevy_app::{App, AppExit, Plugin};
use mcrs_engine_link::Placement;
use mcrs_minecraft_assets::packs::{BuiltinAsset, PackLayering, PackRoots};
use mcrs_minecraft_engine::muxer::muxer_placement::MuxerPlacement;
use mcrs_minecraft_engine_world::server_loop::VoxelServerPlugin;
use mcrs_minecraft_engine_world::world_cells::WorldCells;
use mcrs_minecraft_registries::RegistryAdditions;
use std::num::NonZeroUsize;

use crate::data_pack::DataPack;
use crate::runner::DEFAULT_TPS;

pub struct ServerCorePlugin {
    pub owns_task_pools: bool,
    pub asset_path: Option<String>,
    pub data_pack: DataPack,
    pub builtin_assets: BuiltinAsset,
    pub local_packs: Vec<String>,
    pub worlds: Placement,
    pub cells: WorldCells,
    pub muxer: Placement,
    pub compute_threads: Option<NonZeroUsize>,
}

impl Default for ServerCorePlugin {
    fn default() -> Self {
        Self {
            owns_task_pools: true,
            asset_path: None,
            data_pack: DataPack::default(),
            builtin_assets: |_| None,
            local_packs: vec!["mcrs".to_owned()],
            worlds: Placement::Thread,
            cells: WorldCells::default(),
            muxer: Placement::Thread,
            compute_threads: None,
        }
    }
}

impl Plugin for ServerCorePlugin {
    fn build(&self, app: &mut App) {
        let vanilla = match self.data_pack.resolve() {
            Ok(vanilla) => vanilla,
            Err(error) => {
                tracing::error!("the registries are unusable:\n{error}");
                app.world_mut().write_message(AppExit::error());
                return;
            }
        };
        let roots = PackRoots {
            assets: self
                .asset_path
                .clone()
                .unwrap_or_else(|| bevy_asset::AssetPlugin::default().file_path),
            vanilla,
        };
        mcrs_minecraft_assets::packs::register_pack_sources(
            app,
            &roots,
            PackLayering {
                local_packs: self.local_packs.clone(),
                builtin: self.builtin_assets,
            },
        );
        app.world_mut()
            .get_resource_or_init::<RegistryAdditions>()
            .local_packs
            .extend(self.local_packs.iter().cloned());
        app.insert_resource(MuxerPlacement(self.muxer));
        app.insert_resource(self.cells);
        app.add_plugins(VoxelServerPlugin {
            tick_rate: DEFAULT_TPS,
            owns_task_pools: self.owns_task_pools,
            asset_path: self.asset_path.clone(),
            worlds: self.worlds,
            compute_threads: self.compute_threads,
        });
        app.add_plugins(mcrs_minecraft_assets::MinecraftCorePlugin);
        app.add_plugins(mcrs_minecraft_engine_registries::RegistryLoadPlugin);
    }
}
