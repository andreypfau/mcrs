use bevy_app::{App, AppExit, Plugin};
use bevy_asset::AssetServer;
use bevy_asset::io::{AssetReaderError, AssetSourceId, ErasedAssetReader};
use bevy_ecs::prelude::*;
use bevy_state::prelude::*;
use bevy_tasks::futures_lite::StreamExt;
use mcrs_minecraft_assets::{AppState, LoadedRegistryAssets};
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registries::{CurrentRegistries, Registries, RegistryAdditions};
use std::collections::BTreeSet;
use std::path::Path;

use crate::template_asset::{TemplateAsset, WorldgenAssetsPlugin, named_templates};
use crate::template_handle::template_path;
use mcrs_minecraft_worldgen::registries::add_vanilla_worldgen;

/// The registries of vanilla world generation, the entries built by code and the structure
/// templates the loaded registries name. The world plugin waits for the templates before the
/// registries freeze.
pub struct WorldgenRegistriesPlugin;

impl Plugin for WorldgenRegistriesPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(WorldgenAssetsPlugin);
        let added =
            add_vanilla_worldgen(&mut app.world_mut().get_resource_or_init::<RegistryAdditions>());
        if let Err(error) = added {
            tracing::error!("the built-in worldgen entries are unusable: {error}");
            app.world_mut().write_message(AppExit::error());
        }
        app.add_systems(
            OnEnter(AppState::LoadingDataPack),
            request_data_pack_templates,
        );
    }
}

const TEMPLATE_FOLDER: &str = "minecraft/structure";

/// The `<folder>/<file>.<extension>` paths the default `AssetSource` lists for a folder, with the
/// templates the code builds added, since a listing names files only.
// chisle: a source that cannot list directories yields only the built-in templates here, so
// a template neither built nor named by a loaded value is not requested; listing the files
// through the pack stack the registry loader read lifts this.
fn list_registry_files(asset_server: &AssetServer, folder: &str, extension: &str) -> Vec<String> {
    let mut files: BTreeSet<String> = match asset_server.get_source(AssetSourceId::Default) {
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
            BTreeSet::new()
        }
    };
    files.extend(mcrs_minecraft_worldgen_builtin::template_paths(folder));
    files.into_iter().collect()
}

fn request_templates(
    asset_server: &AssetServer,
    registries: &Registries,
    loaded: &mut LoadedRegistryAssets,
) {
    let listed = list_registry_files(asset_server, TEMPLATE_FOLDER, "nbt");
    let named = named_templates(registries).unwrap_or_else(|error| {
        tracing::error!("the loaded registries cannot list their templates: {error}");
        BTreeSet::new()
    });
    let files = match asset_server.get_source(AssetSourceId::Default) {
        Ok(source) => bevy_tasks::block_on(with_named_templates(source.reader(), listed, &named)),
        Err(_) => listed,
    };
    let count = files.len();
    for path in files {
        loaded.push(asset_server.load::<TemplateAsset>(path).untyped());
    }
    tracing::info!(
        folder = TEMPLATE_FOLDER,
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

fn request_data_pack_templates(
    asset_server: Res<AssetServer>,
    registries: Res<CurrentRegistries>,
    mut loaded: ResMut<LoadedRegistryAssets>,
) {
    request_templates(&asset_server, &registries, &mut loaded);
}

async fn walk_files(
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_asset::io::memory::{Dir, MemoryAssetReader};
    use mcrs_minecraft_assets::packs::layered_reader;
    use mcrs_minecraft_datapack::{DirectoryEntry, DirectoryPacks, PackSource, ReadError};

    #[test]
    fn a_named_template_is_requested_when_any_pack_layer_holds_it() {
        let root = Dir::default();
        for path in [
            "minecraft/structure/listed.nbt",
            "minecraft/structure/unlisted.nbt",
            "mcrs/datapacks/extra/pack.mcmeta",
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

    struct WithExtraPack {
        base: DirectoryPacks,
        files: Vec<(String, Vec<u8>)>,
    }

    impl PackSource for WithExtraPack {
        fn read(&self, path: &str) -> Result<Vec<u8>, ReadError> {
            match self.files.iter().find(|(file, _)| file == path) {
                Some((_, bytes)) => Ok(bytes.clone()),
                None => self.base.read(path),
            }
        }

        fn list(&self, directory: &str) -> Result<Vec<DirectoryEntry>, ReadError> {
            let mut listed = self.base.list(directory)?;
            let prefix = format!("{directory}/");
            for (path, _) in &self.files {
                let Some(rest) = path.strip_prefix(&prefix) else {
                    continue;
                };
                let (name, directory) = match rest.split_once('/') {
                    Some((name, _)) => (name, true),
                    None => (rest, false),
                };
                if !listed.iter().any(|entry| entry.name == name) {
                    listed.push(DirectoryEntry {
                        name: name.to_owned(),
                        directory,
                    });
                }
            }
            Ok(listed)
        }
    }

    #[test]
    fn a_template_named_only_by_an_extra_pack_pool_is_requested() {
        use crate::template_handle::template_handle;
        use bevy_app::{App, TaskPoolPlugin};
        use bevy_asset::io::AssetSourceBuilder;
        use bevy_asset::io::memory::MemoryAssetReader;
        use bevy_asset::{AssetApp, AssetPlugin};
        use mcrs_minecraft_world::registry_load::load_registries;

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
        let metadata = std::fs::read(assets.join("mcrs/datapacks/beta/pack.mcmeta")).unwrap();
        let source = WithExtraPack {
            base: DirectoryPacks::new(assets, mcrs_minecraft_fixtures::vanilla_dir().to_owned()),
            files: vec![
                ("mcrs/datapacks/extra/pack.mcmeta".to_owned(), metadata),
                (
                    "mcrs/datapacks/extra/example/worldgen/template_pool/camp.json".to_owned(),
                    pool.into_bytes(),
                ),
            ],
        };
        let mut additions = RegistryAdditions::default();
        add_vanilla_worldgen(&mut additions).unwrap();
        let (registries, _) =
            load_registries(&source, additions).unwrap_or_else(|error| panic!("{error}"));

        let root = Dir::default();
        root.insert_asset(Path::new("minecraft/structure/listed.nbt"), Vec::new());
        root.insert_asset(Path::new("mcrs/datapacks/extra/pack.mcmeta"), Vec::new());
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
        request_templates(&server, &registries, &mut loaded);

        let requested = |id: &str| template_handle(&server, &ResourceLocation::read(id).unwrap());
        assert!(requested("example:house").is_some());
        assert!(requested("example:missing").is_none());
    }
}
