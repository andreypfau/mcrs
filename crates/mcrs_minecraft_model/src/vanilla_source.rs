use std::path::Path;

use bevy::app::App;
use bevy::asset::AssetApp;
use bevy::asset::io::memory::{Dir, MemoryAssetReader};
use bevy::asset::io::{AssetSourceBuilder, AssetSourceId};
use mcrs_minecraft_client_jar::Files;

pub const SOURCE: &str = "vanilla";

pub fn register_source(app: &mut App, root: Dir) {
    app.register_asset_source(
        AssetSourceId::from(SOURCE),
        AssetSourceBuilder::new(move || Box::new(MemoryAssetReader { root: root.clone() })),
    );
}

pub fn fill(root: &Dir, files: Files) {
    for (path, bytes) in files {
        root.insert_asset(Path::new(&path), bytes);
    }
}

#[cfg(any(test, feature = "test-support"))]
#[expect(
    clippy::unwrap_used,
    reason = "a test fixture with no caller to report to"
)]
pub fn resource_files() -> &'static Files {
    use mcrs_minecraft_client_jar::Progress;
    static FILES: std::sync::LazyLock<Files> = std::sync::LazyLock::new(|| {
        let progress = Progress::default();
        let mut files =
            mcrs_minecraft_client_jar::resolve(&progress, crate::resource_set::is_resource, drop)
                .unwrap()
                .0;
        let layer = mcrs_minecraft_client_jar::index_assets(mcrs_minecraft_client_jar::OBJECTS);
        assert!(layer.failures.is_empty(), "{:?}", layer.failures);
        let layer = layer.files;
        files.retain(|(name, _)| layer.iter().all(|(above, _)| above != name));
        files.extend(layer);
        files
    });
    &FILES
}
