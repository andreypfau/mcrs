pub mod block_state;
pub mod registry;
pub mod resource_location;
pub mod state;
pub mod tag;

pub use registry::{RegistrySnapshot, ResourceKey, SnapshotEntry, StaticId, StaticRegistry};
pub use resource_location::ResourceLocation;
pub use state::AppState;
pub use tag::{DynRegistryIndex, DynTagRegistry, IdBitSet, RawBitSet, TagEntry, TagFile, TagFileLoader, TagFileSettings, TagKey, TagRef, TagRegistry, TaggedRegistry};

// Re-export the proc macro for the rl! declarative macro.
#[doc(hidden)]
pub use mcrs_core_macros::rl_impl as __rl_impl;

use bevy_app::{App, Plugin};
use bevy_asset::AssetApp;
use bevy_state::app::{AppExtStates, StatesPlugin};

/// Foundation plugin — registers the `AppState` state machine, the `TagFile`
/// asset type, and the `TagFileLoader`.
///
/// All other `mc_*` plugins depend on this one.
pub struct MinecraftEnginePlugin;

impl Plugin for MinecraftEnginePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<StatesPlugin>() {
            app.add_plugins(StatesPlugin);
        }
        app.init_state::<AppState>();
        app.init_asset::<TagFile>();
        app.register_asset_loader(TagFileLoader);
    }
}
