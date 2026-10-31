use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::fmt;
use std::sync::Arc;

use mcrs_minecraft_block::definition::{BlockDefinitions, BlockEntry};
use mcrs_minecraft_block::keys::Fluid;
use mcrs_minecraft_block::keys::fluid_tags::WATER;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_item::ItemDefinitions;
use mcrs_minecraft_item::keys::Item;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::{Id, ResourceLocationBuf, TagKey, rl};
use mcrs_voxel::BlockStateId;
use serde::{Deserialize, Deserializer, Serialize, de};

use crate::asset::{BlockStateRef, StateTarget};
use crate::colors::{LightColors, LightType};

pub const CORPUS_DIRECTORY: &str = "mcrs/item_light";
pub const WATER_SENSITIVE: TagKey<'static, Item> = TagKey::new(rl!("mcrs:water_sensitive_light"));

/// A bare block id stands for the block's default state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ItemLightFile(pub BTreeMap<ResourceLocationBuf, BlockStateRef>);

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
        while let Some((item, state)) = map.next_entry::<ResourceLocationBuf, BlockStateRef>()? {
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

#[derive(Clone, Debug)]
pub struct ItemLights {
    mapped: Arc<[Option<BlockStateId>]>,
    water_sensitive: Arc<[bool]>,
    water: Arc<[bool]>,
}

#[derive(Debug, thiserror::Error)]
pub enum ItemLightError {
    #[error("the registries declare no {registry} registry with its tags")]
    Undeclared { registry: &'static str },
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
}

impl ItemLights {
    pub fn from_files(
        files: impl IntoIterator<Item = (String, Vec<u8>)>,
        blocks: &BlockDefinitions,
        items: &ItemDefinitions,
        registries: &Registries,
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
                let (Some(state), Some(state_in)) = (mapped.get_mut(slot), mapped_in.get_mut(slot))
                else {
                    return Err(ItemLightError::UnknownItem {
                        path,
                        item: item.to_string(),
                    });
                };
                if let Some(first) = state_in.take() {
                    return Err(ItemLightError::DuplicateItem {
                        item: item.to_string(),
                        first,
                        second: path,
                    });
                }
                *state = Some(resolve(&target, blocks, &path, &item)?);
                *state_in = Some(path.clone());
            }
        }

        let missing: Vec<String> = items
            .iter()
            .filter(|item| mapped.get(item.id.index()).is_none_or(Option::is_none))
            .filter(|item| {
                item.block_placer
                    .and_then(|state| blocks.owner(state))
                    .is_some_and(|owner| emits(blocks, owner))
            })
            .map(|item| item.identifier.to_string())
            .collect();
        if !missing.is_empty() {
            return Err(ItemLightError::Missing { items: missing });
        }
        let undeclared = |registry| ItemLightError::Undeclared { registry };
        let fluids = registries
            .entries::<Fluid>()
            .ok_or_else(|| undeclared("fluid"))?;
        let item_tags = registries
            .tags::<Item>()
            .ok_or_else(|| undeclared("item"))?;
        let fluid_tags = registries
            .tags::<Fluid>()
            .ok_or_else(|| undeclared("fluid"))?;
        let water_sensitive = item_tags.id(WATER_SENSITIVE);
        let water = fluid_tags.id(WATER);
        Ok(ItemLights {
            mapped: mapped.into(),
            water_sensitive: items
                .iter()
                .map(|entry| water_sensitive.is_some_and(|tag| item_tags.contains(tag, entry.id)))
                .collect(),
            water: fluids
                .iter()
                .map(|(fluid, _)| water.is_some_and(|tag| fluid_tags.contains(tag, fluid)))
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
        let owner = blocks.owner(mapped)?;
        let state = stack_state.into_iter().fold(mapped, |id, (name, value)| {
            owner.with_text(id, name, value).unwrap_or(id)
        });
        let emission = blocks.state(state).light_emission;
        (emission > 0).then(|| ItemLight {
            emission,
            light_type: colours.light_type(VoxelId(state.number())),
            state,
        })
    }
}

impl ItemLights {
    fn holds_water(&self, blocks: &BlockDefinitions, state: BlockStateId) -> bool {
        state.index() < blocks.state_count()
            && blocks
                .state(state)
                .fluid
                .is_some_and(|f| self.water.get(f.fluid.index()) == Some(&true))
    }
}

fn resolve(
    target: &BlockStateRef,
    blocks: &BlockDefinitions,
    path: &str,
    item: &ResourceLocationBuf,
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
    (0..block.state_count)
        .filter_map(|offset| block.state_at(offset))
        .any(|state| blocks.state(state).light_emission > 0)
}
