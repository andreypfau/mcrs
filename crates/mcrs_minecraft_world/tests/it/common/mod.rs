use mcrs_minecraft_core::{Registered, TagKey};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_state::app::StatesPlugin;
use bevy_state::state::State;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_assets::packs::PackRoots;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_minecraft_datapack::{DirectoryEntry, DirectoryPacks, PackSource, ReadError};
use mcrs_minecraft_fixtures::vanilla_dir;
use mcrs_minecraft_item::Items;
use mcrs_minecraft_registries::Diagnostics;
use mcrs_minecraft_registries::{Namer, Registries, Resolve};
use mcrs_minecraft_world::MinecraftWorldPlugin;
use mcrs_minecraft_world::item::test_corpus;
use mcrs_minecraft_world::registry_load::{RegistryLoadError, load_registries};
use serde::Deserialize;
use serde::de::DeserializeOwned;

pub fn read_named<T: Resolve>(set: &Registries, json: &str) -> Result<T, String>
where
    T::Named: DeserializeOwned,
{
    let named = serde_json::from_str::<T::Named>(json).map_err(|e| e.to_string())?;
    T::resolve(named, set).map_err(|e| e.to_string())
}

pub fn write_named<T: Resolve>(set: &Registries, value: &T) -> T::Named {
    value.name(set).unwrap_or_else(|e| panic!("{e}"))
}

pub fn items() -> &'static Items {
    &test_corpus().1
}

pub fn workspace_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

pub fn assets() -> PathBuf {
    workspace_root().join("assets")
}

pub fn pack_roots() -> PackRoots {
    PackRoots {
        assets: assets().to_string_lossy().into_owned(),
        vanilla: vanilla_dir().to_owned(),
    }
}

pub fn directory_packs() -> DirectoryPacks {
    DirectoryPacks::new(assets(), vanilla_dir().to_owned())
}

#[derive(Deserialize)]
pub struct Flags {
    pub elements: bool,
    pub stable: bool,
}

#[derive(Deserialize)]
pub struct DatapackReport {
    pub registries: BTreeMap<String, Flags>,
}

