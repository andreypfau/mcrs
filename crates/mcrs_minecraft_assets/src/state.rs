use bevy_asset::{AssetServer, UntypedHandle};
use bevy_ecs::prelude::Resource;
use bevy_state::prelude::States;

/// Top-level application lifecycle states.
///
/// The state machine progresses linearly during startup:
///
/// ```text
/// Bootstrap → LoadingDataPack → WorldgenFreeze → Playing
/// ```
///
/// - **Bootstrap**: Static registries (blocks, items) are populated.
/// - **LoadingDataPack**: Bevy asset loaders are running; worldgen JSON assets
///   are being loaded (biomes, density functions, noise settings, etc.).
/// - **WorldgenFreeze**: All worldgen assets are ready.  Tags are resolved,
///   the registries are registered for the network, `NoiseRouter` is
///   compiled.  No further data-pack changes until next reconfiguration.
/// - **Playing**: Normal server operation.
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
