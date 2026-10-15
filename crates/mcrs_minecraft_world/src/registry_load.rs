use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registries::{
    AvailablePack, Diagnostic, Diagnostics, Features, KnownPack, Level, PackFiles, PackSelection,
    PackStack, Registries, StackedPack, Stage, load_with, pack_stack,
};
use mcrs_minecraft_registry::{
    AssemblyError, LoadReport, PACKS_ROOT, RegistrySet, WorldRegistries,
};

use crate::declarations::{self, Plan, StageSet};
use crate::packs::DATAPACK_REPORT;
use crate::registries::{reloadable_registries, static_registries, world_registries};

const FEATURE_PACKS: &str = "minecraft/datapacks";
const VANILLA_NAMESPACE: &str = "minecraft";
const METADATA: &str = "pack.mcmeta";
const VANILLA_FEATURE: &str = "minecraft:vanilla";
const STANDARD_DATA: &str = "data";

pub struct PackEntry {
    pub name: String,
    pub directory: bool,
}

pub trait PackSource {
    fn read(&self, path: &str) -> Result<Vec<u8>, String>;

    fn list(&self, directory: &str) -> Result<Vec<PackEntry>, String>;
}

pub struct DirectoryPacks {
    root: PathBuf,
}

impl DirectoryPacks {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl PackSource for DirectoryPacks {
    fn read(&self, path: &str) -> Result<Vec<u8>, String> {
        std::fs::read(self.root.join(path)).map_err(|error| format!("{path}: {error}"))
    }

    fn list(&self, directory: &str) -> Result<Vec<PackEntry>, String> {
        let entries = match std::fs::read_dir(self.root.join(directory)) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(format!("{directory}: {error}")),
        };
        let mut listed = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| format!("{directory}: {error}"))?;
            let kind = entry
                .file_type()
                .map_err(|error| format!("{directory}: {error}"))?;
            if let Ok(name) = entry.file_name().into_string() {
                listed.push(PackEntry {
                    name,
                    directory: kind.is_dir(),
                });
            }
        }
        Ok(listed)
    }
}

#[cfg(feature = "bevy")]
pub(crate) struct AssetReaderPacks<'a>(pub &'a dyn bevy_asset::io::ErasedAssetReader);

#[cfg(feature = "bevy")]
impl PackSource for AssetReaderPacks<'_> {
    fn read(&self, path: &str) -> Result<Vec<u8>, String> {
        bevy_tasks::block_on(mcrs_minecraft_assets::asset::read_whole(
            self.0,
            std::path::Path::new(path),
        ))
        .map_err(|error| format!("{path}: {error}"))
    }

    fn list(&self, directory: &str) -> Result<Vec<PackEntry>, String> {
        use bevy_asset::io::AssetReaderError;
        use bevy_tasks::futures_lite::StreamExt;
        bevy_tasks::block_on(async {
            let mut paths = match self.0.read_directory(std::path::Path::new(directory)).await {
                Ok(paths) => paths,
                Err(AssetReaderError::NotFound(_)) => return Ok(Vec::new()),
                Err(error) => return Err(format!("{directory}: {error}")),
            };
            let mut listed = Vec::new();
            while let Some(path) = paths.next().await {
                let directory = self
                    .0
                    .is_directory(&path)
                    .await
                    .map_err(|error| format!("{}: {error}", path.display()))?;
                if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                    listed.push(PackEntry {
                        name: name.to_owned(),
                        directory,
                    });
                }
            }
            Ok(listed)
        })
    }
}

#[derive(Debug)]
pub enum RegistryLoadError {
    Read(String),
    Malformed(String),
    Declarations(String),
    Report(LoadReport),
    Load(Diagnostics),
    Assembly(AssemblyError),
}

