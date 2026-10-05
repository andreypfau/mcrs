use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::fmt;
use std::sync::Arc;

use bevy_asset::AssetServer;
use bevy_asset::io::AssetSourceId;
use bevy_ecs::resource::Resource;
use mcrs_minecraft_assets::asset::{CorpusReadError, read_json_corpus};
use mcrs_minecraft_block::definition::{BlockDefinitions, BlockEntry};
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::{ResourceLocation, TagKey, rl};
use mcrs_minecraft_item::ItemDefinitions;
use mcrs_minecraft_keys::fluid_tags::WATER;
use mcrs_minecraft_keys::{Fluid, Item};
use mcrs_minecraft_registry::{BlockStateId, Id, RegistrySet, TagId, Tags};
use serde::{Deserialize, Deserializer, Serialize, de};

use crate::asset::{BlockStateRef, StateTarget};
use crate::colors::{LightColors, LightType};

pub const CORPUS_DIRECTORY: &str = "mcrs/item_light";
pub const WATER_SENSITIVE: TagKey<Item> = TagKey::new(rl!("mcrs:water_sensitive_light"));

/// A bare block id stands for the block's default state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ItemLightFile(pub BTreeMap<ResourceLocation<Arc<str>>, BlockStateRef>);

impl<'de> Deserialize<'de> for ItemLightFile {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_map(ItemLightFileVisitor)
    }
}

struct ItemLightFileVisitor;

impl<'de> de::Visitor<'de> for ItemLightFileVisitor {
    type Value = ItemLightFile;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a map from item id to block state")
    }

    fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut items = BTreeMap::new();
        while let Some((item, state)) =
            map.next_entry::<ResourceLocation<Arc<str>>, BlockStateRef>()?
        {
            match items.entry(item) {
                Entry::Vacant(slot) => {
                    slot.insert(state);
                }
                Entry::Occupied(slot) => {
                    return Err(de::Error::custom(format!(
                        "`{}` is mapped twice",
                        slot.key()
                    )));
                }
            }
        }
        Ok(ItemLightFile(items))
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ItemLight {
    pub emission: u8,
    pub light_type: LightType,
    pub state: BlockStateId,
}

#[derive(Clone, Debug, Resource)]
pub struct ItemLights {
    mapped: Arc<[Option<BlockStateId>]>,
    water_sensitive: Arc<[bool]>,
    water: Arc<[bool]>,
}

#[derive(Debug, thiserror::Error)]
pub enum ItemLightError {
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
    #[error("`{path}`: `{item}` is not an item")]
    UnknownItem { path: String, item: String },
    #[error("`{item}` is mapped in both `{first}` and `{second}`")]
    DuplicateItem {
        item: String,
        first: String,
        second: String,
    },
    #[error("`{path}`: `{item}` maps to a tag, not a block")]
    TagTarget { path: String, item: String },
    #[error("`{path}`: `{item}` maps to a block the corpus lacks")]
    UnknownBlock { path: String, item: String },
    #[error("`{path}`: `{item}` states a property its block does not declare")]
    UnknownProperty { path: String, item: String },
    #[error("`{path}`: `{item}` states a value its block does not declare")]
    UnknownValue { path: String, item: String },
    #[error("`{path}`: `{item}` maps to a block with no state that emits light")]
    NoEmittingState { path: String, item: String },
    #[error("items that place a light-emitting block have no mapping: {}", items.join(", "))]
    Missing { items: Vec<String> },
    #[error("the tag `#{tag}` does not exist")]
    MissingTag { tag: String },
}

impl ItemLights {
    pub fn load(
        asset_server: &AssetServer,
        blocks: &BlockDefinitions,
        items: &ItemDefinitions,
        registries: &RegistrySet,
    ) -> Result<Self, ItemLightError> {
        let source = asset_server
            .get_source(AssetSourceId::Default)
            .map_err(|_| ItemLightError::NoAssetSource)?;
        let files = read_json_corpus(source.reader(), CORPUS_DIRECTORY)?;
        Self::from_files(files, blocks, items, registries)
    }