pub fn datapack_report() -> DatapackReport {
    let path = assets().join("mcrs/reports/datapack.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

pub fn declared_registries(stable: bool) -> BTreeSet<String> {
    datapack_report()
        .registries
        .into_iter()
        .filter(|(_, flags)| flags.elements && flags.stable == stable)
        .map(|(registry, _)| registry)
        .collect()
}

pub fn loaded_names(set: &Registries, registry: &str) -> Vec<String> {
    set.names_of(&ResourceLocation::read(registry).unwrap())
        .unwrap_or_else(|| panic!("{registry} is not a loaded registry"))
        .iter()
        .map(ToString::to_string)
        .collect()
}

pub fn tag_members<R: Registered>(set: &Registries, tag: &str) -> Vec<String> {
    let tags = set
        .tags::<R>()
        .unwrap_or_else(|| panic!("the load builds the tags of {}", std::any::type_name::<R>()));
    let id = tags
        .id(&TagKey::from_location(ResourceLocation::read(tag).unwrap()))
        .unwrap_or_else(|| panic!("{tag} is a loaded tag of {}", std::any::type_name::<R>()));
    tags.members(id)
        .map(|member| set.name(member).unwrap().as_str().to_owned())
        .collect()
}

pub fn run_to_playing() -> App {
    std::env::set_current_dir(workspace_root()).unwrap();
    let _ = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_test_writer()
        .try_init();

    let mut app = App::new();
    app.add_plugins(TaskPoolPlugin {
        task_pool_options: bevy_app::TaskPoolOptions::with_num_threads(2),
    });
    app.add_plugins(StatesPlugin);
    mcrs_minecraft_assets::packs::register_pack_sources(
        &mut app,
        &pack_roots(),
        mcrs_minecraft_worldgen_builtin::asset,
    );
    app.add_plugins(AssetPlugin {
        watch_for_changes_override: Some(false),
        ..Default::default()
    });
    app.add_plugins(mcrs_minecraft_assets::MinecraftCorePlugin);
    app.add_plugins(MinecraftWorldPlugin);
    app.finish();
    app.cleanup();

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        app.update();
        if *app.world().resource::<State<AppState>>().get() == AppState::Playing {
            return app;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "never reached Playing"
        );
    }
}

pub fn plugin_app() -> App {
    let mut app = App::new();
    app.add_plugins(TaskPoolPlugin::default());
    app.add_plugins(StatesPlugin);
    mcrs_minecraft_assets::packs::register_pack_sources(
        &mut app,
        &pack_roots(),
        mcrs_minecraft_worldgen_builtin::asset,
    );
    app.add_plugins(AssetPlugin {
        watch_for_changes_override: Some(false),
        ..Default::default()
    });
    app.add_plugins(mcrs_minecraft_assets::MinecraftCorePlugin);
    app.add_plugins(MinecraftWorldPlugin);
    app.finish();
    app.cleanup();
    app
}

pub const TEST_PACK: &str = "mcrs/datapacks/zz_test";

pub struct WithFiles {
    base: DirectoryPacks,
    files: BTreeMap<String, Option<Vec<u8>>>,
    hidden: Vec<String>,
}

impl WithFiles {
    pub fn over_assets() -> Self {
        Self {
            base: directory_packs(),
            files: BTreeMap::new(),
            hidden: Vec::new(),
        }
    }

    pub fn with<P, B>(mut self, path: P, bytes: B) -> Self
    where
        P: Into<String>,
        B: Into<Vec<u8>>,
    {
        self.files.insert(path.into(), Some(bytes.into()));
        self
    }

    pub fn unreadable(mut self, path: &str) -> Self {
        self.files.insert(path.to_owned(), None);
        self
    }

    pub fn pack(self, root: &str) -> Self {
        let metadata = std::fs::read(assets().join("mcrs/datapacks/beta/pack.mcmeta")).unwrap();
        self.with(format!("{root}/pack.mcmeta"), metadata)
    }

    pub fn test_pack<P: AsRef<str>, T: AsRef<str>>(files: &[(P, T)]) -> Self {
        files.iter().fold(
            Self::over_assets().pack(TEST_PACK),
            |source, (path, text)| {
                source.with(
                    format!("{TEST_PACK}/{}", path.as_ref()),
                    text.as_ref().as_bytes(),
                )
            },
        )
    }

    pub fn hiding(mut self, directory: &str) -> Self {
        self.hidden.push(directory.to_owned());
        self
    }

    fn is_hidden(&self, path: &str) -> bool {
        self.hidden.iter().any(|hidden| {
            path == hidden
                || path
                    .strip_prefix(hidden.as_str())
                    .is_some_and(|rest| rest.starts_with('/'))
        })
    }

    pub fn load(&self) -> Result<(Registries, Diagnostics), RegistryLoadError> {
        load_registries(self, Vec::new())
    }
}

impl PackSource for WithFiles {
    fn read(&self, path: &str) -> Result<Vec<u8>, ReadError> {
        match self.files.get(path) {
            Some(Some(bytes)) => Ok(bytes.clone()),
            Some(None) => Err(ReadError::new(path, "unreadable")),
            None if self.is_hidden(path) => Err(ReadError::new(path, "hidden")),
            None => self.base.read(path),
        }
    }

    fn list(&self, directory: &str) -> Result<Vec<DirectoryEntry>, ReadError> {
        let mut listed = match self.is_hidden(directory) {
            true => Vec::new(),
            false => self.base.list(directory)?,
        };
        listed.retain(|entry| !self.is_hidden(&format!("{directory}/{}", entry.name)));
        let prefix = format!("{directory}/");
        for path in self.files.keys() {
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