impl fmt::Display for RegistryLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryLoadError::Read(message) => write!(f, "cannot read {message}"),
            RegistryLoadError::Malformed(message) | RegistryLoadError::Declarations(message) => {
                f.write_str(message)
            }
            RegistryLoadError::Report(report) => write!(f, "{report}"),
            RegistryLoadError::Load(diagnostics) => {
                let errors = diagnostics.iter().filter(|d| d.level() == Level::Error);
                write!(f, "registry load failed: {} errors", errors.count())?;
                for diagnostic in diagnostics.iter() {
                    write!(f, "\n{:?}: {diagnostic}", diagnostic.level())?;
                }
                Ok(())
            }
            RegistryLoadError::Assembly(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for RegistryLoadError {}

impl From<AssemblyError> for RegistryLoadError {
    fn from(error: AssemblyError) -> Self {
        RegistryLoadError::Assembly(error)
    }
}

pub struct LoadedRegistries {
    pub set: RegistrySet,
    pub known_packs: KnownPacks,
    pub diagnostics: Diagnostics,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackedKnownPack {
    pub pack: String,
    pub known: KnownPack,
}

#[cfg_attr(feature = "bevy", derive(bevy_ecs::resource::Resource))]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KnownPacks(pub Vec<StackedKnownPack>);

impl KnownPacks {
    fn of(stack: &PackStack) -> Self {
        let known = stack.packs().iter().filter_map(|pack| {
            Some(StackedKnownPack {
                pack: pack.id().to_owned(),
                known: pack.known_pack()?.clone(),
            })
        });
        Self(known.collect())
    }
}

fn read(source: &dyn PackSource, path: &str) -> Result<Vec<u8>, RegistryLoadError> {
    source.read(path).map_err(RegistryLoadError::Read)
}

fn list(source: &dyn PackSource, directory: &str) -> Result<Vec<PackEntry>, RegistryLoadError> {
    let mut entries = source.list(directory).map_err(RegistryLoadError::Read)?;
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

fn walk(source: &dyn PackSource, root: &str) -> Result<Vec<(String, String)>, RegistryLoadError> {
    let mut found = Vec::new();
    let mut directories = vec![root.to_owned()];
    while let Some(directory) = directories.pop() {
        for entry in list(source, &directory)? {
            let path = format!("{directory}/{}", entry.name);
            if entry.directory {
                if path != FEATURE_PACKS {
                    directories.push(path);
                }
            } else if entry.name.ends_with(".json")
                && let Some(relative) = path.strip_prefix(root)
            {
                let relative = relative.trim_start_matches('/').to_owned();
                found.push((path, relative));
            }
        }
    }
    Ok(found)
}

enum PackRoot {
    Vanilla,
    Local(String),
}

struct Available {
    packs: Vec<AvailablePack>,
    features: Features,
    roots: BTreeMap<String, PackRoot>,
    unavailable: Vec<Diagnostic>,
}

fn available_packs(source: &dyn PackSource) -> Result<Available, RegistryLoadError> {
    let vanilla_feature = ResourceLocation::read(VANILLA_FEATURE)
        .map_err(|error| RegistryLoadError::Malformed(error.to_string()))?;
    let vanilla = AvailablePack::vanilla();
    let mut available = Available {
        features: Features {
            known: vec![vanilla_feature.clone()],
            vanilla: vec![vanilla_feature],
            world: None,
        },
        roots: BTreeMap::from([(vanilla.id().to_owned(), PackRoot::Vanilla)]),
        packs: vec![vanilla],
        unavailable: Vec::new(),
    };

    for entry in list(source, PACKS_ROOT)?
        .into_iter()
        .filter(|e| e.directory)
    {
        let root = format!("{PACKS_ROOT}/{}", entry.name);
        let unavailable = |reason| Diagnostic::UnavailablePack {
            pack: format!("file/{}", entry.name),
            reason,
        };
        if list(source, &root)?
            .iter()
            .any(|e| e.directory && e.name == STANDARD_DATA)
        {
            available.unavailable.push(unavailable(format!(
                "{root} has a top-level {STANDARD_DATA} directory, but a local pack keeps its \
                 namespaces at the pack root"
            )));
            continue;
        }
        let pack = match source.read(&format!("{root}/{METADATA}")) {
            Ok(metadata) => AvailablePack::directory(&entry.name, metadata),
            Err(reason) => {
                available.unavailable.push(unavailable(reason));
                continue;
            }
        };
        available
            .roots
            .insert(pack.id().to_owned(), PackRoot::Local(root));
        available.packs.push(pack);
    }
    Ok(available)
}

fn namespace_into(
    source: &dyn PackSource,
    files: &mut PackFiles,
    pack: &str,
    (root, data): (&str, &str),
    namespace: &str,
) -> Result<(), RegistryLoadError> {
    let directory = match root {
        "" => namespace.to_owned(),
        _ => format!("{root}/{namespace}"),
    };
    for (path, relative) in walk(source, &directory)? {
        files.insert(
            pack,
            format!("{data}/{namespace}/{relative}"),
            read(source, &path)?,
        );
    }
    Ok(())
}

fn local_into(
    source: &dyn PackSource,
    files: &mut PackFiles,
    pack: &StackedPack,
    root: &str,
) -> Result<(), RegistryLoadError> {
    let overlays = pack.overlays();
    for entry in list(source, root)?.into_iter().filter(|e| e.directory) {
        if !overlays.contains(&entry.name) {
            namespace_into(source, files, pack.id(), (root, STANDARD_DATA), &entry.name)?;
        }
    }
    for overlay in overlays {
        let overlay_root = format!("{root}/{overlay}");
        let data = format!("{overlay}/{STANDARD_DATA}");
        for entry in list(source, &overlay_root)?
            .into_iter()
            .filter(|e| e.directory)
        {
            namespace_into(
                source,
                files,
                pack.id(),
                (&overlay_root, &data),
                &entry.name,
            )?;
        }
    }
    Ok(())
}

fn pack_files(
    source: &dyn PackSource,
    stack: &PackStack,
    roots: &BTreeMap<String, PackRoot>,
) -> Result<PackFiles, RegistryLoadError> {
    let mut files = PackFiles::default();
    for pack in stack.packs() {
        match roots.get(pack.id()) {
            Some(PackRoot::Vanilla) => {
                namespace_into(
                    source,
                    &mut files,
                    pack.id(),
                    ("", STANDARD_DATA),
                    VANILLA_NAMESPACE,
                )?;
            }
            Some(PackRoot::Local(root)) => local_into(source, &mut files, pack, root)?,
            None => {}
        }
    }
    Ok(files)
}

pub(crate) fn log_diagnostics(diagnostics: &Diagnostics) {
    for diagnostic in diagnostics.iter() {
        match diagnostic.level() {
            Level::Info => tracing::info!("{diagnostic}"),
            Level::Warning => tracing::warn!("{diagnostic}"),
            Level::Error => tracing::error!("{diagnostic}"),
        }
    }
}

fn merged(mut first: Diagnostics, second: &Diagnostics) -> Diagnostics {
    first.extend(second.iter().cloned());
    first
}

pub fn load_registries(source: &dyn PackSource) -> Result<LoadedRegistries, RegistryLoadError> {
    let report = read(source, DATAPACK_REPORT)?;
    let statics = static_registries().map_err(RegistryLoadError::Report)?;
    let world_tables = world_registries(&report).map_err(RegistryLoadError::Report)?;
    let reloadable_tables = reloadable_registries(&report).map_err(RegistryLoadError::Report)?;

    let available = available_packs(source)?;
    let (stack, mut diagnostics) = pack_stack(
        &PackSelection::default(),
        &available.packs,
        &available.features,
        false,
    );
    diagnostics.extend(available.unavailable);
    let files = pack_files(source, &stack, &available.roots)?;
    let stack = Arc::new(stack);

    let stage = StageSet::new(statics);
    let plan = declarations::plan(&world_tables, &reloadable_tables, &stage)
        .map_err(RegistryLoadError::Declarations)?;

    let mut failure = None;
    let outcome = load_with(
        &plan.declarations,
        Arc::clone(&stack),
        &files,
        None,
        &mut |view, decode| match stage.grow(view) {
            Ok(set) => set.scope(&mut *decode),
            Err(error) => {
                failure.get_or_insert(error);
            }
        },
    );
    drop(files);
    if let Some(error) = failure {
        return Err(error.into());
    }
    let (registries, loaded) =
        outcome.map_err(|failed| RegistryLoadError::Load(merged(diagnostics.clone(), &failed)))?;
    let diagnostics = merged(diagnostics, &loaded);

    let set = assemble(
        &plan,
        &registries,
        &stack,
        stage.get(),
        &world_tables,
        &reloadable_tables,
    )?;
    Ok(LoadedRegistries {
        set,
        known_packs: KnownPacks::of(&stack),
        diagnostics,
    })
}

fn assemble(
    plan: &Plan,
    registries: &Registries,
    stack: &PackStack,
    mut set: RegistrySet,
    world_tables: &WorldRegistries,
    reloadable_tables: &WorldRegistries,
) -> Result<RegistrySet, RegistryLoadError> {
    let mut origins = Vec::new();
    for planned in &plan.registries {
        let key = planned.key.as_str();
        let table = match planned.stage {
            Stage::Reloadable => reloadable_tables,
            Stage::World | Stage::Dimension => world_tables,
        };
        if let Some(attach) = planned.attach {
            set = attach(registries, table, set, key)?;
        }
        let of = (planned.origins)(registries, stack).ok_or_else(|| {
            RegistryLoadError::Declarations(format!("{key} has an entry of no stacked pack"))
        })?;
        origins.push((key.to_owned(), of.into_boxed_slice()));
    }
    let packs: Vec<String> = stack
        .packs()
        .iter()
        .map(|pack| pack.id().to_owned())
        .collect();
    let set = set.with_origins(packs, origins)?;
    world_tables.sync(set).map_err(RegistryLoadError::Report)
}

static TEST_REGISTRIES: LazyLock<LoadedRegistries> = LazyLock::new(|| {
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    load_registries(&DirectoryPacks::new(assets)).unwrap_or_else(|error| {
        eprintln!("the registries are unusable:\n{error}");
        std::process::exit(1)
    })
});

pub fn test_registries() -> &'static RegistrySet {
    &TEST_REGISTRIES.set
}

pub fn test_known_packs() -> &'static KnownPacks {
    &TEST_REGISTRIES.known_packs
}
