// std: bevy_ecs

mod asset_reader_packs;
mod item_definitions;
mod loading_state;
mod log_diagnostics;
mod registry_load_plugin;
mod registry_snapshot;
#[cfg(all(feature = "test-support", not(target_family = "wasm")))]
mod test_corpus;

pub use item_definitions::{ItemLoadError, load_item_definitions};
pub use registry_load_plugin::{RegistryLoadPlugin, SyncedRegistries, transition_to_playing};
#[cfg(feature = "test-support")]
pub use registry_snapshot::replace_registries;
pub use registry_snapshot::{RegistrySnapshot, extract_registries, install_registries};
#[cfg(all(feature = "test-support", not(target_family = "wasm")))]
pub use test_corpus::test_corpus;
