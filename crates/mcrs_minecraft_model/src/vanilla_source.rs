use std::path::Path;

use bevy::app::App;
use bevy::asset::AssetApp;
use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSourceBuilder, AssetSourceId};
use mcrs_minecraft_client_jar::Files;

/// The asset source holding the resource pack of the vanilla client jar.
pub const SOURCE: &str = "vanilla";

/// Registers `root` as the `vanilla` source. Must run before `AssetPlugin` builds.
pub fn register_source(app: &mut App, root: Dir) {
    app.register_asset_source(
        AssetSourceId::from(SOURCE),
        AssetSourceBuilder::new(move || Box::new(MemoryAssetReader { root: root.clone() })),
    );
}

// chisle: the resource half is held twice, here and again in `Pack`; have `Pack` hold the
// `Dir`'s `Arc<Vec<u8>>` values instead if the memory matters.
pub fn fill(root: &Dir, files: Files) {
    for (path, bytes) in files {
        root.insert_asset(Path::new(&path), bytes);
    }
}

/// The resource half of the installed client jar, read once per process.
///
/// # Panics
///
/// When the client jar can not be installed or read.
#[cfg(any(test, feature = "test-support"))]
#[expect(
    clippy::unwrap_used,
    reason = "a test fixture with no caller to report to"
)]
pub fn resource_files() -> &'static Files {
    use mcrs_minecraft_client_jar::Progress;
    static FILES: std::sync::LazyLock<Files> = std::sync::LazyLock::new(|| {
        mcrs_minecraft_client_jar::resolve(&Progress::default(), crate::model::is_resource, drop)
            .unwrap()
            .0
    });
    &FILES
}
