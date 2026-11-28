use std::sync::Arc;

use mcrs_minecraft_datapack::{
    AvailablePack, Features, PackSelection, PackSource, ReadError, RootPacks, pack_stack,
    read_vanilla,
};
use mcrs_minecraft_registries::{
    ContentPaths, Diagnostic, Diagnostics, Received, Registries, RegistryAdditions,
};

use crate::packs::DATAPACK_REPORT;
use crate::registries::declarations;
pub use crate::registry_load_error::RegistryLoadError;

const FILE_PREFIX: &str = "file/";

pub fn load_registries(
    source: &dyn PackSource,
    additions: RegistryAdditions,
) -> Result<(Registries, Diagnostics), RegistryLoadError> {
    let RegistryAdditions {
        declarations: extra,
        embedded_packs,
        mut files,
        local_packs,
    } = additions;
    let packs = RootPacks::read(source)?;
    let declarations = declarations(&source.read(DATAPACK_REPORT)?, extra)?;

    let standard_packs = source.standard_packs();
    if let Some(name) = standard_packs
        .iter()
        .find(|name| local_packs.contains(name))
    {
        return Err(RegistryLoadError::Declarations(format!(
            "the extra pack {name} has the name of a local pack"
        )));
    }
    let standard = |name: &str| standard_packs.iter().any(|standard| standard == name);
    let local = |name: &str| local_packs.iter().any(|local| local == name);
    let mut selection = PackSelection::default();
    let mut available: Vec<AvailablePack> = packs
        .available()
        .iter()
        .filter(|pack| {
            (pack.id().strip_prefix(FILE_PREFIX)).is_none_or(|name| local(name) || standard(name))
        })
        .cloned()
        .collect();
    for id in embedded_packs {
        if available.iter().any(|pack| pack.id() == id) {
            return Err(RegistryLoadError::Declarations(format!(
                "the embedded pack {id} has the id of another pack"
            )));
        }
        selection.enabled.push(id.clone());
        available.push(AvailablePack::embedded(id));
    }
    if !standard_packs.is_empty() {
        let mut local_ids: Vec<&str> = (available.iter())
            .map(AvailablePack::id)
            .filter(|id| {
                id.strip_prefix(FILE_PREFIX)
                    .is_some_and(|name| !standard(name))
            })
            .collect();
        local_ids.sort_unstable();
        let standard_ids = standard_packs
            .iter()
            .map(|name| format!("{FILE_PREFIX}{name}"))
            .filter(|id| available.iter().any(|pack| pack.id() == id));
        let enabled = local_ids.into_iter().map(str::to_owned).chain(standard_ids);
        selection.enabled.extend(enabled);
    }
    let (stack, found) = pack_stack(&selection, &available, &Features::vanilla(), false);
    let mut diagnostics = Diagnostics::default();
    let unavailable = packs.unavailable().iter().cloned();
    diagnostics.extend(found.into_iter().chain(unavailable).map(Diagnostic::Pack));
    let content = ContentPaths::new(&declarations);
    packs.read_files(source, &stack, &mut files, ".json", |path| {
        content.needs(path)
    })?;

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

#[cfg(all(feature = "test-support", not(target_family = "wasm")))]
pub fn test_packs() -> mcrs_minecraft_datapack::DirectoryPacks {
    let assets = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    mcrs_minecraft_datapack::DirectoryPacks::new(
        assets,
        mcrs_minecraft_fixtures::vanilla_dir().to_owned(),
    )
}

#[cfg(all(feature = "test-support", not(target_family = "wasm")))]
#[expect(clippy::panic, reason = "test support reports a failure by panicking")]
pub fn test_additions() -> RegistryAdditions {
    let mut additions = RegistryAdditions::default();
    additions
        .local_packs
        .extend(mcrs_minecraft_datapack::REPOSITORY_PACKS.map(str::to_owned));
    mcrs_minecraft_worldgen::registries::add_vanilla_worldgen(&mut additions)
        .unwrap_or_else(|error| panic!("{error}"));
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

#[cfg(all(feature = "test-support", not(target_family = "wasm")))]
pub fn test_network() -> &'static mcrs_minecraft_registries::NetworkSnapshot {
    &TEST_NETWORK
}

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
