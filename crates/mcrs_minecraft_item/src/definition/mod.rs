pub mod schema;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use bevy_asset::AssetServer;
use bevy_asset::io::{AssetReaderError, AssetSourceId};
use bevy_ecs::resource::Resource;
use bevy_tasks::block_on;
use bevy_tasks::futures_lite::StreamExt;
use mcrs_minecraft_assets::asset::read_whole;
use mcrs_minecraft_assets::tag::registry::TagSource;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_protocol::item::{ComponentMap, Template};
use mcrs_minecraft_registry::{BlockStateId, ItemId};
use rustc_hash::FxHashMap;

use self::schema::ItemDefinitionFile;

pub const CORPUS_DIRECTORY: &str = "mcrs/item_definition";
pub const FORMAT_VERSION: &str = "1.21.130";

#[derive(Debug)]
pub struct ItemEntry {
    pub identifier: ResourceLocation<Arc<str>>,
    pub id: ItemId,
    pub prototype: ComponentMap,
    pub block_placer: Option<BlockStateId>,
    pub container_slots: Option<u8>,
    pub crafting_remainder: Option<Template>,
}

#[derive(Debug, Default)]
pub struct ItemDefinitions {
    entries: Vec<ItemEntry>,
    by_identifier: FxHashMap<ResourceLocation<Arc<str>>, ItemId>,
}

impl ItemDefinitions {
    pub fn get(&self, id: ItemId) -> Option<&ItemEntry> {
        self.entries.get(id.0 as usize)
    }

    pub fn id_of(&self, location: &str) -> Option<ItemId> {
        self.by_identifier.get(location).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = &ItemEntry> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn from_files(
        files: impl IntoIterator<Item = (String, Vec<u8>)>,
        blocks: &BlockDefinitions,
    ) -> Result<Self, ItemCorpusError> {
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

        let mut definitions = ItemDefinitions::default();
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
            let id = ItemId(u16::try_from(found).map_err(|_| ItemCorpusError::ProtocolIds {
                expected,
                found,
                file: path.clone(),
            })?);
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
            if definitions
                .by_identifier
                .insert(item.description.identifier.clone(), id)
                .is_some()
            {
                return Err(ItemCorpusError::DuplicateIdentifier {
                    item: item.description.identifier.as_str().to_owned(),
                    file: path,
                });
            }
            definitions.entries.push(ItemEntry {
                identifier: item.description.identifier,
                id,
                prototype: item.components,
                block_placer: placed.map(|block| block.default_state_id),
                container_slots: placed.and_then(|block| block.container_slots),
                crafting_remainder: item.crafting_remainder,
            });
        }
        Ok(definitions)
    }
}

#[derive(Resource, Clone, Debug)]
pub struct Items(pub Arc<ItemDefinitions>);

impl std::ops::Deref for Items {
    type Target = ItemDefinitions;

    fn deref(&self) -> &ItemDefinitions {
        &self.0
    }
}

impl TagSource for Items {
    type Id = u32;

    fn id_of(&self, loc: &str) -> Option<u32> {
        ItemDefinitions::id_of(self, loc).map(|id| u32::from(id.0))
    }

    fn capacity(&self) -> u32 {
        self.len() as u32
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ItemCorpusError {
    #[error("no default asset source")]
    NoAssetSource,
    #[error("cannot list `{directory}`: {source}")]
    ListDirectory {
        directory: String,
        source: AssetReaderError,
    },
    #[error("cannot read `{path}`: {source}")]
    Read {
        path: String,
        source: AssetReaderError,
    },
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
    let reader = source.reader();

    let mut paths = block_on(async {
        let mut stream = reader
            .read_directory(Path::new(CORPUS_DIRECTORY))
            .await
            .map_err(|source| ItemCorpusError::ListDirectory {
                directory: CORPUS_DIRECTORY.into(),
                source,
            })?;
        let mut paths = Vec::new();
        while let Some(path) = stream.next().await {
            if path.extension().is_some_and(|e| e == "json") {
                paths.push(path);
            }
        }
        Ok::<Vec<PathBuf>, ItemCorpusError>(paths)
    })?;
    paths.sort();

    let files = block_on(async {
        let mut files = Vec::with_capacity(paths.len());
        for path in &paths {
            let display = path.display().to_string();
            let bytes = read_whole(reader, path)
                .await
                .map_err(|source| ItemCorpusError::Read {
                    path: display.clone(),
                    source,
                })?;
            files.push((display, bytes));
        }
        Ok::<Vec<(String, Vec<u8>)>, ItemCorpusError>(files)
    })?;

    ItemDefinitions::from_files(files, blocks)
}
