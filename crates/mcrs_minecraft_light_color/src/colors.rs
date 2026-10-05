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
    use mcrs_minecraft_block::definition::{BlockDefinitions, BlockEntry};
    use mcrs_minecraft_core::tag_key::TagKey;
    use mcrs_minecraft_keys::Block;
    use mcrs_minecraft_registry::{BlockStateId, Tags};

    use super::{LightColors, LightType};
    use crate::asset::{BlockStateRef, LightColorFile, StateTarget};

    pub const CORPUS_DIRECTORY: &str = "mcrs/light_color";

    #[derive(Debug, thiserror::Error)]
    pub enum LightColorError {
        #[error("the default asset source is missing")]
        NoAssetSource,
        #[error(transparent)]
        Corpus(#[from] CorpusReadError),
        #[error("{count} light colour files, but at most 255 fit a light type")]
        TooManyColours { count: usize },
        #[error("`{path}` read as zero bytes")]
        Empty { path: String },
        #[error("failed to parse `{path}`: {source}")]
        Parse {
            path: String,
            source: serde_json::Error,
        },
        #[error("`{path}`: `{entry}` names a block the corpus lacks")]
        UnknownBlock { path: String, entry: String },
        #[error("`{path}`: `{entry}` names a block tag that does not exist")]
        UnknownTag { path: String, entry: String },
        #[error("`{path}`: `{entry}` states a property `{block}` does not declare")]
        UnknownProperty {
            path: String,
            entry: String,
            block: String,
        },
        #[error("`{path}`: `{entry}` states a value `{block}` does not declare")]
        UnknownValue {
            path: String,
            entry: String,
            block: String,
        },
        #[error("`{path}`: `{entry}` matches no state that emits light")]
        NoEmittingState { path: String, entry: String },
        #[error("`{state}` is coloured by both {first} and {second}")]
        TwoColours {
            state: String,
            first: String,
            second: String,
        },
    }

    impl LightColors {
        pub fn load(
            asset_server: &AssetServer,
            blocks: &BlockDefinitions,
            tags: &Tags<Block>,
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
            tags: &Tags<Block>,
        ) -> Result<Self, LightColorError> {
            let files: Vec<_> = files.into_iter().collect();
            if files.len() > u8::MAX as usize {
                return Err(LightColorError::TooManyColours { count: files.len() });
            }
            let mut colours = Vec::with_capacity(files.len());
            let mut types = vec![LightType::DEFAULT; blocks.state_count()];
            let mut claimants: Vec<String> = Vec::new();
            let mut claimed_by: Vec<Option<usize>> = vec![None; blocks.state_count()];
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
                    let members = members(entry, blocks, tags, &path)?;
                    check_predicate(entry, &members, &path)?;
                    let claimant = claimants.len();
                    claimants.push(format!("`{entry}` in `{path}`"));
                    let mut emits = false;
                    for block in members {
                        for id in states(block).filter(|&id| matches(block, id, entry)) {
                            let slot = id.0 as usize;
                            if let Some(first) = claimed_by[slot] {
                                return Err(LightColorError::TwoColours {
                                    state: describe(block, id),
                                    first: claimants[first].clone(),
                                    second: claimants[claimant].clone(),
                                });
                            }
                            claimed_by[slot] = Some(claimant);
                            types[slot] = light_type;
                            emits |= blocks.state(id).light_emission > 0;
                        }
                    }
                    if !emits {
                        return Err(LightColorError::NoEmittingState {
                            path,
                            entry: entry.to_string(),
                        });
                    }
                }
            }
            Ok(LightColors::new(colours, types))
        }
    }

    fn members<'a>(
        entry: &BlockStateRef,
        blocks: &'a BlockDefinitions,
        tags: &Tags<Block>,
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
            StateTarget::Tag(id) => tags
                .get(&TagKey::<Block, _>::from_location(id.clone()))
                .map(|tag| {
                    tags.members(tag)
                        .map(|member| &blocks.blocks()[member.index()])
                        .collect()
                })
                .ok_or_else(|| LightColorError::UnknownTag {
                    path: path.to_owned(),
                    entry: entry.to_string(),
                }),
        }
    }

    fn check_predicate(
        entry: &BlockStateRef,
        members: &[&BlockEntry],
        path: &str,
    ) -> Result<(), LightColorError> {
        for block in members {
            for (name, text) in &entry.properties {
                let Some(index) = block.properties.index_of(name) else {
                    return Err(LightColorError::UnknownProperty {
                        path: path.to_owned(),
                        entry: entry.to_string(),
                        block: block.identifier.to_string(),
                    });
                };
                if !block.properties.0[index]
                    .values
                    .iter()
                    .any(|value| value.renders_to(text))
                {
                    return Err(LightColorError::UnknownValue {
                        path: path.to_owned(),
                        entry: entry.to_string(),
                        block: block.identifier.to_string(),
                    });
                }
            }
        }
        Ok(())
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

    fn describe(block: &BlockEntry, id: BlockStateId) -> String {
        let values: Vec<String> = block
            .properties
            .0
            .iter()
            .filter_map(|p| {
                Some(format!(
                    "{}={}",
                    p.name,
                    block.value_of(id, &p.name)?.to_text()
                ))
            })
            .collect();
        if values.is_empty() {
            block.identifier.to_string()
        } else {
            format!("{}[{}]", block.identifier, values.join(","))
        }
    }
}
