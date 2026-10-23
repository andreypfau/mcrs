use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mcrs_minecraft_assets::packs::{PACKS_ROOT, VANILLA_ROOT, below_vanilla_root};
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::Decode;
use mcrs_minecraft_protocol::packets::configuration::ClientboundRegistryData;
use mcrs_minecraft_datapack::{
    AvailablePack, Features, Level, PackDiagnostic, PackFiles, PackSelection, PackStack,
    StackedPack, pack_stack,
};
use mcrs_minecraft_registries::{Diagnostic, Diagnostics, Received, Registries, load};

use crate::declarations;
use crate::packs::DATAPACK_REPORT;

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
    Load(Diagnostics),
}

impl fmt::Display for RegistryLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryLoadError::Read(message) => write!(f, "cannot read {message}"),
            RegistryLoadError::Malformed(message) | RegistryLoadError::Declarations(message) => {
                f.write_str(message)
            }
            RegistryLoadError::Load(diagnostics) => {
                let errors = diagnostics.iter().filter(|d| d.level() == Level::Error);
                write!(f, "registry load failed: {} errors", errors.count())?;
                for diagnostic in diagnostics.iter() {
                    write!(f, "\n{:?}: {diagnostic}", diagnostic.level())?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for RegistryLoadError {}

pub struct LoadedRegistries {
    pub registries: Registries,
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
    unavailable: Vec<PackDiagnostic>,
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
        let unavailable = |reason| PackDiagnostic::UnavailablePack {
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

#[cfg(feature = "bevy")]
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
    let declarations = declarations::plan(&report).map_err(RegistryLoadError::Declarations)?;

    let (stack, found) = pack_stack(
        &PackSelection::default(),
        &available.packs,
        &available.features,
        false,
    );
    let mut diagnostics = Diagnostics::default();
    diagnostics.extend(found.into_iter().chain(available.unavailable).map(Diagnostic::Pack));
    let files = pack_files(source, &stack, &available.roots)?;

    let outcome = load(&declarations, Arc::new(stack), &files, None);
    drop(files);
    let (registries, loaded) =
        outcome.map_err(|failed| RegistryLoadError::Load(merged(diagnostics.clone(), &failed)))?;
    Ok(LoadedRegistries {
        registries,
        diagnostics: merged(diagnostics, &loaded),
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

    let declarations = declarations::plan(report).map_err(RegistryLoadError::Declarations)?;
    let (registries, diagnostics) =
        mcrs_minecraft_registries::load_received(&declarations, &received)
            .map_err(RegistryLoadError::Load)?;
    Ok(LoadedRegistries {
        registries,
        diagnostics,
    })
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
pub fn test_registries() -> &'static Registries {
    &TEST_REGISTRIES.registries
}

/// What a client collects from the payloads `registries` encoded; entries of the vanilla pack go
/// without data when the client is `known` to hold it.
#[cfg(all(feature = "test-support", not(target_family = "wasm")))]
pub fn sent_by(registries: &Registries, known: bool) -> Collected {
    use mcrs_minecraft_protocol::packets::configuration::ClientboundUpdateTags;

    let network = registries.network();
    let registries = network
        .registry_data(known)
        .map(|body| {
            let packet = ClientboundRegistryData::decode(&mut &body[..])
                .expect("the load encodes registry data it can read");
            let entries = packet
                .entries
                .into_iter()
                .map(|entry| (entry.id.into(), entry.data.map(|data| data.into_owned())))
                .collect();
            (packet.registry.into(), entries)
        })
        .collect();
    let tags = ClientboundUpdateTags::decode(&mut network.tags())
        .expect("the load encodes tags it can read")
        .registries
        .into_iter()
        .map(|registry| {
            let tags = registry
                .tags
                .into_iter()
                .map(|tag| (tag.name.into(), tag.entries.iter().map(|id| id.0).collect()))
                .collect();
            (registry.registry.into(), tags)
        })
        .collect();
    (registries, tags)
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
