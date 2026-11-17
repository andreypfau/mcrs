use bevy_asset::{AssetServer, UntypedHandle};
use bevy_ecs::prelude::Resource;
use bevy_state::prelude::States;

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Bootstrap,
    LoadingDataPack,
    WorldgenFreeze,
    Playing,
}

#[derive(Resource, Default)]
pub struct LoadedRegistryAssets {
    handles: Vec<UntypedHandle>,
}

impl LoadedRegistryAssets {
    pub fn push(&mut self, handle: UntypedHandle) {
        self.handles.push(handle);
    }

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
