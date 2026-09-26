use std::sync::Arc;

use mcrs_minecraft_chunk::VoxelId;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LightType(pub u8);

impl LightType {
    pub const DEFAULT: LightType = LightType(0);
}

/// Colour `i` belongs to light type `i + 1`; type 0 is the default and has no
/// colour here because the dimension's block light tint is applied at shading.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::resource::Resource))]
pub struct LightColors {
    colours: Arc<[[u8; 3]]>,
    types: Arc<[LightType]>,
}

impl LightColors {
    pub fn new(colours: impl Into<Arc<[[u8; 3]]>>, types: impl Into<Arc<[LightType]>>) -> Self {
        let colours = colours.into();
        let types = types.into();
        assert!(
            colours.len() <= u8::MAX as usize,
            "more than 255 light colours"
        );
        assert!(
            types.iter().all(|t| t.0 as usize <= colours.len()),
            "a state names a light type past the colour table"
        );
        LightColors { colours, types }
    }

    /// States past the table, such as the light registry's filler ids for
    /// unloaded and out-of-world space, are the default type.
    pub fn light_type(&self, state: VoxelId) -> LightType {
        self.types
            .get(state.0 as usize)
            .copied()
            .unwrap_or(LightType::DEFAULT)
    }

    pub fn rgb(&self, t: LightType) -> Option<[u8; 3]> {
        (t.0 as usize)
            .checked_sub(1)
            .and_then(|i| self.colours.get(i))
            .copied()
    }

    /// Every type including the default.
    pub fn type_count(&self) -> usize {
        self.colours.len() + 1
    }
}

#[cfg(feature = "bevy")]
pub use self::load::{CORPUS_DIRECTORY, LightColorError};

#[cfg(feature = "bevy")]
mod load {
    use bevy_asset::AssetServer;
    use bevy_asset::io::AssetSourceId;
    use mcrs_minecraft_assets::asset::{CorpusReadError, read_json_corpus};
    use mcrs_minecraft_assets::tag::DynTagRegistry;
    use mcrs_minecraft_block::Block;
    use mcrs_minecraft_block::definition::{BlockDefinitions, BlockEntry};
    use mcrs_minecraft_core::tag_key::TagKey;
    use mcrs_minecraft_registry::BlockStateId;

    use super::{LightColors, LightType};
    use crate::asset::{BlockStateRef, LightColorFile, StateTarget};

    pub const CORPUS_DIRECTORY: &str = "mcrs/light_color";

    #[derive(Debug, thiserror::Error)]
    pub enum LightColorError {
        #[error("the default asset source is missing")]
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
        #[error("`{path}`: `{entry}` names a block the corpus lacks")]
        UnknownBlock { path: String, entry: String },
    }

    impl LightColors {
        pub fn load(
            asset_server: &AssetServer,
            blocks: &BlockDefinitions,
            tags: &DynTagRegistry<Block>,
        ) -> Result<Self, LightColorError> {
            let source = asset_server
                .get_source(AssetSourceId::Default)
                .map_err(|_| LightColorError::NoAssetSource)?;
            let files = read_json_corpus(source.reader(), CORPUS_DIRECTORY)?;
            Self::from_files(files, blocks, tags)
        }

        /// Colours are numbered from 1 in the order the files come.
        pub fn from_files(
            files: impl IntoIterator<Item = (String, Vec<u8>)>,
            blocks: &BlockDefinitions,
            tags: &DynTagRegistry<Block>,
        ) -> Result<Self, LightColorError> {
            let mut colours = Vec::new();
            let mut types = vec![LightType::DEFAULT; blocks.state_count()];
            for (path, bytes) in files {
                if bytes.is_empty() {
                    return Err(LightColorError::Empty { path });
                }
                let file: LightColorFile =
                    serde_json::from_slice(&bytes).map_err(|source| LightColorError::Parse {
                        path: path.clone(),
                        source,
                    })?;
                colours.push(file.color.0);
                let light_type = LightType(colours.len() as u8);
                for entry in &file.blocks {
                    for block in members(entry, blocks, tags, &path)? {
                        for id in states(block) {
                            if matches(block, id, entry) {
                                types[id.0 as usize] = light_type;
                            }
                        }
                    }
                }
            }
            Ok(LightColors::new(colours, types))
        }
    }

    fn members<'a>(
        entry: &BlockStateRef,
        blocks: &'a BlockDefinitions,
        tags: &DynTagRegistry<Block>,
        path: &str,
    ) -> Result<Vec<&'a BlockEntry>, LightColorError> {
        match &entry.target {
            StateTarget::Block(id) => blocks
                .block(id.as_str())
                .map(|block| vec![block])
                .ok_or_else(|| LightColorError::UnknownBlock {
                    path: path.to_owned(),
                    entry: entry.to_string(),
                }),
            StateTarget::Tag(id) => Ok(tags
                .get(&TagKey::<Block, _>::from_location(id.clone()))
                .map(|set| set.iter().map(|i| &blocks.blocks()[i as usize]).collect())
                .unwrap_or_default()),
        }
    }

    fn states(block: &BlockEntry) -> impl Iterator<Item = BlockStateId> + '_ {
        (0..block.state_count).map(|offset| BlockStateId(block.base_state_id.0 + offset))
    }

    fn matches(block: &BlockEntry, id: BlockStateId, entry: &BlockStateRef) -> bool {
        entry.properties.iter().all(|(name, text)| {
            block
                .value_of(id, name)
                .is_some_and(|value| value.renders_to(text))
        })
    }
}
