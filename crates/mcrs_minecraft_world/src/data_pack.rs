use crate::LoadedRegistryAssets;
use bevy_asset::io::AssetSourceId;
use bevy_asset::{Asset, AssetServer};
use bevy_ecs::prelude::*;
use bevy_state::prelude::*;
use bevy_tasks::futures_lite::StreamExt;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_assets::tag::file::{TagFile, TagFileSettings};
use mcrs_minecraft_assets::tag::{TagLoader, TagLoadersSettled};
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::tag_key::TagKey;
use mcrs_minecraft_dimension::dimension_type::DimensionType;
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_registry::EntrySet;
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_registry::TagId;

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
    keep_ordered_timeline_tags(&asset_server, &set, &mut loaded);
}

/// A dimension's timelines stack in the order their tag file lists them, which
/// the frozen tag bitsets do not keep, so the tag files stay loaded for
/// `build_dimension_environments` to read in order.
fn keep_ordered_timeline_tags(
    asset_server: &AssetServer,
    set: &RegistrySet,
    loaded: &mut LoadedRegistryAssets,
) {
    let Some(dimension_types) = set.column::<DimensionType>(keys::DimensionType::KEY.as_str())
    else {
        return;
    };
    for dimension_type in dimension_types {
        if let EntrySet::Tag(tag) = &dimension_type.timelines {
            let key = TagKey::<keys::Timeline, _>::from_location(tag.clone());
            let handle = asset_server
                .load_builder()
                .with_settings(|settings: &mut TagFileSettings| {
                    settings.registry_segment = keys::Timeline::KEY.path().to_string();
                })
                .load::<TagFile>(key.asset_path());
            loaded.handles.push(handle.untyped());
        }
    }
}

/// `(tag location, asset path)`, in asset path order.
pub fn list_tag_files(
    set: &RegistrySet,
    registry_path: &str,
) -> Vec<(ResourceLocation<std::sync::Arc<str>>, String)> {
    let Some(table) = set
        .tables()
        .find(|table| table.registry().path() == registry_path)
    else {
        return Vec::new();
    };
    let mut found: Vec<_> = table
        .tags()
        .map(|tag| {
            let path = format!(
                "{}/tags/{registry_path}/{}.json",
                tag.namespace(),
                tag.path()
            );
            (tag.clone(), path)
        })
        .collect();
    found.sort_by(|a, b| a.1.cmp(&b.1));
    found
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

#[allow(clippy::needless_pass_by_value)]
pub(crate) fn request_every_tag<T: RegistryKey + 'static, I: TagId>(
    mut loader: ResMut<TagLoader<T, I>>,
    asset_server: Res<AssetServer>,
    set: Res<RegistrySet>,
) {
    let files = list_tag_files(&set, T::KEY.path());
    let count = files.len();
    for (location, _) in files {
        loader.request(&TagKey::<T, _>::from_location(location), &asset_server);
    }
    tracing::info!(
        count,
        registry = T::KEY.path(),
        "requested every shipped tag"
    );
}

pub(crate) fn check_tags_ready(
    tags_settled: Res<TagLoadersSettled>,
    registry_assets: Res<LoadedRegistryAssets>,
    asset_server: Res<AssetServer>,
    mut next: ResMut<NextState<AppState>>,
) {
    if tags_settled.get() && registry_assets.all_handles_settled(&asset_server) {
        tracing::info!("all tag files and registry assets settled — entering WorldgenFreeze");
        next.set(AppState::WorldgenFreeze);
    }
}
