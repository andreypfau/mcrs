use bevy_asset::AssetServer;
use bevy_asset::io::AssetSourceId;
use mcrs_minecraft_assets::asset::{CorpusReadError, read_json_corpus};
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_item::ItemDefinitions;
use mcrs_minecraft_item::definition::CORPUS_DIRECTORY;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_world::item::definitions::{ItemCorpusError, from_files};

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
}

/// The item definitions of the corpus the default asset source serves.
pub fn load_item_definitions(
    asset_server: &AssetServer,
    registries: &Registries,
    blocks: &BlockDefinitions,
) -> Result<ItemDefinitions, ItemLoadError> {
    let source = asset_server
        .get_source(AssetSourceId::Default)
        .map_err(|_| ItemLoadError::NoAssetSource)?;
    let files = read_json_corpus(source.reader(), CORPUS_DIRECTORY)?;
    Ok(from_files(files, registries, blocks)?)
}
