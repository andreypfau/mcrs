pub mod registry;
pub mod resource_location;
pub mod state;
pub mod tag;

pub use registry::{RegistrySnapshot, StaticId, StaticRegistry};
pub use resource_location::ResourceLocation;
pub use state::AppState;
pub use tag::{TagEntry, TagFile, TagFileLoader, TagFileSettings, TagKey, TagRegistryType};

use bevy_app::{App, Plugin};
use bevy_asset::AssetApp;
use bevy_state::app::AppExtStates;

/// Foundation plugin — registers the `AppState` state machine, the `TagFile`
/// asset type, and the `TagFileLoader`.
///
/// All other `mc_*` plugins depend on this one.
pub struct MinecraftEnginePlugin;

impl Plugin for MinecraftEnginePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>();
        app.init_asset::<TagFile>();
        app.register_asset_loader(TagFileLoader);
        app.init_resource::<RegistrySnapshot>();
    }
}
