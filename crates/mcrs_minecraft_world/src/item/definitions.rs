#[cfg(feature = "bevy")]
use bevy_asset::AssetServer;
#[cfg(feature = "bevy")]
use bevy_asset::io::AssetSourceId;
#[cfg(feature = "bevy")]
use mcrs_minecraft_assets::asset::{CorpusReadError, read_json_corpus};
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_core::ResourceKey;
#[cfg(feature = "bevy")]
use mcrs_minecraft_item::definition::CORPUS_DIRECTORY;
use mcrs_minecraft_item::definition::FORMAT_VERSION;
use mcrs_minecraft_item::definition::schema::ItemDefinitionFile;
use mcrs_minecraft_item::keys::Item;
use mcrs_minecraft_item::{ItemDefinitions, ItemEntry, ItemTableError};
use mcrs_minecraft_registry::RegistrySet;

pub fn from_files(
    files: impl IntoIterator<Item = (String, Vec<u8>)>,
    registries: &RegistrySet,
    blocks: &BlockDefinitions,
) -> Result<ItemDefinitions, ItemCorpusError> {
    let items = registries
        .registry::<Item>()
        .ok_or(ItemCorpusError::NoItemRegistry)?;
    let mut entries = Vec::new();
    let mut paths = Vec::new();
    for (path, bytes) in files {
        if bytes.is_empty() {
            return Err(ItemCorpusError::Empty { path });
        }
        let file: ItemDefinitionFile = registries
            .scope(|| serde_json::from_slice(&bytes))
            .map_err(|source| ItemCorpusError::Parse {
                path: path.clone(),
                source,
            })?;
        if file.format_version != FORMAT_VERSION {
            return Err(ItemCorpusError::FormatVersion {
                path,
                found: file.format_version,
            });
        }
        let item = file.item;
        let found = item.description.protocol_id;
        let reported = items
            .require(&ResourceKey::from_location(
                item.description.identifier.clone(),
            ))
            .map_err(ItemTableError::from)?;
        if found != reported.number() {
            return Err(ItemCorpusError::ProtocolIds {
                expected: reported.number(),
                found,
                file: path,
            });
        }
        let placed = item
            .block_placer
            .map(|block| {
                blocks
                    .block(block.as_str())
                    .ok_or_else(|| ItemCorpusError::UnknownBlock {
                        item: item.description.identifier.as_str().to_owned(),
                        block: block.as_str().to_owned(),
                    })
            })
            .transpose()?;
        entries.push(ItemEntry {
            identifier: item.description.identifier,
            id: Item::Air.id(),
            prototype: item.components,
            block_placer: placed.map(|block| block.default_state_id),
            container_slots: placed.and_then(|block| block.container_slots),
            crafting_remainder: item.crafting_remainder,
        });
        paths.push(path);
    }
    ItemDefinitions::from_entries(&items, entries).map_err(|error| match error {
        ItemTableError::Duplicate(duplicate) => ItemCorpusError::DuplicateIdentifier {
            item: duplicate.identifier.as_str().to_owned(),
            file: paths.swap_remove(duplicate.second),
        },
        other => ItemCorpusError::Table(other),
    })
}

#[derive(Debug, thiserror::Error)]
pub enum ItemCorpusError {
    #[cfg(feature = "bevy")]
    #[error("no default asset source")]
    NoAssetSource,
    #[error("the registry set holds no item registry")]
    NoItemRegistry,
    #[cfg(feature = "bevy")]
    #[error(transparent)]
    Corpus(#[from] CorpusReadError),
    #[error("`{path}` read as zero bytes")]
    Empty { path: String },
    #[error("failed to parse `{path}`: {source}")]
    Parse {
        path: String,
        source: serde_json::Error,
    },
    #[error("`{path}` has format_version `{found}`, expected `{FORMAT_VERSION}`")]
    FormatVersion { path: String, found: String },
    #[error("`{file}` has protocol_id {found} where {expected} was expected")]
    ProtocolIds {
        expected: u16,
        found: u16,
        file: String,
    },
    #[error(transparent)]
    Table(#[from] ItemTableError),
    #[error("`{item}` places `{block}`, which the block corpus lacks")]
    UnknownBlock { item: String, block: String },
    #[error("`{file}` redefines `{item}`")]
    DuplicateIdentifier { item: String, file: String },
}

#[cfg(feature = "bevy")]
pub fn load_item_definitions(
    asset_server: &AssetServer,
    registries: &RegistrySet,
    blocks: &BlockDefinitions,
) -> Result<ItemDefinitions, ItemCorpusError> {
    let source = asset_server
        .get_source(AssetSourceId::Default)
        .map_err(|_| ItemCorpusError::NoAssetSource)?;
    let files = read_json_corpus(source.reader(), CORPUS_DIRECTORY)?;
    from_files(files, registries, blocks)
}
