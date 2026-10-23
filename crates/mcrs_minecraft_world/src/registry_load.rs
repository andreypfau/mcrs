use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::sync::Arc;

use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_datapack::{
    Features, Level, PackFiles, PackSelection, PackSource, ReadError, RootPacks, VANILLA_PACK,
    pack_stack,
};
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::Decode;
use mcrs_minecraft_protocol::packets::configuration::ClientboundRegistryData;
use mcrs_minecraft_registries::{
    DataValue, Diagnostic, Diagnostics, Received, Registries, insert_entry, load,
};
use mcrs_minecraft_worldgen_builtin as builtin;
use mcrs_minecraft_worldgen_density::proto::DensityFunctionHolder;
use mcrs_minecraft_worldgen_density::router::NoiseGeneratorSettings;
use mcrs_minecraft_worldgen_feature::pool::TemplatePool;
use mcrs_minecraft_worldgen_noise::proto::NoiseParam;

use crate::declarations;
use crate::packs::DATAPACK_REPORT;

#[cfg(feature = "bevy")]
pub(crate) struct AssetReaderPacks<'a>(pub &'a dyn bevy_asset::io::ErasedAssetReader);

#[cfg(feature = "bevy")]
impl PackSource for AssetReaderPacks<'_> {
    fn read(&self, path: &str) -> Result<Vec<u8>, ReadError> {
        bevy_tasks::block_on(mcrs_minecraft_assets::asset::read_whole(
            self.0,
            std::path::Path::new(path),
        ))
        .map_err(|error| ReadError::new(path, error))
    }

    fn list(
        &self,
        directory: &str,
    ) -> Result<Vec<mcrs_minecraft_datapack::DirectoryEntry>, ReadError> {
        use bevy_asset::io::AssetReaderError;
        use bevy_tasks::futures_lite::StreamExt;
        bevy_tasks::block_on(async {
            let mut paths = match self.0.read_directory(std::path::Path::new(directory)).await {
                Ok(paths) => paths,
                Err(AssetReaderError::NotFound(_)) => return Ok(Vec::new()),
                Err(error) => return Err(ReadError::new(directory, error)),
            };
            let mut listed = Vec::new();
            while let Some(path) = paths.next().await {
                let directory = self
                    .0
                    .is_directory(&path)
                    .await
                    .map_err(|error| ReadError::new(path.display().to_string(), error))?;
                if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                    listed.push(mcrs_minecraft_datapack::DirectoryEntry {
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
    Read(ReadError),
    Declarations(String),
    Packet(String),
    Load(Diagnostics),
}

impl fmt::Display for RegistryLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryLoadError::Read(error) => write!(f, "cannot read {error}"),
            RegistryLoadError::Declarations(message) | RegistryLoadError::Packet(message) => {
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

impl From<ReadError> for RegistryLoadError {
    fn from(error: ReadError) -> Self {
        RegistryLoadError::Read(error)
    }
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

pub fn load_registries(
    source: &dyn PackSource,
) -> Result<(Registries, Diagnostics), RegistryLoadError> {
    load_stack(source, RootPacks::read(source)?)
}

pub fn load_vanilla_registries(
    source: &dyn PackSource,
) -> Result<(Registries, Diagnostics), RegistryLoadError> {
    load_stack(source, RootPacks::vanilla())
}

fn load_stack(
    source: &dyn PackSource,
    packs: RootPacks,
) -> Result<(Registries, Diagnostics), RegistryLoadError> {
    let report = source.read(DATAPACK_REPORT)?;
    let declarations = declarations::plan(&report).map_err(RegistryLoadError::Declarations)?;

    let (stack, found) = pack_stack(
        &PackSelection::default(),
        packs.available(),
        &Features::vanilla(),
        false,
    );
    let mut diagnostics = Diagnostics::default();
    let unavailable = packs.unavailable().iter().cloned();
    diagnostics.extend(found.into_iter().chain(unavailable).map(Diagnostic::Pack));
    let mut files = PackFiles::default();
    insert_built_ins(&mut files);
    packs.read_files(source, &stack, &mut files)?;

    match load(&declarations, Arc::new(stack), files, None) {
        Ok((registries, loaded)) => {
            diagnostics.extend(loaded);
            Ok((registries, diagnostics))
        }
        Err(failed) => {
            diagnostics.extend(failed);
            Err(RegistryLoadError::Load(diagnostics))
        }
    }
}

const BETA_PACK: &str = "file/beta";

fn insert_built_ins(files: &mut PackFiles) {
    for name in builtin::biome_names() {
        if let Some(biome) = builtin::biome(&name) {
            insert_entry::<BiomeFile>(files, VANILLA_PACK, &name, biome);
        }
    }
    insert_all::<TemplatePool>(files, VANILLA_PACK, builtin::template_pools());
    insert_all::<NoiseParam>(files, VANILLA_PACK, builtin::vanilla_noises());
    insert_all::<NoiseParam>(files, BETA_PACK, builtin::beta_noises());
    insert_all::<DensityFunctionHolder>(files, VANILLA_PACK, builtin::vanilla_density_functions());
    insert_all::<DensityFunctionHolder>(files, BETA_PACK, builtin::beta_density_functions());
    insert_all::<NoiseGeneratorSettings>(files, VANILLA_PACK, builtin::vanilla_noise_settings());
    insert_all::<NoiseGeneratorSettings>(files, BETA_PACK, builtin::beta_noise_settings());
}

fn insert_all<V: DataValue>(
    files: &mut PackFiles,
    pack: &str,
    values: BTreeMap<ResourceLocation, V::Named>,
) where
    V::Named: Send + Sync,
{
    for (name, value) in values {
        insert_entry::<V>(files, pack, &name, value);
    }
}

/// The network form of every synced entry of a load: what a server that holds the same pack
/// would send. A client fills an entry the server sent without data from it.
pub struct KnownEntries(HashMap<(ResourceLocation, ResourceLocation), NbtTag>);

impl KnownEntries {
    pub fn of(registries: &Registries) -> Result<Self, RegistryLoadError> {
        let mut entries = HashMap::new();
        for body in registries.network().registry_data(false) {
            let packet = ClientboundRegistryData::decode(&mut &body[..])
                .map_err(|error| RegistryLoadError::Packet(error.to_string()))?;
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
) -> Result<(Registries, Diagnostics), RegistryLoadError> {
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
    mcrs_minecraft_registries::load_received(&declarations, &received)
        .map_err(RegistryLoadError::Load)
}

/// The repository's assets over the pinned client jar's data pack, which must be installed.
#[cfg(all(feature = "test-support", not(target_family = "wasm")))]
pub fn test_packs() -> mcrs_minecraft_datapack::DirectoryPacks {
    let assets = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    mcrs_minecraft_datapack::DirectoryPacks::new(
        assets,
        mcrs_minecraft_fixtures::vanilla_dir().to_owned(),
    )
}

#[cfg(all(feature = "test-support", not(target_family = "wasm")))]
static TEST_REGISTRIES: std::sync::LazyLock<Registries> = std::sync::LazyLock::new(|| {
    load_registries(&test_packs())
        .map(|(registries, _)| registries)
        .unwrap_or_else(|error| panic!("the registries are unusable:\n{error}"))
});

#[cfg(all(feature = "test-support", not(target_family = "wasm")))]
pub fn test_registries() -> &'static Registries {
    &TEST_REGISTRIES
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
    let (stack, _) = pack_stack(
        &PackSelection::default(),
        RootPacks::vanilla().available(),
        &Features::vanilla(),
        false,
    );
    mcrs_minecraft_registries::load(declarations, Arc::new(stack), PackFiles::default(), None)
        .map(|(registries, _)| registries)
        .unwrap_or_else(|diagnostics| panic!("the declarations do not load:\n{diagnostics}"))
}
