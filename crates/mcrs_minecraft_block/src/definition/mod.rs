pub mod molang;
pub mod schema;

use core::time::Duration;
use std::sync::Arc;

#[cfg(feature = "bevy")]
use bevy_platform::time::Instant;

#[cfg(feature = "bevy")]
use bevy_asset::AssetServer;
#[cfg(feature = "bevy")]
use bevy_asset::io::AssetSourceId;
use bevy_math::Vec3;
use rustc_hash::{FxBuildHasher, FxHashMap};

use self::molang::{MolangError, StateCondition};
use self::schema::{
    BlockDefinitionFile, BlockProperties, Components, LavaFlammable, ModelBox, NoteBlockInstrument,
    PlacementFilter, PropertyValue, Sticky,
};
use crate::material::PushReaction;
use crate::material::map::MapColor;
#[cfg(feature = "bevy")]
use mcrs_minecraft_assets::asset::{CorpusReadError, read_json_corpus};
use mcrs_minecraft_value_provider::IntProvider;
use mcrs_minecraft_core::voxel_shape::Aabb;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use crate::keys::Fluid;
use crate::keys::Block;
use mcrs_minecraft_registry::{BlockStateId, Id, Registry, RegistryLookup, UnknownEntry};

pub const CORPUS_DIRECTORY: &str = "mcrs/block_definition";

bitflags::bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub struct BlockStateFlags: u16 {
        const REQUIRES_CORRECT_TOOL_FOR_DROPS = 1 << 0;
        const IS_AIR = 1 << 1;
        const REPLACEABLE = 1 << 2;
        const IGNITED_BY_LAVA = 1 << 3;
        const USE_SHAPE_FOR_LIGHT_OCCLUSION = 1 << 4;
        const PROPAGATES_SKYLIGHT_DOWN = 1 << 5;
        const EMISSIVE_RENDERING = 1 << 6;
        const IS_SOLID_RENDER = 1 << 7;
        const IS_COLLISION_SHAPE_FULL_BLOCK = 1 << 8;
        const HAS_BLOCK_ENTITY = 1 << 9;
        const IS_SIGNAL_SOURCE = 1 << 10;
        const REDSTONE_CONDUCTOR = 1 << 11;
        const STICKY = 1 << 12;
        const LEGACY_SOLID = 1 << 13;
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ShapeId(pub u32);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct LootId(pub u16);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ExperienceId(pub u16);

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FluidState {
    pub fluid: Id<Fluid>,
    pub level: u8,
    pub source: bool,
}

/// Everything the corpus states about one block state, resolved. Shapes are
/// interned, so a slot is a fixed-size record and shape identity is a `u32`
/// comparison.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct BlockStateData {
    pub light_emission: u8,
    pub light_dampening: u8,
    pub friction: f32,
    pub hardness: f32,
    pub explosion_resistance: f32,
    pub map_color: MapColor,
    pub collision_shape: ShapeId,
    pub selection_shape: ShapeId,
    pub occlusion_shape: ShapeId,
    pub push_reaction: PushReaction,
    pub instrument: NoteBlockInstrument,
    pub redstone_power: u8,
    pub loot: Option<LootId>,
    pub experience: Option<ExperienceId>,
    pub fluid: Option<FluidState>,
    pub flags: BlockStateFlags,
}

#[derive(Debug)]
pub struct BlockEntry {
    pub identifier: ResourceLocation<Arc<str>>,
    pub base_state_id: BlockStateId,
    pub default_state_id: BlockStateId,
    pub state_count: u16,
    pub properties: BlockProperties,
    pub placement_filter: Option<PlacementFilter>,
    pub container_slots: Option<u8>,
}

impl BlockEntry {
    pub fn owns(&self, id: BlockStateId) -> bool {
        id.0 >= self.base_state_id.0 && id.0 - self.base_state_id.0 < self.state_count
    }

    /// The state that differs from `id` in one property, the way vanilla's
    /// `BlockState.setValue` does. One property is one digit of the state id,
    /// so this is arithmetic over the layout rather than a search.
    pub fn with(
        &self,
        id: BlockStateId,
        property: &str,
        value: &PropertyValue,
    ) -> Option<BlockStateId> {
        if !self.owns(id) {
            return None;
        }
        let index = self.properties.index_of(property)?;
        let property = &self.properties.0[index];
        let target = property.values.iter().position(|v| v == value)? as u16;
        let stride: u16 = self.properties.0[index + 1..]
            .iter()
            .map(|p| p.values.len() as u16)
            .product();
        let current = (id.0 - self.base_state_id.0) / stride % property.values.len() as u16;
        Some(BlockStateId(id.0 - current * stride + target * stride))
    }

