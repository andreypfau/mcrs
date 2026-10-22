use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::Decode;
use mcrs_minecraft_protocol::packets::configuration::ClientboundRegistryData;
use mcrs_minecraft_registries::{
    AvailablePack, Diagnostic, Diagnostics, Features, Level, PackFiles, PackSelection, PackStack,
    Received, Registries, StackedPack, Stage, load_with, pack_stack,
};
use mcrs_minecraft_registry::{
    AssemblyError, LoadReport, PACKS_ROOT, RegistrySet, VANILLA_ROOT, WorldRegistries,
    below_vanilla_root,
};

use crate::declarations::{self, Plan, StageSet};
use crate::packs::DATAPACK_REPORT;
use crate::registries::{reloadable_registries, static_registries, world_registries};

const FEATURE_PACKS: &str = "minecraft/datapacks";
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

/// The packs under `assets`, with the vanilla pack's directory read from `vanilla`, which holds
/// the game jar's `data/minecraft` tree.
pub struct DirectoryPacks {
    assets: PathBuf,
    vanilla: PathBuf,
}

impl DirectoryPacks {
    pub fn new(assets: PathBuf, vanilla: PathBuf) -> Self {
        Self { assets, vanilla }
    }

    fn path(&self, path: &str) -> PathBuf {
        match below_vanilla_root(Path::new(path)) {
            Some(below) => self.vanilla.join(below),
            None => self.assets.join(path),
        }
    }
}

impl PackSource for DirectoryPacks {
    fn read(&self, path: &str) -> Result<Vec<u8>, String> {
        std::fs::read(self.path(path)).map_err(|error| format!("{path}: {error}"))
    }

