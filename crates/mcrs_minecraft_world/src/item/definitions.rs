use bevy_asset::AssetServer;
use bevy_asset::io::AssetSourceId;
use mcrs_minecraft_assets::asset::{CorpusReadError, read_json_corpus};
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_item::definition::schema::ItemDefinitionFile;
use mcrs_minecraft_item::definition::{CORPUS_DIRECTORY, FORMAT_VERSION};
use mcrs_minecraft_item::{ItemDefinitions, ItemEntry};
use mcrs_minecraft_registry::ItemId;

pub fn from_files(
    files: impl IntoIterator<Item = (String, Vec<u8>)>,
    blocks: &BlockDefinitions,
) -> Result<ItemDefinitions, ItemCorpusError> {
    let mut parsed: Vec<(String, ItemDefinitionFile)> = files
        .into_iter()
        .map(|(path, bytes)| {
            if bytes.is_empty() {
                return Err(ItemCorpusError::Empty { path });
            }
            let file: ItemDefinitionFile =
                serde_json::from_slice(&bytes).map_err(|source| ItemCorpusError::Parse {
                    path: path.clone(),
                    source,
                })?;
            if file.format_version != FORMAT_VERSION {
                return Err(ItemCorpusError::FormatVersion {
                    path,
                    found: file.format_version,
                });
            }
            Ok((path, file))
        })
        .collect::<Result<_, _>>()?;
    parsed.sort_by_key(|(_, file)| file.item.description.protocol_id);

    let mut entries = Vec::with_capacity(parsed.len());
    let mut paths = Vec::with_capacity(parsed.len());
    for (expected, (path, file)) in parsed.into_iter().enumerate() {
        let item = file.item;
        let found = item.description.protocol_id;
        if found as usize != expected {
            return Err(ItemCorpusError::ProtocolIds {
                expected,
                found,
                file: path,
            });
        }
        let id = ItemId(
            u16::try_from(found).map_err(|_| ItemCorpusError::ProtocolIds {
                expected,
                found,
                file: path.clone(),
            })?,
        );
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
            id,
            prototype: item.components,
            block_placer: placed.map(|block| block.default_state_id),
            container_slots: placed.and_then(|block| block.container_slots),
            crafting_remainder: item.crafting_remainder,
        });
        paths.push(path);
    }
    ItemDefinitions::from_entries(entries).map_err(|duplicate| {
        ItemCorpusError::DuplicateIdentifier {
            item: duplicate.identifier.as_str().to_owned(),
            file: paths.swap_remove(duplicate.second),
        }
    })
}

#[derive(Debug, thiserror::Error)]
pub enum ItemCorpusError {
    #[error("no default asset source")]
    NoAssetSource,
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
        expected: usize,
        found: u32,
        file: String,
    },
    #[error("`{item}` places `{block}`, which the block corpus lacks")]
    UnknownBlock { item: String, block: String },
    #[error("`{file}` redefines `{item}`")]
    DuplicateIdentifier { item: String, file: String },
}

pub fn load_item_definitions(
    asset_server: &AssetServer,
    blocks: &BlockDefinitions,
) -> Result<ItemDefinitions, ItemCorpusError> {
    let source = asset_server
        .get_source(AssetSourceId::Default)
        .map_err(|_| ItemCorpusError::NoAssetSource)?;
    let files = read_json_corpus(source.reader(), CORPUS_DIRECTORY)?;
    from_files(files, blocks)
}
