use bevy_asset::AssetServer;
use bevy_asset::io::AssetSourceId;
use mcrs_minecraft_assets::asset::{CorpusReadError, read_json_corpus};
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_game_registries::item::definitions::{ItemCorpusError, from_files};
use mcrs_minecraft_item::ItemDefinitions;
use mcrs_minecraft_item::definition::CORPUS_DIRECTORY;
use mcrs_minecraft_item_table::{BakeError, DefinitionFile, ItemTable, bake};
use mcrs_minecraft_registries::Registries;

use crate::registry_names::RegistryNames;

/// Why the item definitions did not load through the asset server.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ItemLoadError {
    /// The app registered no default asset source.
    #[error("no default asset source")]
    NoAssetSource,
    /// The corpus could not be read.
    #[error(transparent)]
    Corpus(#[from] CorpusReadError),
    /// The corpus was read but does not make item definitions.
    #[error(transparent)]
    Definitions(#[from] ItemCorpusError),
    /// The corpus was read but does not bake into an item table.
    #[error(transparent)]
    Table(#[from] BakeError),
}

/// The item definitions of the corpus and the table of the item prototypes that the same files
/// bake into.
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

/// The item definitions of the corpus the default asset source serves.
pub fn load_item_definitions(
    asset_server: &AssetServer,
    registries: &Registries,
    blocks: &BlockDefinitions,
) -> Result<ItemDefinitions, ItemLoadError> {
    Ok(from_files(read_corpus(asset_server)?, registries, blocks)?)
}

/// The item definitions and the item table of the corpus the default asset source serves. The
/// corpus is read once.
pub fn load_items(
    asset_server: &AssetServer,
    registries: &Registries,
    blocks: &BlockDefinitions,
) -> Result<LoadedItems, ItemLoadError> {
    let files = read_corpus(asset_server)?;
    let baked = bake(
        files
            .iter()
            .map(|(name, bytes)| DefinitionFile { name, bytes }),
        &RegistryNames::new(registries),
    )?;
    Ok(LoadedItems {
        definitions: from_files(files, registries, blocks)?,
        table: baked.table,
    })
}