    fn list(&self, directory: &str) -> Result<Vec<PackEntry>, String> {
        let entries = match std::fs::read_dir(self.path(directory)) {
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
    pub registries: Arc<Registries>,
    pub diagnostics: Diagnostics,
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

fn vanilla_pack() -> Result<Available, RegistryLoadError> {
    let vanilla_feature = ResourceLocation::read(VANILLA_FEATURE)
        .map_err(|error| RegistryLoadError::Malformed(error.to_string()))?;
    let vanilla = AvailablePack::vanilla();
    Ok(Available {
        features: Features {
            known: vec![vanilla_feature.clone()],
            vanilla: vec![vanilla_feature],
            world: None,
        },
        roots: BTreeMap::from([(vanilla.id().to_owned(), PackRoot::Vanilla)]),
        packs: vec![vanilla],
        unavailable: Vec::new(),
    })
}

fn available_packs(source: &dyn PackSource) -> Result<Available, RegistryLoadError> {
    let mut available = vanilla_pack()?;
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
                    VANILLA_ROOT,
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
    load_stack(source, available_packs(source)?)
}

pub fn load_vanilla_registries(
    source: &dyn PackSource,
) -> Result<LoadedRegistries, RegistryLoadError> {
    load_stack(source, vanilla_pack()?)
}

fn load_stack(
    source: &dyn PackSource,
    available: Available,
) -> Result<LoadedRegistries, RegistryLoadError> {
    let report = read(source, DATAPACK_REPORT)?;
    let statics = static_registries().map_err(RegistryLoadError::Report)?;
    let world_tables = world_registries(&report).map_err(RegistryLoadError::Report)?;
    let reloadable_tables = reloadable_registries(&report).map_err(RegistryLoadError::Report)?;

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
        false,
    )?;
    Ok(LoadedRegistries {
        set,
        registries: Arc::new(registries),
        diagnostics,
    })
}

/// The network form of every synced entry of a load: what a server that holds the same pack
/// would send. A client fills an entry the server sent without data from it.
pub struct KnownEntries(HashMap<(ResourceLocation, ResourceLocation), NbtTag>);

impl KnownEntries {
    pub fn of(registries: &Registries) -> Result<Self, RegistryLoadError> {
        let mut entries = HashMap::new();
        for body in registries.network().registry_data(false) {
            let packet = ClientboundRegistryData::decode(&mut &body[..])
                .map_err(|error| RegistryLoadError::Declarations(error.to_string()))?;
            let registry: ResourceLocation = packet.registry.into();
            for entry in packet.entries {
                if let Some(data) = entry.data {
                    entries.insert((registry.clone(), entry.id.into()), data.into_owned());
                }
            }
        }
        Ok(Self(entries))
    }
}

/// What a client collected of a server's registries in one configuration: each synced
/// registry's entries in the order sent, data absent where the server left it out, and the tags.
pub type Collected = (
    Vec<(ResourceLocation, Vec<(ResourceLocation, Option<NbtTag>)>)>,
    Vec<(ResourceLocation, Vec<(ResourceLocation, Vec<i32>)>)>,
);

/// The registries a client holds after a server's configuration. An entry sent without data is
/// filled from `known`; without `known`, every entry must carry its data.
pub fn load_received(
    report: &[u8],
    known: Option<&KnownEntries>,
    (registries, tags): Collected,
) -> Result<LoadedRegistries, RegistryLoadError> {
    let mut missing = Vec::new();
    let registries = registries
        .into_iter()
        .map(|(registry, entries)| {
            let entries = entries
                .into_iter()
                .filter_map(|(name, data)| {
                    let data = data.or_else(|| {
                        known.and_then(|known| {
                            known.0.get(&(registry.clone(), name.clone())).cloned()
                        })
                    });
                    if data.is_none() {
                        missing.push(Diagnostic::Entry {
                            registry: registry.clone(),
                            entry: name.clone(),
                            pack: None,
                            file: None,
                            field: None,
                            message: "the entry is sent without data and no local pack holds it"
                                .to_owned(),
                        });
                    }
                    Some((name, data?))
                })
                .collect();
            (registry, entries)
        })
        .collect();
    if !missing.is_empty() {
        let mut report = Diagnostics::default();
        report.extend(missing);
        return Err(RegistryLoadError::Load(report));
    }
    let received = Received { registries, tags };

    let statics = static_registries().map_err(RegistryLoadError::Report)?;
    let world_tables = world_registries(report).map_err(RegistryLoadError::Report)?;
    let reloadable_tables = reloadable_registries(report).map_err(RegistryLoadError::Report)?;
    let stage = StageSet::new(statics);
    let plan = declarations::plan(&world_tables, &reloadable_tables, &stage)
        .map_err(RegistryLoadError::Declarations)?;
    let mut failure = None;
    let outcome = mcrs_minecraft_registries::load_received(
        &plan.declarations,
        &received,
        &mut |view, decode| match stage.grow(view) {
            Ok(set) => set.scope(&mut *decode),
            Err(error) => {
                failure.get_or_insert(error);
            }
        },
    );
    if let Some(error) = failure {
        return Err(error.into());
    }
    let (registries, diagnostics) = outcome.map_err(RegistryLoadError::Load)?;
    let stack = PackStack::default();
    let set = assemble(
        &plan,
        &registries,
        &stack,
        stage.get(),
        &world_tables,
        &reloadable_tables,
        true,
    )?;
    Ok(LoadedRegistries {
        set,
        registries: Arc::new(registries),
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
    received: bool,
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
        match (planned.origins)(registries, stack) {
            Some(of) => origins.push((key.to_owned(), of.into_boxed_slice())),
            // An entry a server sent with its data comes from no pack of the client's stack.
            None if received => {}
            None => {
                return Err(RegistryLoadError::Declarations(format!(
                    "{key} has an entry of no stacked pack"
                )));
            }
        }
    }
    let packs: Vec<String> = stack
        .packs()
        .iter()
        .map(|pack| pack.id().to_owned())
        .collect();
    let set = set.with_origins(packs, origins)?;
    world_tables.sync(set).map_err(RegistryLoadError::Report)
}

/// The repository's assets over the pinned client jar's data pack, which must be installed.
#[cfg(all(feature = "test-support", not(target_family = "wasm")))]
pub fn test_packs() -> DirectoryPacks {
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    DirectoryPacks::new(assets, mcrs_minecraft_fixtures::vanilla_dir().to_owned())
}

#[cfg(all(feature = "test-support", not(target_family = "wasm")))]
static TEST_REGISTRIES: std::sync::LazyLock<LoadedRegistries> = std::sync::LazyLock::new(|| {
    load_registries(&test_packs())
        .unwrap_or_else(|error| panic!("the registries are unusable:\n{error}"))
});

#[cfg(all(feature = "test-support", not(target_family = "wasm")))]
pub fn test_registries() -> &'static RegistrySet {
    &TEST_REGISTRIES.set
}

#[cfg(all(feature = "test-support", not(target_family = "wasm")))]
pub fn test_registries_arc() -> &'static Arc<Registries> {
    &TEST_REGISTRIES.registries
}

/// Registries over `declarations` alone: vanilla is the only pack and it holds no files.
#[cfg(all(feature = "test-support", not(target_family = "wasm")))]
pub fn registries_declaring(declarations: &[mcrs_minecraft_registries::Declaration]) -> Registries {
    let vanilla = ResourceLocation::read(VANILLA_FEATURE).expect("a valid feature name");
    let features = Features {
        known: vec![vanilla.clone()],
        vanilla: vec![vanilla],
        world: None,
    };
    let (stack, _) = pack_stack(
        &PackSelection::default(),
        &[AvailablePack::vanilla()],
        &features,
        false,
    );
    mcrs_minecraft_registries::load(declarations, Arc::new(stack), &PackFiles::default(), None)
        .map(|(registries, _)| registries)
        .unwrap_or_else(|diagnostics| panic!("the declarations do not load:\n{diagnostics}"))
}