    pub fn from_files(
        files: impl IntoIterator<Item = (String, Vec<u8>)>,
        blocks: &BlockDefinitions,
        items: &ItemDefinitions,
        registries: &RegistrySet,
    ) -> Result<Self, ItemLightError> {
        let mut mapped: Vec<Option<BlockStateId>> = vec![None; items.len()];
        let mut mapped_in: Vec<Option<String>> = vec![None; items.len()];
        for (path, bytes) in files {
            if bytes.is_empty() {
                return Err(ItemLightError::Empty { path });
            }
            let file: ItemLightFile =
                serde_json::from_slice(&bytes).map_err(|source| ItemLightError::Parse {
                    path: path.clone(),
                    source,
                })?;
            for (item, target) in file.0 {
                let Some(id) = items.id_of(item.as_str()) else {
                    return Err(ItemLightError::UnknownItem {
                        path,
                        item: item.to_string(),
                    });
                };
                let slot = id.index();
                if let Some(first) = mapped_in[slot].take() {
                    return Err(ItemLightError::DuplicateItem {
                        item: item.to_string(),
                        first,
                        second: path,
                    });
                }
                mapped[slot] = Some(resolve(&target, blocks, &path, &item)?);
                mapped_in[slot] = Some(path.clone());
            }
        }

        let missing: Vec<String> = items
            .iter()
            .filter(|item| mapped[item.id.index()].is_none())
            .filter(|item| {
                item.block_placer
                    .is_some_and(|state| emits(blocks, blocks.owner(state)))
            })
            .map(|item| item.identifier.to_string())
            .collect();
        if !missing.is_empty() {
            return Err(ItemLightError::Missing { items: missing });
        }
        let item_tags = registries
            .tags::<Item>()
            .expect("the loaded registries hold the item tags");
        let fluid_tags = registries
            .tags::<Fluid>()
            .expect("the loaded registries hold the fluid tags");
        let fluids = registries
            .registry::<Fluid>()
            .expect("the loaded registries hold the fluid registry");
        let water_sensitive = tag_of(&item_tags, &WATER_SENSITIVE)?;
        let water = tag_of(&fluid_tags, &WATER)?;
        Ok(ItemLights {
            mapped: mapped.into(),
            water_sensitive: items
                .iter()
                .map(|entry| item_tags.contains(water_sensitive, entry.id))
                .collect(),
            water: fluids
                .ids()
                .map(|fluid| fluid_tags.contains(water, fluid))
                .collect(),
        })
    }

    pub fn mapped_count(&self) -> usize {
        self.mapped.iter().flatten().count()
    }

    /// Unknown properties and values in `stack_state` are skipped, as the
    /// server skips them when it places the block. `origin` is the state of the
    /// cell the light comes from; water there puts out water-sensitive items.
    pub fn light<'a>(
        &self,
        blocks: &BlockDefinitions,
        colours: &LightColors,
        item: Id<Item>,
        stack_state: impl IntoIterator<Item = (&'a str, &'a str)>,
        origin: BlockStateId,
    ) -> Option<ItemLight> {
        let mapped = self.mapped.get(item.index()).copied().flatten()?;
        if self.water_sensitive.get(item.index()) == Some(&true) && self.holds_water(blocks, origin)
        {
            return None;
        }
        let owner = blocks.owner(mapped);
        let state = stack_state.into_iter().fold(mapped, |id, (name, value)| {
            owner.with_text(id, name, value).unwrap_or(id)
        });
        let emission = blocks.state(state).light_emission;
        (emission > 0).then(|| ItemLight {
            emission,
            light_type: colours.light_type(VoxelId(state.0)),
            state,
        })
    }
}

impl ItemLights {
    fn holds_water(&self, blocks: &BlockDefinitions, state: BlockStateId) -> bool {
        (state.0 as usize) < blocks.state_count()
            && blocks
                .state(state)
                .fluid
                .is_some_and(|f| self.water.get(f.fluid.index()) == Some(&true))
    }
}

fn tag_of<T: RegistryKey>(tags: &Tags<T>, tag: &TagKey<T>) -> Result<TagId<T>, ItemLightError> {
    tags.get(tag).ok_or_else(|| ItemLightError::MissingTag {
        tag: tag.as_str().to_owned(),
    })
}

fn resolve(
    target: &BlockStateRef,
    blocks: &BlockDefinitions,
    path: &str,
    item: &ResourceLocation<Arc<str>>,
) -> Result<BlockStateId, ItemLightError> {
    let error =
        |make: fn(String, String) -> ItemLightError| make(path.to_owned(), item.to_string());
    let StateTarget::Block(block) = &target.target else {
        return Err(error(|path, item| ItemLightError::TagTarget { path, item }));
    };
    let Some(block) = blocks.block(block.as_str()) else {
        return Err(error(|path, item| ItemLightError::UnknownBlock {
            path,
            item,
        }));
    };
    let mut state = block.default_state_id;
    for (name, value) in &target.properties {
        if block.properties.index_of(name).is_none() {
            return Err(error(|path, item| ItemLightError::UnknownProperty {
                path,
                item,
            }));
        }
        state = block
            .with_text(state, name, value)
            .ok_or_else(|| error(|path, item| ItemLightError::UnknownValue { path, item }))?;
    }
    if !emits(blocks, block) {
        return Err(error(|path, item| ItemLightError::NoEmittingState {
            path,
            item,
        }));
    }
    Ok(state)
}

fn emits(blocks: &BlockDefinitions, block: &BlockEntry) -> bool {
    (0..block.state_count).any(|offset| {
        blocks
            .state(BlockStateId(block.base_state_id.0 + offset))
            .light_emission
            > 0
    })
}
