use crate::LoadedRegistryAssets;
use bevy_asset::io::AssetSourceId;
use bevy_asset::{Asset, AssetServer};
use bevy_ecs::prelude::*;
use bevy_state::prelude::*;
use bevy_tasks::futures_lite::StreamExt;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_registry::RegistrySet;

pub(crate) fn start_loading_data_pack(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::LoadingDataPack);
}

// File listings baked from `assets/` at build time. Used as the fallback
// manifest when the active `AssetSource` cannot enumerate directories
// (HTTP/WASM, embedded packs without an index, etc.).
pub(crate) mod registry_files {
    include!(concat!(env!("OUT_DIR"), "/registry_files.rs"));
}

/// Resolve the set of `<folder>/<file>.<extension>` paths that should be loaded
/// for a registry folder.
///
/// First tries `AssetReader::read_directory` on the default `AssetSource` —
/// this picks up files that the active source can enumerate, including
/// future resource packs mounted as file-system folders or ZIPs.
/// Falls back to the build-time manifest baked from the vanilla `assets/`
/// tree for sources that cannot list directories (HTTP/WASM). Either listing
/// names files only, so the entries the code builds are added to it.
fn list_registry_files(
    asset_server: &AssetServer,
    folder: &str,
    extension: &str,
    fallback: &'static [&'static str],
) -> Vec<String> {
    let dynamic: Vec<String> = match asset_server.get_source(AssetSourceId::Default) {
        Ok(source) => {
            let reader = source.reader();
            bevy_tasks::block_on(walk_files(reader, std::path::PathBuf::from(folder)))
                .into_iter()
                .filter(|p| p.extension().and_then(|s| s.to_str()) == Some(extension))
                .filter_map(|p| p.to_str().map(str::to_owned))
                .collect()
        }
        Err(err) => {
            tracing::warn!(folder, %err, "default AssetSource missing");
            Vec::new()
        }
    };

    let mut files: std::collections::BTreeSet<String> = if dynamic.is_empty() {
        fallback.iter().map(|s| (*s).to_owned()).collect()
    } else {
        dynamic.into_iter().collect()
    };
    files.extend(mcrs_minecraft_worldgen_builtin::paths(folder));
    files.into_iter().collect()
}

fn request_registry<T: Asset>(
    asset_server: &AssetServer,
    set: &RegistrySet,
    loaded: &mut LoadedRegistryAssets,
    registry: &str,
    extension: &str,
) {
    let table = set
        .table(registry)
        .unwrap_or_else(|| panic!("{registry} is not a loaded registry"));
    let directory = table.registry().path();
    for name in table.names() {
        let path = format!(
            "{}/{directory}/{}.{extension}",
            name.namespace(),
            name.path()
        );
        loaded.handles.push(asset_server.load::<T>(path).untyped());
    }
    tracing::info!(
        registry,
        count = table.len(),
        kind = std::any::type_name::<T>(),
        "requested registry assets"
    );
}

fn request_templates(asset_server: &AssetServer, loaded: &mut LoadedRegistryAssets) {
    use registry_files::{FILES_TEMPLATE, FOLDER_TEMPLATE};
    let files = list_registry_files(asset_server, FOLDER_TEMPLATE, "nbt", FILES_TEMPLATE);
    let count = files.len();
    for path in files {
        loaded.handles.push(
            asset_server
                .load::<mcrs_minecraft_worldgen::bevy::TemplateAsset>(path)
                .untyped(),
        );
    }
    tracing::info!(
        folder = FOLDER_TEMPLATE,
        count,
        "requested structure templates"
    );
}

pub(crate) fn request_data_pack_assets(
    asset_server: Res<AssetServer>,
    set: Res<RegistrySet>,
    mut loaded: ResMut<LoadedRegistryAssets>,
) {
    request_registry::<mcrs_minecraft_worldgen::bevy::CarverConfigAsset>(
        &asset_server,
        &set,
        &mut loaded,
        "minecraft:worldgen/carver",
        "json",
    );
    request_registry::<mcrs_minecraft_worldgen::bevy::FeatureAsset>(
        &asset_server,
        &set,
        &mut loaded,
        "minecraft:worldgen/feature",
        "json",
    );
    request_registry::<mcrs_minecraft_worldgen::bevy::PlacedFeatureAsset>(
        &asset_server,
        &set,
        &mut loaded,
        "minecraft:worldgen/placed_feature",
        "json",
    );
    request_registry::<mcrs_minecraft_worldgen::bevy::StructureSetAsset>(
        &asset_server,
        &set,
        &mut loaded,
        "minecraft:worldgen/structure_set",
        "json",
    );
    request_registry::<mcrs_minecraft_worldgen::bevy::StructureAsset>(
        &asset_server,
        &set,
        &mut loaded,
        "minecraft:worldgen/structure",
        "json",
    );
    request_registry::<mcrs_minecraft_worldgen::bevy::TemplatePoolAsset>(
        &asset_server,
        &set,
        &mut loaded,
        "minecraft:worldgen/template_pool",
        "json",
    );
    request_templates(&asset_server, &mut loaded);
}

pub(crate) async fn walk_files(
    reader: &dyn bevy_asset::io::ErasedAssetReader,
    root: std::path::PathBuf,
) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root];
    while let Some(directory) = stack.pop() {
        let Ok(mut entries) = reader.read_directory(&directory).await else {
            continue;
        };
        while let Some(path) = entries.next().await {
            if reader.is_directory(&path).await.unwrap_or(false) {
                stack.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files
}

pub(crate) fn check_registry_assets_ready(
    registry_assets: Res<LoadedRegistryAssets>,
    asset_server: Res<AssetServer>,
    mut next: ResMut<NextState<AppState>>,
) {
    if registry_assets.all_handles_settled(&asset_server) {
        tracing::info!("all registry assets settled — entering WorldgenFreeze");
        next.set(AppState::WorldgenFreeze);
    }
}