    /// The state's value for one property, the way vanilla's
    /// `BlockState.getValue` does.
    pub fn value_of(&self, id: BlockStateId, property: &str) -> Option<&PropertyValue> {
        if !self.owns(id) {
            return None;
        }
        let index = self.properties.index_of(property)?;
        let property = &self.properties.0[index];
        let stride: u16 = self.properties.0[index + 1..]
            .iter()
            .map(|p| p.values.len() as u16)
            .product();
        let current = (id.0 - self.base_state_id.0) / stride % property.values.len() as u16;
        property.values.get(current as usize)
    }

    /// [`BlockEntry::with`] against a value stated as text, as a datapack and a
    /// save state it.
    pub fn with_text(&self, id: BlockStateId, property: &str, text: &str) -> Option<BlockStateId> {
        let index = self.properties.index_of(property)?;
        let value = self.properties.0[index]
            .values
            .iter()
            .find(|value| value.renders_to(text))?
            .clone();
        self.with(id, property, &value)
    }

    /// The state id for a full set of property values, in any order. `None` if
    /// a property is not the block's, a value is not the property's, or the
    /// set does not name every property.
    pub fn state_id(&self, values: &[(&str, PropertyValue)]) -> Option<BlockStateId> {
        if values.len() != self.properties.0.len() {
            return None;
        }
        let mut offset = 0u16;
        for property in &self.properties.0 {
            let (_, value) = values.iter().find(|(name, _)| *name == &*property.name)?;
            let index = property.values.iter().position(|v| v == value)?;
            offset = offset * property.values.len() as u16 + index as u16;
        }
        Some(BlockStateId(self.base_state_id.0 + offset))
    }
}

/// The per-state table: the corpus is the truth, this is the materialization
/// of it, and it is the only one. Building it costs one pass over the corpus at
/// startup; reading it costs one array index.
#[derive(Debug)]
pub struct BlockDefinitions {
    states: Vec<BlockStateData>,
    shapes: Vec<Box<[Aabb]>>,
    loot: Vec<ResourceLocation<Arc<str>>>,
    experience: Vec<IntProvider>,
    blocks: Vec<BlockEntry>,
    owners: Vec<Id<Block>>,
    registry: Registry<Block>,
}

const _: () = assert!(size_of::<Id<Block>>() == 2);

impl BlockDefinitions {
    #[inline]
    pub fn state(&self, id: BlockStateId) -> &BlockStateData {
        &self.states[id.0 as usize]
    }

    #[inline]
    pub fn shape(&self, id: ShapeId) -> &[Aabb] {
        &self.shapes[id.0 as usize]
    }

    #[inline]
    pub fn loot_table(&self, id: LootId) -> &ResourceLocation<Arc<str>> {
        &self.loot[id.0 as usize]
    }

    pub fn loot_table_count(&self) -> usize {
        self.loot.len()
    }

    #[inline]
    pub fn experience_drop(&self, id: ExperienceId) -> &IntProvider {
        &self.experience[id.0 as usize]
    }

    pub fn state_count(&self) -> usize {
        self.states.len()
    }

    pub fn blocks(&self) -> &[BlockEntry] {
        &self.blocks
    }

    pub fn block(&self, identifier: &str) -> Option<&BlockEntry> {
        self.id_of(identifier).map(|id| &self[id])
    }

    /// The block's default state. Panics if the corpus does not name it: the
    /// corpus is every block the game has, so a miss is a typo and not input.
    pub fn default_state(&self, identifier: &str) -> BlockStateId {
        self.block(identifier)
            .unwrap_or_else(|| panic!("the corpus has no block `{identifier}`"))
            .default_state_id
    }

    #[inline]
    pub fn default_state_of(&self, block: Id<Block>) -> BlockStateId {
        self[block].default_state_id
    }

    /// The block a state belongs to. Every state in the table has one:
    /// `Builder::finish` refuses a table with a state no block claimed.
    #[inline]
    pub fn owner(&self, id: BlockStateId) -> &BlockEntry {
        &self[self.block_index(id)]
    }

    /// The block's id in the registries report, which is also its position in
    /// the table and the number the protocol names it by. Block tags are
    /// resolved against it, so a state's tag membership is two array reads.
    #[inline]
    pub fn block_index(&self, id: BlockStateId) -> Id<Block> {
        self.owners[id.0 as usize]
    }

    #[inline]
    pub fn id_of(&self, identifier: &str) -> Option<Id<Block>> {
        self.registry.by_name(identifier)
    }

    pub fn table_bytes(&self) -> usize {
        let states = self.states.len() * size_of::<BlockStateData>()
            + self.owners.len() * size_of::<Id<Block>>();
        let shapes: usize = self
            .shapes
            .iter()
            .map(|s| size_of::<Box<[Aabb]>>() + s.len() * size_of::<Aabb>())
            .sum();
        states + shapes
    }
}

impl std::ops::Index<Id<Block>> for BlockDefinitions {
    type Output = BlockEntry;

    #[inline]
    fn index(&self, id: Id<Block>) -> &BlockEntry {
        &self.blocks[id.index()]
    }
}

