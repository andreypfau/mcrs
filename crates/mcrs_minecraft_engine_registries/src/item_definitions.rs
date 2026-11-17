use bevy_asset::AssetServer;
use bevy_asset::io::AssetSourceId;
use mcrs_minecraft_assets::asset::{CorpusReadError, read_json_corpus};
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_game_registries::item::definitions::{ItemCorpusError, from_files};
use mcrs_minecraft_item::ItemDefinitions;
use mcrs_minecraft_item::definition::CORPUS_DIRECTORY;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_table::{BakeError, DefinitionFile, ItemTable, bake};
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::ResourceLocation;

use crate::registry_names::RegistryNames;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ItemLoadError {
    #[error("no default asset source")]
    NoAssetSource,
    #[error(transparent)]
    Corpus(#[from] CorpusReadError),
    #[error(transparent)]
    Definitions(#[from] ItemCorpusError),
    #[error(transparent)]
    Table(#[from] BakeError),
    #[error("item {id} of the item corpus is not item {id} of the item registry")]
    ItemIdMismatch { id: u16 },
}

pub struct LoadedItems {
    pub definitions: ItemDefinitions,
    pub table: ItemTable,
}

fn read_corpus(asset_server: &AssetServer) -> Result<Vec<(String, Vec<u8>)>, ItemLoadError> {
    let source = asset_server
        .get_source(AssetSourceId::Default)
        .map_err(|_| ItemLoadError::NoAssetSource)?;
    Ok(read_json_corpus(source.reader(), CORPUS_DIRECTORY)?)
}

pub fn load_item_definitions(
    asset_server: &AssetServer,
    registries: &Registries,
    blocks: &BlockDefinitions,
) -> Result<ItemDefinitions, ItemLoadError> {
    Ok(from_files(read_corpus(asset_server)?, registries, blocks)?)
}

pub fn load_items(
    asset_server: &AssetServer,
    registries: &Registries,
    blocks: &BlockDefinitions,
) -> Result<LoadedItems, ItemLoadError> {
    items_from_files(read_corpus(asset_server)?, registries, blocks)
}

pub(crate) fn items_from_files(
    files: Vec<(String, Vec<u8>)>,
    registries: &Registries,
    blocks: &BlockDefinitions,
) -> Result<LoadedItems, ItemLoadError> {
    let baked = bake(
        files
            .iter()
            .map(|(name, bytes)| DefinitionFile { name, bytes }),
        &RegistryNames::without_items(registries),
    )?;
    check_item_ids(&baked.table, registries)?;
    Ok(LoadedItems {
        definitions: from_files(files, registries, blocks)?,
        table: baked.table,
    })
}

fn check_item_ids(table: &ItemTable, registries: &Registries) -> Result<(), ItemLoadError> {
    let registry: &[_] = ResourceLocation::read("minecraft:item")
        .ok()
        .and_then(|key| registries.names_of(key))
        .unwrap_or_default();
    for id in 0..=u16::MAX {
        let corpus = table.name(NameTable::Item, id);
        let registered = registry.get(usize::from(id)).map(|name| name.as_location());
        if corpus != registered {
            return Err(ItemLoadError::ItemIdMismatch { id });
        }
        if corpus.is_none() {
            return Ok(());
        }
    }
    Ok(())
}
