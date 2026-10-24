use std::fmt;
use std::sync::Arc;

use mcrs_minecraft_datapack::{
    AvailablePack, Features, Level, PackSelection, PackSource, ReadError, RootPacks, pack_stack,
    read_vanilla,
};
use mcrs_minecraft_registries::{Diagnostic, Diagnostics, Received, Registries, RegistryAdditions};

use crate::packs::DATAPACK_REPORT;
use crate::registries::declarations;

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
    Load(Diagnostics),
}

impl fmt::Display for RegistryLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryLoadError::Read(error) => write!(f, "cannot read {error}"),
            RegistryLoadError::Declarations(message) => f.write_str(message),
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

/// The registries of every pack of `source` and of `additions`, over the vanilla declarations
/// and those of `additions`.
pub fn load_registries(
    source: &dyn PackSource,
    additions: RegistryAdditions,
) -> Result<(Registries, Diagnostics), RegistryLoadError> {
    let RegistryAdditions {
        declarations: extra,
        embedded_packs,
        mut files,
    } = additions;
    let packs = RootPacks::read(source)?;
    let declarations = declarations(&source.read(DATAPACK_REPORT)?, extra)?;

    let mut selection = PackSelection::default();
    let mut available = packs.available().to_vec();
    for id in embedded_packs {
        if available.iter().any(|pack| pack.id() == id) {
            return Err(RegistryLoadError::Declarations(format!(
                "the embedded pack {id} has the id of another pack"
            )));
        }
        selection.enabled.push(id.clone());
        available.push(AvailablePack::embedded(id));
    }
    let (stack, found) = pack_stack(&selection, &available, &Features::vanilla(), false);
    let mut diagnostics = Diagnostics::default();
    let unavailable = packs.unavailable().iter().cloned();
    diagnostics.extend(found.into_iter().chain(unavailable).map(Diagnostic::Pack));
    packs.read_files(source, &stack, &mut files)?;

    match Registries::load(declarations, Arc::new(stack), files) {
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

/// The registries a client holds after a server's configuration, over the vanilla declarations.
/// An entry sent without data is read from the vanilla pack of `vanilla`, a pack root the client
/// holds, when the server negotiated that pack.
pub fn load_received(
    report: &[u8],
    received: &Received,
    vanilla: Option<&dyn PackSource>,
) -> Result<(Registries, Diagnostics), RegistryLoadError> {
    let declarations = declarations(report, Vec::new())?;
    let read = vanilla.map(|source| move |path: &str| read_vanilla(source, path));
    let known = read
        .as_ref()
        .map(|read| read as &dyn Fn(&str) -> Result<Vec<u8>, ReadError>);
    Registries::receive(declarations, received, known).map_err(RegistryLoadError::Load)
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

/// The additions of vanilla world generation, which the beta pack of the test packs needs.
#[cfg(all(feature = "test-support", not(target_family = "wasm")))]
pub fn test_additions() -> RegistryAdditions {
    let mut additions = RegistryAdditions::default();
    mcrs_minecraft_worldgen::registries::add_vanilla_worldgen(&mut additions);
    additions
}

#[cfg(all(feature = "test-support", not(target_family = "wasm")))]
#[expect(clippy::panic, reason = "test support reports a failure by panicking")]
static TEST_REGISTRIES: std::sync::LazyLock<Registries> = std::sync::LazyLock::new(|| {
    load_registries(&test_packs(), test_additions())
        .map(|(registries, _)| registries)
        .unwrap_or_else(|error| panic!("the registries are unusable:\n{error}"))
});

#[cfg(all(feature = "test-support", not(target_family = "wasm")))]
pub fn test_registries() -> &'static Registries {
    &TEST_REGISTRIES
}

#[cfg(all(feature = "test-support", not(target_family = "wasm")))]
#[expect(clippy::panic, reason = "test support reports a failure by panicking")]
static TEST_NETWORK: std::sync::LazyLock<mcrs_minecraft_registries::NetworkSnapshot> =
    std::sync::LazyLock::new(|| {
        mcrs_minecraft_registries::NetworkSnapshot::new(&TEST_REGISTRIES)
            .unwrap_or_else(|error| panic!("the registries cannot be sent:\n{error}"))
    });

/// What the configuration phase sends of [`test_registries`].
#[cfg(all(feature = "test-support", not(target_family = "wasm")))]
pub fn test_network() -> &'static mcrs_minecraft_registries::NetworkSnapshot {
    &TEST_NETWORK
}

/// What a client collects from the payloads `registries` encoded; entries of the vanilla pack go
/// without data when the client is `known` to hold it.
#[cfg(all(feature = "test-support", not(target_family = "wasm")))]
#[expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test support reports a failure by panicking"
)]
pub fn sent_by(registries: &Registries, known: bool) -> Received {
    use mcrs_minecraft_protocol::Decode;
    use mcrs_minecraft_protocol::packets::configuration::{
        ClientboundRegistryData, ClientboundUpdateTags,
    };

    let network = mcrs_minecraft_registries::NetworkSnapshot::new(registries)
        .unwrap_or_else(|error| panic!("the registries cannot be sent:\n{error}"));
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
    Received { registries, tags }
}

/// Registries over `declarations` alone: vanilla is the only pack and it holds no files.
#[cfg(all(feature = "test-support", not(target_family = "wasm")))]
#[expect(clippy::panic, reason = "test support reports a failure by panicking")]
pub fn registries_declaring(
    declarations: Vec<mcrs_minecraft_registries::Declaration>,
) -> Registries {
    let (stack, _) = pack_stack(
        &PackSelection::default(),
        RootPacks::vanilla().available(),
        &Features::vanilla(),
        false,
    );
    Registries::load(
        declarations,
        Arc::new(stack),
        mcrs_minecraft_datapack::PackFiles::default(),
    )
    .map(|(registries, _)| registries)
    .unwrap_or_else(|diagnostics| panic!("the declarations do not load:\n{diagnostics}"))
}