impl RegistryLookup for BlockDefinitions {
    fn id(&self, _: &str, _: &ResourceLocation) -> Option<u16> {
        None
    }

    fn name(&self, _: &str, _: u16) -> Option<&ResourceLocation> {
        None
    }

    fn block_state_id(&self, block: &ResourceLocation, properties: &[(&str, &str)]) -> Option<u16> {
        let entry = self.block(block.as_str())?;
        let id = properties
            .iter()
            .fold(entry.default_state_id, |id, (name, value)| {
                entry.with_text(id, name, value).unwrap_or(id)
            });
        Some(id.0)
    }

    fn block_state(&self, id: u16) -> Option<(ResourceLocation, Vec<(String, String)>)> {
        let id = BlockStateId(id);
        if usize::from(id.0) >= self.states.len() {
            return None;
        }
        let entry = self.owner(id);
        let properties = if id == entry.default_state_id {
            Vec::new()
        } else {
            entry
                .properties
                .0
                .iter()
                .map(|property| {
                    let value = entry
                        .value_of(id, &property.name)
                        .expect("the state's own property");
                    (property.name.to_string(), value.to_text())
                })
                .collect()
        };
        Some((entry.identifier.clone(), properties))
    }
}

/// The corpus as every world sees it. A dimension sub-app is handed a clone at
/// spawn, so the table is shared rather than rebuilt or copied per dimension.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::resource::Resource))]
pub struct Blocks(pub Arc<BlockDefinitions>);

