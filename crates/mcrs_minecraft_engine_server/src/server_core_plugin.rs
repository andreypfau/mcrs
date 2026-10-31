use bevy_app::{App, AppExit, Plugin};
use mcrs_minecraft_assets::packs::{BuiltinAsset, PackLayering, PackRoots};
use mcrs_minecraft_engine_world::server_loop::VoxelServerPlugin;
use mcrs_minecraft_registries::RegistryAdditions;

use crate::data_pack::DataPack;
use crate::runner::DEFAULT_TPS;

/// The tick, the task pools, the asset server, and the registries with the block and item
/// definitions.
pub struct ServerCorePlugin {
    /// Clear when the server shares a process with another Bevy app, which
    /// then owns the process-global task pools.
    pub owns_task_pools: bool,
    /// Absolute path to the asset corpus, for a server that cannot resolve
    /// `assets` from its own working directory or executable location.
    pub asset_path: Option<String>,
    /// Where the game's data pack is read from.
    pub data_pack: DataPack,
    /// The assets built in code, layered under the packs.
    pub builtin_assets: BuiltinAsset,
    /// The local packs of the pack root that the registries load and the assets layer, by
    /// directory name.
    pub local_packs: Vec<String>,
}

impl Default for ServerCorePlugin {
    fn default() -> Self {
        Self {
            owns_task_pools: true,
            asset_path: None,
            data_pack: DataPack::default(),
            builtin_assets: |_| None,
            local_packs: vec!["mcrs".to_owned()],
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
        app.add_plugins(VoxelServerPlugin {
            tick_rate: DEFAULT_TPS,
            owns_task_pools: self.owns_task_pools,
            asset_path: self.asset_path.clone(),
        });
        app.add_plugins(mcrs_minecraft_assets::MinecraftCorePlugin);
        app.add_plugins(mcrs_minecraft_world::MinecraftWorldPlugin);
    }
}
