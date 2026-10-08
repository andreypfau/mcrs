use crate::LoadedRegistryAssets;
use bevy_asset::AssetServer;
use bevy_asset::io::{AssetReaderError, AssetSourceId, ErasedAssetReader};
use bevy_ecs::prelude::*;
use bevy_state::prelude::*;
use bevy_tasks::futures_lite::StreamExt;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_worldgen::bevy::named_templates;
use mcrs_minecraft_worldgen::tables::template_path;
use std::collections::BTreeSet;
use std::path::Path;

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
/// names files only, so the templates the code builds are added to it.
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
    files.extend(mcrs_minecraft_worldgen_builtin::template_paths(folder));
    files.into_iter().collect()
}

fn request_templates(
    asset_server: &AssetServer,
    set: &RegistrySet,
    loaded: &mut LoadedRegistryAssets,
) {
    use registry_files::{FILES_TEMPLATE, FOLDER_TEMPLATE};
    let listed = list_registry_files(asset_server, FOLDER_TEMPLATE, "nbt", FILES_TEMPLATE);
    let named = named_templates(set).unwrap_or_else(|error| {
        panic!("the loaded registries cannot list their templates: {error}")
    });
    let files = match asset_server.get_source(AssetSourceId::Default) {
        Ok(source) => bevy_tasks::block_on(with_named_templates(source.reader(), listed, &named)),
        Err(_) => listed,
    };
    let count = files.len();
    for path in files {
        loaded.push(
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

// A listing sees only the root while a read sees every pack layer, so a
// template a datapack ships is found by reading the path a value names.
async fn with_named_templates(
    reader: &dyn ErasedAssetReader,
    listed: Vec<String>,
    named: &BTreeSet<ResourceLocation>,
) -> Vec<String> {
    let mut files: BTreeSet<String> = listed.into_iter().collect();
    for id in named {
        let path = template_path(id);
        if !files.contains(&path)
            && !matches!(
                reader.read(Path::new(&path)).await,
                Err(AssetReaderError::NotFound(_))
            )
        {
            files.insert(path);
        }
    }
    files.into_iter().collect()
}

pub(crate) fn request_data_pack_assets(
    asset_server: Res<AssetServer>,
    set: Res<RegistrySet>,
    mut loaded: ResMut<LoadedRegistryAssets>,
) {
    request_templates(&asset_server, &set, &mut loaded);
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_asset::io::memory::{Dir, MemoryAssetReader};
    use mcrs_minecraft_assets::packs::layered_reader;

    #[test]
    fn a_named_template_is_requested_when_any_pack_layer_holds_it() {
        let root = Dir::default();
        for path in [
            "minecraft/structure/listed.nbt",
            "minecraft/structure/unlisted.nbt",
            "mcrs/datapacks/extra/example/structure/house.nbt",
            "example/structure/twice.nbt",
            "mcrs/datapacks/extra/example/structure/twice.nbt",
            "example/structure/unnamed.nbt",
        ] {
            root.insert_asset(Path::new(path), Vec::new());
        }
        let reader = layered_reader(Box::new(MemoryAssetReader { root }), |_| None);
        let named = [
            "minecraft:listed",
            "minecraft:unlisted",
            "example:house",
            "example:twice",
            "minecraft:missing",
        ]
        .map(|id| ResourceLocation::read(id).unwrap())
        .into();

        let files = bevy_tasks::block_on(with_named_templates(
            &*reader,
            vec!["minecraft/structure/listed.nbt".to_owned()],
            &named,
        ));
        assert_eq!(
            files,
            [
                "example/structure/house.nbt",
                "example/structure/twice.nbt",
                "minecraft/structure/listed.nbt",
                "minecraft/structure/unlisted.nbt",
            ]
        );
    }

    #[test]
    fn a_template_named_only_by_an_extra_pack_pool_is_requested() {
        use crate::packs::{DATAPACK_REPORT, read_packs_from_directory};
        use crate::registries::{static_registries, world_registries};
        use bevy_app::{App, TaskPoolPlugin};
        use bevy_asset::io::AssetSourceBuilder;
        use bevy_asset::io::memory::MemoryAssetReader;
        use bevy_asset::{AssetApp, AssetPlugin};
        use mcrs_minecraft_registry::{Pack, PackFile};
        use mcrs_minecraft_worldgen::bevy::WorldgenAssetsPlugin;
        use mcrs_minecraft_worldgen::tables::template_handle;

        let element = |template: &str| {
            format!(
                r#"{{"weight":1,"element":{{"element_type":"minecraft:single_pool_element","location":"{template}","processors":"minecraft:empty","projection":"rigid"}}}}"#
            )
        };
        let pool = format!(
            r#"{{"fallback":"minecraft:empty","elements":[{},{}]}}"#,
            element("example:house"),
            element("example:missing")
        );

        let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let report = std::fs::read(assets.join(DATAPACK_REPORT)).unwrap();
        let statics = static_registries().unwrap();
        let world = world_registries(&report).unwrap();
        let mut packs = read_packs_from_directory(&assets, &world, &statics).unwrap();
        packs.push(Pack {
            name: "extra".to_owned(),
            files: vec![PackFile {
                path: "example/worldgen/template_pool/camp.json".to_owned(),
                bytes: Some(pool.into_bytes()),
            }],
            built: Vec::new(),
        });
        let set = world
            .load(&statics, &packs)
            .unwrap_or_else(|report| panic!("{report}"));

        let root = Dir::default();
        root.insert_asset(Path::new("minecraft/structure/listed.nbt"), Vec::new());
        root.insert_asset(
            Path::new("mcrs/datapacks/extra/example/structure/house.nbt"),
            Vec::new(),
        );
        let mut app = App::new();
        app.register_asset_source(
            AssetSourceId::Default,
            AssetSourceBuilder::new(move || {
                layered_reader(Box::new(MemoryAssetReader { root: root.clone() }), |_| None)
            }),
        );
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin::default(),
            WorldgenAssetsPlugin,
        ));
        let server = app.world().resource::<AssetServer>().clone();

        let mut loaded = LoadedRegistryAssets::default();
        request_templates(&server, &set, &mut loaded);

        let requested = |id: &str| template_handle(&server, &ResourceLocation::read(id).unwrap());
        assert!(requested("example:house").is_some());
        assert!(requested("example:missing").is_none());
    }
}