#[cfg(feature = "bevy")]
impl mcrs_minecraft_registry::shared::SharedResource for Blocks {
    fn shares_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl std::ops::Deref for Blocks {
    type Target = BlockDefinitions;

    fn deref(&self) -> &BlockDefinitions {
        &self.0
    }
}

#[derive(Debug)]
pub struct LoadReport {
    pub files: usize,
    pub states: usize,
    pub permutations: usize,
    pub shapes: usize,
    pub table_bytes: usize,
    pub elapsed: Duration,
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[cfg(feature = "bevy")]
    #[error("the default asset source is missing")]
    NoAssetSource,
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
    #[error("`{block}`: {source}")]
    Block { block: String, source: BlockError },
    #[error("block state {0} is claimed by no block")]
    UnclaimedState(u16),
    #[error("`{block}` is in the registries report and has no definition file")]
    MissingDefinition { block: String },
}

#[derive(Debug, thiserror::Error)]
pub enum BlockError {
    #[error(transparent)]
    Unknown(#[from] UnknownEntry),
    #[error("states protocol id {stated}, which is {report} in the registries report")]
    ProtocolIdDisagrees { stated: u16, report: usize },
    #[error("condition `{condition}`: {source}")]
    Condition {
        condition: String,
        source: MolangError,
    },
    #[error("{states} states starting at {base} do not fit the state id space")]
    StateSpace { base: u16, states: usize },
    #[error("default state {default} is outside [{base}, {base} + {states})")]
    DefaultOutOfRange {
        base: u16,
        default: u16,
        states: usize,
    },
    #[error("state {state} is already claimed by `{owner}`")]
    OverlappingState { state: u16, owner: String },
    #[error("the block is already defined by another file")]
    DuplicateBlock,
    #[error("state {state} states no `{component}`")]
    MissingComponent { state: u16, component: &'static str },
}

#[cfg(feature = "bevy")]
pub fn load_block_definitions(
    asset_server: &AssetServer,
    blocks: &Registry<Block>,
) -> Result<(BlockDefinitions, LoadReport), LoadError> {
    let started = Instant::now();
    let source = asset_server
        .get_source(AssetSourceId::Default)
        .map_err(|_| LoadError::NoAssetSource)?;
    let corpus = read_json_corpus(source.reader(), CORPUS_DIRECTORY)?;

    let mut builder = Builder::new(blocks);
    let files = corpus.len();
    for (path, bytes) in corpus {
        if bytes.is_empty() {
            return Err(LoadError::Empty { path });
        }
        let file: BlockDefinitionFile =
            serde_json::from_slice(&bytes).map_err(|source| LoadError::Parse {
                path: path.clone(),
                source,
            })?;
        let block = file.block.description.identifier.as_str().to_owned();
        builder
            .add(file)
            .map_err(|source| LoadError::Block { block, source })?;
    }

    let definitions = builder.finish()?;
    let report = LoadReport {
        files,
        states: definitions.states.len(),
        permutations: builder.permutations,
        shapes: definitions.shapes.len(),
        table_bytes: definitions.table_bytes(),
        elapsed: started.elapsed(),
    };
    Ok((definitions, report))
}

/// Fills a slot until its block claims it. `Builder::finish` refuses a table
/// with a slot no block claimed, so it never reaches a reader.
const UNCLAIMED: BlockStateData = BlockStateData {
    light_emission: 0,
    light_dampening: 0,
    friction: 0.0,
    hardness: 0.0,
    explosion_resistance: 0.0,
    map_color: MapColor::NONE,
    collision_shape: ShapeId(u32::MAX),
    selection_shape: ShapeId(u32::MAX),
    occlusion_shape: ShapeId(u32::MAX),
    push_reaction: PushReaction::Normal,
    instrument: NoteBlockInstrument::Harp,
    redstone_power: 0,
    loot: None,
    experience: None,
    fluid: None,
    flags: BlockStateFlags::empty(),
};

struct Builder {
    states: Vec<BlockStateData>,
    owners: Vec<Option<Id<Block>>>,
    shapes: Vec<Box<[Aabb]>>,
    shape_ids: FxHashMap<Vec<u32>, ShapeId>,
    loot: Vec<ResourceLocation<Arc<str>>>,
    experience: Vec<IntProvider>,
    registry: Registry<Block>,
    blocks: Vec<Option<BlockEntry>>,
    permutations: usize,
}

impl Builder {
    fn new(registry: &Registry<Block>) -> Self {
        Builder {
            states: Vec::new(),
            owners: Vec::new(),
            shapes: Vec::new(),
            shape_ids: FxHashMap::with_hasher(FxBuildHasher),
            loot: Vec::new(),
            experience: Vec::new(),
            blocks: std::iter::repeat_with(|| None)
                .take(registry.len())
                .collect(),
            registry: registry.clone(),
            permutations: 0,
        }
    }

    fn intern_shape(&mut self, boxes: &[ModelBox]) -> ShapeId {
        let key: Vec<u32> = boxes
            .iter()
            .flat_map(|b| b.origin.into_iter().chain(b.size))
            .map(f32::to_bits)
            .collect();
        if let Some(&id) = self.shape_ids.get(&key) {
            return id;
        }
        let id = ShapeId(self.shapes.len() as u32);
        self.shapes.push(boxes.iter().map(to_engine_aabb).collect());
        self.shape_ids.insert(key, id);
        id
    }

    fn intern_loot(&mut self, table: &ResourceLocation<Arc<str>>) -> LootId {
        LootId(intern(&mut self.loot, table))
    }

    fn intern_experience(&mut self, drop: IntProvider) -> ExperienceId {
        match self.experience.iter().position(|v| *v == drop) {
            Some(index) => ExperienceId(index as u16),
            None => {
                self.experience.push(drop);
                ExperienceId(self.experience.len() as u16 - 1)
            }
        }
    }

    fn add(&mut self, file: BlockDefinitionFile) -> Result<(), BlockError> {
        let description = file.block.description;
        let id = self
            .registry
            .require(&ResourceKey::from_location(description.identifier.clone()))?;
        if usize::from(description.protocol_id) != id.index() {
            return Err(BlockError::ProtocolIdDisagrees {
                stated: description.protocol_id,
                report: id.index(),
            });
        }
        if self.blocks[id.index()].is_some() {
            return Err(BlockError::DuplicateBlock);
        }
        let properties = description.properties;
        let base = description.base_state_id;
        let state_count = properties.state_count();
        if state_count == 0 || base as usize + state_count > u16::MAX as usize + 1 {
            return Err(BlockError::StateSpace {
                base,
                states: state_count,
            });
        }
        let default = description.default_state_id;
        if default < base || (default - base) as usize >= state_count {
            return Err(BlockError::DefaultOutOfRange {
                base,
                default,
                states: state_count,
            });
        }

        let components = file.block.components;

        let mut permutations = Vec::with_capacity(file.block.permutations.len());
        for permutation in &file.block.permutations {
            let condition =
                StateCondition::compile(&permutation.condition, &properties).map_err(|source| {
                    BlockError::Condition {
                        condition: permutation.condition.clone(),
                        source,
                    }
                })?;
            permutations.push((condition, &permutation.components));
        }
        self.permutations += permutations.len();

        let end = base as usize + state_count;
        if self.states.len() < end {
            self.states.resize(end, UNCLAIMED);
            self.owners.resize(end, None);
        }

        // Bedrock knows its air block by identifier and states no component for
        // it; Java's three air blocks are the same fact.
        let air = [
            Block::Air,
            Block::CaveAir,
            Block::VoidAir,
        ]
        .iter()
        .any(|block| block.as_static_str() == description.identifier.as_str());
        let mut values = vec![0u8; properties.0.len()];
        for index in 0..state_count {
            let mut rest = index;
            for (slot, property) in values.iter_mut().zip(&properties.0).rev() {
                *slot = (rest % property.values.len()) as u8;
                rest /= property.values.len();
            }

            let mut resolved = components.clone();
            for (condition, overlay) in &permutations {
                if condition.matches(&values) {
                    resolved.overlay(overlay);
                }
            }

            let state = base as usize + index;
            if let Some(owner) = self.owners[state] {
                return Err(BlockError::OverlappingState {
                    state: state as u16,
                    owner: self
                        .registry
                        .table()
                        .name(owner.index())
                        .map_or_else(String::new, |name| name.as_str().to_owned()),
                });
            }
            self.owners[state] = Some(id);
            if let Some(component) = resolved.missing() {
                return Err(BlockError::MissingComponent {
                    state: state as u16,
                    component,
                });
            }
            let mut data = self.resolve(&resolved)?;
            data.flags.set(BlockStateFlags::IS_AIR, air);
            self.states[state] = data;
        }

        let identifier = self
            .registry
            .name(id)
            .expect("the id came from this registry")
            .clone();
        self.blocks[id.index()] = Some(BlockEntry {
            identifier,
            base_state_id: BlockStateId(base),
            default_state_id: BlockStateId(default),
            state_count: state_count as u16,
            properties,
            placement_filter: components.placement_filter,
            container_slots: components
                .block_entity
                .as_ref()
                .and_then(|block_entity| block_entity.container)
                .map(|container| container.slot_count),
        });
        Ok(())
    }

    /// Every component a state may not leave unstated is present here:
    /// [`Components::missing`] has already run against these components.
    fn resolve(&mut self, components: &Components) -> Result<BlockStateData, BlockError> {
        let shape = |builder: &mut Self, boxes: &Option<schema::BoxList>| {
            builder.intern_shape(&boxes.as_ref().unwrap().0)
        };
        let mut flags = BlockStateFlags::empty();
        let mut flag = |set: Option<bool>, bit: BlockStateFlags| {
            if set.unwrap() {
                flags |= bit;
            }
        };
        flag(
            Some(
                !components
                    .destructible_by_mining
                    .as_ref()
                    .unwrap()
                    .harvested_by
                    .is_empty(),
            ),
            BlockStateFlags::REQUIRES_CORRECT_TOOL_FOR_DROPS,
        );
        flag(
            Some(components.replaceable.is_some()),
            BlockStateFlags::REPLACEABLE,
        );
        flag(
            Some(
                components
                    .flammable
                    .is_some_and(|f| f.lava_flammable == LavaFlammable::Always),
            ),
            BlockStateFlags::IGNITED_BY_LAVA,
        );
        flag(
            components.use_shape_for_light_occlusion,
            BlockStateFlags::USE_SHAPE_FOR_LIGHT_OCCLUSION,
        );
        flag(
            Some(components.light_dampening.unwrap() == 0),
            BlockStateFlags::PROPAGATES_SKYLIGHT_DOWN,
        );
        flag(
            components.emissive_rendering,
            BlockStateFlags::EMISSIVE_RENDERING,
        );
        flag(
            Some(fills_the_cube(
                &components.occlusion_shape.as_ref().unwrap().0,
            )),
            BlockStateFlags::IS_SOLID_RENDER,
        );
        flag(
            Some(fills_the_cube(
                &components.collision_box.as_ref().unwrap().0,
            )),
            BlockStateFlags::IS_COLLISION_SHAPE_FULL_BLOCK,
        );
        flag(
            Some(legacy_solid(&components.collision_box.as_ref().unwrap().0)),
            BlockStateFlags::LEGACY_SOLID,
        );
        flag(
            Some(components.block_entity.is_some()),
            BlockStateFlags::HAS_BLOCK_ENTITY,
        );
        flag(
            Some(components.redstone_producer.is_some()),
            BlockStateFlags::IS_SIGNAL_SOURCE,
        );
        flag(
            Some(components.movable.unwrap().sticky == Sticky::Same),
            BlockStateFlags::STICKY,
        );
        flag(
            components
                .redstone_conductivity
                .map(|c| c.redstone_conductor),
            BlockStateFlags::REDSTONE_CONDUCTOR,
        );

        let collision_shape = shape(self, &components.collision_box);
        let selection_shape = shape(self, &components.selection_box);
        let occlusion_shape = shape(self, &components.occlusion_shape);
        let loot = components
            .loot
            .as_ref()
            .map(|table| self.intern_loot(table));
        let experience = components
            .experience_drop
            .clone()
            .map(|drop| self.intern_experience(drop));
        let fluid = components
            .fluid_state
            .as_ref()
            .map(|fluid| {
                Ok::<_, BlockError>(FluidState {
                    fluid: Fluid::find(fluid.fluid.as_str())
                        .ok_or_else(|| UnknownEntry {
                            registry: crate::keys::FLUID.location().into(),
                            name: fluid.fluid.as_str().to_owned(),
                        })?
                        .id(),
                    level: fluid.level,
                    source: fluid.source,
                })
            })
            .transpose()?;

        Ok(BlockStateData {
            light_emission: components.light_emission.unwrap(),
            light_dampening: components.light_dampening.unwrap(),
            friction: components.friction.unwrap(),
            hardness: components.destructible_by_mining.as_ref().unwrap().hardness,
            explosion_resistance: components
                .destructible_by_explosion
                .unwrap()
                .explosion_resistance,
            map_color: components.map_color.unwrap(),
            collision_shape,
            selection_shape,
            occlusion_shape,
            push_reaction: components.movable.unwrap().movement_type,
            instrument: components.instrument_sound.unwrap().0,
            redstone_power: components.redstone_producer.map_or(0, |p| p.power),
            loot,
            experience,
            fluid,
            flags,
        })
    }

    fn finish(&mut self) -> Result<BlockDefinitions, LoadError> {
        let mut blocks = Vec::with_capacity(self.blocks.len());
        for (index, slot) in std::mem::take(&mut self.blocks).into_iter().enumerate() {
            blocks.push(slot.ok_or_else(|| {
                LoadError::MissingDefinition {
                    block: self
                        .registry
                        .table()
                        .name(index)
                        .map_or_else(String::new, |name| name.as_str().to_owned()),
                }
            })?);
        }
        let owners = self
            .owners
            .iter()
            .enumerate()
            .map(|(state, owner)| owner.ok_or(LoadError::UnclaimedState(state as u16)))
            .collect::<Result<_, _>>()?;
        Ok(BlockDefinitions {
            states: std::mem::take(&mut self.states),
            shapes: std::mem::take(&mut self.shapes),
            loot: std::mem::take(&mut self.loot),
            experience: std::mem::take(&mut self.experience),
            blocks,
            owners,
            registry: self.registry.clone(),
        })
    }
}

fn intern(table: &mut Vec<ResourceLocation<Arc<str>>>, value: &ResourceLocation<Arc<str>>) -> u16 {
    match table.iter().position(|v| v.as_str() == value.as_str()) {
        Some(index) => index as u16,
        None => {
            table.push(value.clone());
            table.len() as u16 - 1
        }
    }
}

/// Java's `Block.isShapeFullBlock`: the shape covers the whole block. The boxes
/// of a `VoxelShape` never overlap, so covering the cube is the same as filling
/// its volume without leaving it.
fn fills_the_cube(boxes: &[ModelBox]) -> bool {
    const EPSILON: f32 = 1.0 / 4096.0;
    let mut volume = 0.0;
    for b in boxes {
        let max = [
            b.origin[0] + b.size[0],
            b.origin[1] + b.size[1],
            b.origin[2] + b.size[2],
        ];
        if b.origin[0] < -8.0 - EPSILON || max[0] > 8.0 + EPSILON {
            return false;
        }
        if b.origin[1] < -EPSILON || max[1] > 16.0 + EPSILON {
            return false;
        }
        if b.origin[2] < -8.0 - EPSILON || max[2] > 8.0 + EPSILON {
            return false;
        }
        volume += b.size[0] * b.size[1] * b.size[2];
    }
    (volume - 16.0 * 16.0 * 16.0).abs() < 0.5
}

/// Java's `BlockBehaviour.calculateSolid`: the collision shape's bounds are
/// either large enough on average or a full block tall.
///
/// The corpus carries no `forceSolidOn`/`forceSolidOff` component, so this is
/// the shape derivation alone and answers differently from the reference for
/// the blocks it overrides by hand — signs, pressure plates, cobweb, snow,
/// azalea, ladders, amethyst clusters, pointed dripstone and corals.
fn legacy_solid(boxes: &[ModelBox]) -> bool {
    let Some(first) = boxes.first() else {
        return false;
    };
    let mut min = Vec3::from(first.origin);
    let mut max = min + Vec3::from(first.size);
    for b in &boxes[1..] {
        min = min.min(Vec3::from(b.origin));
        max = max.max(Vec3::from(b.origin) + Vec3::from(b.size));
    }
    let size = (max - min).as_dvec3() / 16.0;
    size.element_sum() / 3.0 >= 0.7291666666666666 || size.y >= 1.0
}

/// Bedrock states a box in sixteenths, from the block centre on X and Z and
/// the block bottom on Y. The engine's own convention is Java's: a 0..1 box
/// from the block's lower corner.
fn to_engine_aabb(value: &ModelBox) -> Aabb {
    let min = Vec3::new(
        (value.origin[0] + 8.0) / 16.0,
        value.origin[1] / 16.0,
        (value.origin[2] + 8.0) / 16.0,
    );
    Aabb {
        min,
        max: min + Vec3::from(value.size) / 16.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_core::rl;

    const COMPONENTS: &str = r##"{
        "minecraft:light_emission": 0,
        "minecraft:light_dampening": 15,
        "minecraft:friction": 0.6,
        "minecraft:map_color": "#707070",
        "minecraft:collision_box": true,
        "minecraft:selection_box": true,
        "minecraft:destructible_by_explosion": { "explosion_resistance": 6 },
        "minecraft:redstone_conductivity": { "redstone_conductor": true },
        "minecraft:movable": { "movement_type": "push_pull" },
        "minecraft:instrument_sound": { "down": "basedrum" },
        "minecraft:destructible_by_mining": { "seconds_to_destroy": 1.5 },
        "mcrs:use_shape_for_light_occlusion": false,
        "mcrs:occlusion_shape": true,
        "mcrs:emissive_rendering": false
    }"##;

    fn file(identifier: &str, base: u16, properties: &str, rest: &str) -> String {
        format!(
            r#"{{"format_version": "1.21.130", "minecraft:block": {{
                "description": {{
                    "identifier": "{identifier}",
                    "protocol_id": 0,
                    "base_state_id": {base},
                    "default_state_id": {base}
                    {properties}
                }},
                "components": {COMPONENTS}
                {rest}
            }}}}"#
        )
    }

    fn registry(names: &[&str]) -> Registry<Block> {
        Registry::new(
            crate::keys::BLOCK,
            names
                .iter()
                .map(|name| ResourceLocation::read(name).unwrap()),
        )
        .unwrap()
    }

    fn build_against(names: &[&str], files: &[String]) -> Result<BlockDefinitions, LoadError> {
        let mut builder = Builder::new(&registry(names));
        for source in files {
            let file: BlockDefinitionFile = serde_json::from_str(source).expect("valid json");
            let block = file.block.description.identifier.as_str().to_owned();
            builder
                .add(file)
                .map_err(|source| LoadError::Block { block, source })?;
        }
        builder.finish()
    }

    fn build(files: &[String]) -> Result<BlockDefinitions, LoadError> {
        let identifiers: Vec<String> = files
            .iter()
            .map(|source| {
                let file: BlockDefinitionFile = serde_json::from_str(source).expect("valid json");
                file.block.description.identifier.as_str().to_owned()
            })
            .collect();
        let names: Vec<&str> = identifiers.iter().map(String::as_str).collect();
        let stated: Vec<String> = files
            .iter()
            .enumerate()
            .map(|(position, source)| {
                source.replacen(
                    r#""protocol_id": 0"#,
                    &format!(r#""protocol_id": {position}"#),
                    1,
                )
            })
            .collect();
        build_against(&names, &stated)
    }

    #[test]
    fn a_block_sits_at_its_registry_id_whatever_order_its_file_is_read_in() {
        let first = file("minecraft:first", 0, "", "").replacen(
            r#""protocol_id": 0"#,
            r#""protocol_id": 1"#,
            1,
        );
        let second = file("minecraft:second", 1, "", "");
        let definitions =
            build_against(&["minecraft:second", "minecraft:first"], &[first, second]).unwrap();
        assert_eq!(
            definitions.id_of("minecraft:second").map(Id::index),
            Some(0)
        );
        assert_eq!(definitions.id_of("minecraft:first").map(Id::index), Some(1));
        assert_eq!(
            definitions.blocks()[0].identifier.as_str(),
            "minecraft:second"
        );
        assert_eq!(definitions.block_index(BlockStateId(0)).index(), 1);
        assert_eq!(definitions.block_index(BlockStateId(1)).index(), 0);
    }

    #[test]
    fn a_report_block_without_a_file_fails_at_load_naming_it() {
        let error = build_against(
            &["minecraft:first", "minecraft:second"],
            &[file("minecraft:first", 0, "", "")],
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("`minecraft:second`"), "{error}");
    }

    #[test]
    fn a_file_for_a_block_the_report_lacks_fails_at_load_naming_it() {
        let error = build_against(
            &["minecraft:first"],
            &[
                file("minecraft:first", 0, "", ""),
                file("minecraft:stray", 1, "", ""),
            ],
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("minecraft:stray"), "{error}");
    }

    #[test]
    fn a_stated_protocol_id_the_report_disagrees_with_fails_at_load_naming_the_block() {
        let error = build_against(
            &["minecraft:first", "minecraft:second"],
            &[
                file("minecraft:first", 0, "", ""),
                file("minecraft:second", 1, "", ""),
            ],
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("minecraft:second"), "{error}");
        assert!(error.contains("protocol id 0"), "{error}");
    }

    #[test]
    fn a_block_without_properties_has_one_state() {
        let definitions = build(&[file("minecraft:test", 0, "", "")]).unwrap();
        assert_eq!(definitions.state_count(), 1);
        let block = definitions.block("minecraft:test").unwrap();
        assert_eq!(block.state_count, 1);
        assert_eq!(block.base_state_id, block.default_state_id);
        assert_eq!(
            definitions.shape(definitions.state(BlockStateId(0)).collision_shape),
            [Aabb {
                min: Vec3::ZERO,
                max: Vec3::ONE
            }]
        );
    }

    #[test]
    fn an_unstated_component_fails_at_load_naming_it() {
        let mut source = file("minecraft:test", 0, "", "");
        source = source.replace(r#""mcrs:use_shape_for_light_occlusion": false,"#, "");
        let error = build(&[source]).unwrap_err().to_string();
        assert!(
            error.contains("state 0 states no `mcrs:use_shape_for_light_occlusion`"),
            "{error}"
        );
    }

    #[test]
    fn a_component_map_outside_components_and_permutations_fails_at_load() {
        let source = file(
            "minecraft:test",
            0,
            r#", "properties": { "level": [0, 1] }"#,
            r#", "mcrs:state_components": [{}, {}]"#,
        );
        let error = serde_json::from_str::<BlockDefinitionFile>(&source)
            .unwrap_err()
            .to_string();
        assert!(error.contains("mcrs:state_components"), "{error}");
    }

    #[test]
    fn a_state_takes_the_last_matching_permutation() {
        let source = file(
            "minecraft:test",
            0,
            r#", "properties": { "level": [0, 1] }"#,
            r#", "permutations": [
                {
                    "condition": "q.block_state('level') == 0",
                    "components": { "minecraft:light_emission": 7 }
                },
                {
                    "condition": "q.block_state('level') == 0",
                    "components": { "minecraft:light_emission": 9 }
                }
            ]"#,
        );
        let definitions = build(&[source]).unwrap();
        assert_eq!(definitions.state(BlockStateId(0)).light_emission, 9);
    }

    #[test]
    fn overlapping_blocks_fail_at_load() {
        let error = build(&[
            file("minecraft:first", 0, "", ""),
            file("minecraft:second", 0, "", ""),
        ])
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("state 0 is already claimed by `minecraft:first`"),
            "{error}"
        );
    }

    #[test]
    fn a_block_defined_by_two_files_fails_at_load() {
        let error = build_against(
            &["minecraft:test"],
            &[
                file("minecraft:test", 0, "", ""),
                file("minecraft:test", 1, "", ""),
            ],
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("minecraft:test"), "{error}");
        assert!(error.contains("already defined"), "{error}");
    }

    #[test]
    fn a_hole_in_the_state_id_space_fails_at_load() {
        let error = build(&[file("minecraft:test", 1, "", "")])
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("block state 0 is claimed by no block"),
            "{error}"
        );
    }

    #[test]
    fn permutations_layer_over_the_block_wide_components() {
        let source = file(
            "minecraft:test",
            0,
            r#", "properties": { "lit": [true, false] }"#,
            r#", "permutations": [{
                "condition": "q.block_state('lit') == true",
                "components": { "minecraft:light_emission": 15 }
            }]"#,
        );
        let definitions = build(&[source]).unwrap();
        assert_eq!(definitions.state(BlockStateId(0)).light_emission, 15);
        assert_eq!(definitions.state(BlockStateId(1)).light_emission, 0);
    }

    #[test]
    fn the_registry_lookup_resolves_states_the_way_the_block_state_codec_does() {
        let source = file(
            "minecraft:furnace",
            0,
            r#", "properties": { "facing": ["north", "south"], "lit": [true, false] }"#,
            "",
        )
        .replace(r#""default_state_id": 0"#, r#""default_state_id": 1"#);
        let definitions = build(&[source, file("minecraft:stone", 4, "", "")]).unwrap();
        let furnace = rl!("minecraft:furnace").to_arc();
        let stone = rl!("minecraft:stone").to_arc();
        let lookup: &dyn RegistryLookup = &definitions;

        assert_eq!(lookup.block_state_id(&furnace, &[]), Some(1));
        assert_eq!(lookup.block_state_id(&furnace, &[("lit", "true")]), Some(0));
        assert_eq!(
            lookup.block_state_id(&furnace, &[("lit", "nope"), ("facing", "south")]),
            Some(3)
        );
        assert_eq!(
            lookup.block_state_id(&furnace, &[("bogus", "x"), ("lit", "true")]),
            Some(0)
        );
        assert_eq!(lookup.block_state_id(&stone, &[("lit", "true")]), Some(4));
        assert_eq!(
            lookup.block_state_id(&rl!("minecraft:nope").to_arc(), &[]),
            None
        );

        assert_eq!(lookup.block_state(1), Some((furnace.clone(), vec![])));
        assert_eq!(
            lookup.block_state(2),
            Some((
                furnace,
                vec![
                    ("facing".to_string(), "south".to_string()),
                    ("lit".to_string(), "true".to_string())
                ]
            ))
        );
        assert_eq!(lookup.block_state(4), Some((stone, vec![])));
        assert_eq!(lookup.block_state(5), None);
        assert_eq!(lookup.block_state(u16::MAX), None);
    }

    #[test]
    fn a_bedrock_box_becomes_a_block_local_box() {
        let torch = ModelBox {
            origin: [-2.0, 0.0, -2.0],
            size: [4.0, 10.0, 4.0],
        };
        assert_eq!(
            to_engine_aabb(&torch),
            Aabb {
                min: Vec3::new(0.375, 0.0, 0.375),
                max: Vec3::new(0.625, 0.625, 0.625)
            }
        );
        assert_eq!(
            to_engine_aabb(&ModelBox::FULL_CUBE),
            Aabb {
                min: Vec3::ZERO,
                max: Vec3::ONE
            }
        );
    }
}
