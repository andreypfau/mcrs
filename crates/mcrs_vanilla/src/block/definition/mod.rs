pub mod molang;
pub mod schema;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bevy_asset::AssetServer;
use bevy_asset::io::AssetSourceId;
use bevy_ecs::resource::Resource;
use bevy_math::Vec3;
use bevy_tasks::block_on;
use futures_lite::StreamExt;
use rustc_hash::{FxBuildHasher, FxHashMap};

use self::molang::{MolangError, StateCondition};
use self::schema::{
    BlockBox, BlockDefinitionFile, BlockProperties, Components, Instrument, PropertyValue,
    RenderShape,
};
use crate::material::PushReaction;
use crate::material::map::MapColor;
use mcrs_core::ResourceLocation;
use mcrs_core::voxel_shape::Aabb;
use mcrs_protocol::BlockStateId;

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
        const HAS_ANALOG_OUTPUT_SIGNAL = 1 << 11;
        const REDSTONE_CONDUCTOR = 1 << 12;
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ShapeId(pub u32);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct FluidId(pub u16);

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FluidState {
    pub fluid: FluidId,
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
    pub instrument: Instrument,
    pub render_shape: RenderShape,
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
}

impl BlockEntry {
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
#[derive(Debug, Resource)]
pub struct BlockDefinitions {
    states: Vec<BlockStateData>,
    shapes: Vec<Box<[Aabb]>>,
    fluids: Vec<ResourceLocation<Arc<str>>>,
    blocks: Vec<BlockEntry>,
    by_identifier: FxHashMap<Arc<str>, usize>,
}

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
    pub fn fluid(&self, id: FluidId) -> &ResourceLocation<Arc<str>> {
        &self.fluids[id.0 as usize]
    }

    pub fn state_count(&self) -> usize {
        self.states.len()
    }

    pub fn blocks(&self) -> &[BlockEntry] {
        &self.blocks
    }

    pub fn block(&self, identifier: &str) -> Option<&BlockEntry> {
        self.by_identifier.get(identifier).map(|&i| &self.blocks[i])
    }

    pub fn table_bytes(&self) -> usize {
        let states = self.states.len() * size_of::<BlockStateData>();
        let shapes: usize = self
            .shapes
            .iter()
            .map(|s| size_of::<Box<[Aabb]>>() + s.len() * size_of::<Aabb>())
            .sum();
        states + shapes
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
    #[error("the default asset source is missing")]
    NoAssetSource,
    #[error("failed to list `{directory}`: {source}")]
    ListDirectory {
        directory: String,
        source: bevy_asset::io::AssetReaderError,
    },
    #[error("failed to read `{path}`: {source}")]
    Read {
        path: String,
        source: bevy_asset::io::AssetReaderError,
    },
    #[error("failed to parse `{path}`: {source}")]
    Parse {
        path: String,
        source: serde_json::Error,
    },
    #[error("`{block}`: {source}")]
    Block { block: String, source: BlockError },
    #[error("block state {0} is claimed by no block")]
    UnclaimedState(u16),
}

#[derive(Debug, thiserror::Error)]
pub enum BlockError {
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
    #[error("`mcrs:state_components` holds {found} entries for {states} states")]
    StateComponentsLength { found: usize, states: usize },
    #[error("`{0}` is stated by both a permutation and `mcrs:state_components`")]
    StateComponentsCollision(&'static str),
    #[error("state {state} states no `{component}`")]
    MissingComponent { state: u16, component: &'static str },
}

pub fn load_block_definitions(
    asset_server: &AssetServer,
) -> Result<(BlockDefinitions, LoadReport), LoadError> {
    let started = Instant::now();
    let source = asset_server
        .get_source(AssetSourceId::Default)
        .map_err(|_| LoadError::NoAssetSource)?;
    let reader = source.reader();

    let mut paths = block_on(async {
        let mut stream = reader
            .read_directory(Path::new(CORPUS_DIRECTORY))
            .await
            .map_err(|source| LoadError::ListDirectory {
                directory: CORPUS_DIRECTORY.into(),
                source,
            })?;
        let mut paths = Vec::new();
        while let Some(path) = stream.next().await {
            if path.extension().is_some_and(|e| e == "json") {
                paths.push(path);
            }
        }
        Ok::<Vec<PathBuf>, LoadError>(paths)
    })?;
    paths.sort();

    let mut builder = Builder::new();
    for path in &paths {
        let display = path.display().to_string();
        let bytes = block_on(async {
            let mut file = reader.read(path).await.map_err(|source| LoadError::Read {
                path: display.clone(),
                source,
            })?;
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)
                .await
                .map_err(|e| LoadError::Read {
                    path: display.clone(),
                    source: e.into(),
                })?;
            Ok::<Vec<u8>, LoadError>(bytes)
        })?;
        let file: BlockDefinitionFile =
            serde_json::from_slice(&bytes).map_err(|source| LoadError::Parse {
                path: display.clone(),
                source,
            })?;
        let block = file.block.description.identifier.as_str().to_owned();
        builder
            .add(file)
            .map_err(|source| LoadError::Block { block, source })?;
    }

    let definitions = builder.finish()?;
    let report = LoadReport {
        files: paths.len(),
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
    instrument: Instrument::Harp,
    render_shape: RenderShape::Model,
    fluid: None,
    flags: BlockStateFlags::empty(),
};

struct Builder {
    states: Vec<BlockStateData>,
    owners: Vec<u32>,
    shapes: Vec<Box<[Aabb]>>,
    shape_ids: FxHashMap<Vec<u32>, ShapeId>,
    fluids: Vec<ResourceLocation<Arc<str>>>,
    blocks: Vec<BlockEntry>,
    permutations: usize,
}

const NO_OWNER: u32 = u32::MAX;

impl Builder {
    fn new() -> Self {
        Builder {
            states: Vec::new(),
            owners: Vec::new(),
            shapes: Vec::new(),
            shape_ids: FxHashMap::with_hasher(FxBuildHasher),
            fluids: Vec::new(),
            blocks: Vec::new(),
            permutations: 0,
        }
    }

    fn intern_shape(&mut self, boxes: &[BlockBox]) -> ShapeId {
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

    fn intern_fluid(&mut self, fluid: &ResourceLocation<Arc<str>>) -> FluidId {
        match self
            .fluids
            .iter()
            .position(|f| f.as_str() == fluid.as_str())
        {
            Some(index) => FluidId(index as u16),
            None => {
                self.fluids.push(fluid.clone());
                FluidId(self.fluids.len() as u16 - 1)
            }
        }
    }

    fn add(&mut self, file: BlockDefinitionFile) -> Result<(), BlockError> {
        let description = file.block.description;
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
        let state_components = file.block.state_components;
        if let Some(per_state) = &state_components
            && per_state.len() != state_count
        {
            return Err(BlockError::StateComponentsLength {
                found: per_state.len(),
                states: state_count,
            });
        }

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

        if let Some(per_state) = &state_components {
            let mut dense = Vec::new();
            for entry in per_state {
                entry.present(&mut dense);
            }
            let mut varying = Vec::new();
            for (_, components) in &permutations {
                components.present(&mut varying);
            }
            if let Some(name) = dense.iter().find(|name| varying.contains(name)) {
                return Err(BlockError::StateComponentsCollision(name));
            }
        }

        let block_index = self.blocks.len() as u32;
        let end = base as usize + state_count;
        if self.states.len() < end {
            self.states.resize(end, UNCLAIMED);
            self.owners.resize(end, NO_OWNER);
        }

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
            if let Some(per_state) = &state_components {
                resolved.overlay(&per_state[index]);
            }

            let state = base as usize + index;
            if self.owners[state] != NO_OWNER {
                return Err(BlockError::OverlappingState {
                    state: state as u16,
                    owner: self.blocks[self.owners[state] as usize]
                        .identifier
                        .as_str()
                        .to_owned(),
                });
            }
            self.owners[state] = block_index;
            if let Some(component) = resolved.missing() {
                return Err(BlockError::MissingComponent {
                    state: state as u16,
                    component,
                });
            }
            let data = self.resolve(&resolved);
            self.states[state] = data;
        }

        self.blocks.push(BlockEntry {
            identifier: description.identifier,
            base_state_id: BlockStateId(base),
            default_state_id: BlockStateId(default),
            state_count: state_count as u16,
            properties,
        });
        Ok(())
    }

    /// Every component a state may not leave unstated is present here:
    /// [`Components::missing`] has already run against these components.
    fn resolve(&mut self, components: &Components) -> BlockStateData {
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
            components.requires_correct_tool_for_drops,
            BlockStateFlags::REQUIRES_CORRECT_TOOL_FOR_DROPS,
        );
        flag(components.is_air, BlockStateFlags::IS_AIR);
        flag(components.replaceable, BlockStateFlags::REPLACEABLE);
        flag(components.ignited_by_lava, BlockStateFlags::IGNITED_BY_LAVA);
        flag(
            components.use_shape_for_light_occlusion,
            BlockStateFlags::USE_SHAPE_FOR_LIGHT_OCCLUSION,
        );
        flag(
            components.propagates_skylight_down,
            BlockStateFlags::PROPAGATES_SKYLIGHT_DOWN,
        );
        flag(
            components.emissive_rendering,
            BlockStateFlags::EMISSIVE_RENDERING,
        );
        flag(components.is_solid_render, BlockStateFlags::IS_SOLID_RENDER);
        flag(
            components.is_collision_shape_full_block,
            BlockStateFlags::IS_COLLISION_SHAPE_FULL_BLOCK,
        );
        flag(
            components.has_block_entity,
            BlockStateFlags::HAS_BLOCK_ENTITY,
        );
        flag(
            components.is_signal_source,
            BlockStateFlags::IS_SIGNAL_SOURCE,
        );
        flag(
            components.has_analog_output_signal,
            BlockStateFlags::HAS_ANALOG_OUTPUT_SIGNAL,
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
        let fluid = components.fluid_state.as_ref().map(|fluid| FluidState {
            fluid: self.intern_fluid(&fluid.fluid),
            level: fluid.level,
            source: fluid.source,
        });

        BlockStateData {
            light_emission: components.light_emission.unwrap(),
            light_dampening: components.light_dampening.unwrap(),
            friction: components.friction.unwrap(),
            hardness: components.hardness.unwrap(),
            explosion_resistance: components
                .destructible_by_explosion
                .unwrap()
                .explosion_resistance,
            map_color: components.map_color.unwrap(),
            collision_shape,
            selection_shape,
            occlusion_shape,
            push_reaction: components.push_reaction.unwrap(),
            instrument: components.instrument.unwrap(),
            render_shape: components.render_shape.unwrap(),
            fluid,
            flags,
        }
    }

    fn finish(&mut self) -> Result<BlockDefinitions, LoadError> {
        if let Some(state) = self.owners.iter().position(|&owner| owner == NO_OWNER) {
            return Err(LoadError::UnclaimedState(state as u16));
        }
        let by_identifier = self
            .blocks
            .iter()
            .enumerate()
            .map(|(index, block)| (block.identifier.as_str().into(), index))
            .collect();
        Ok(BlockDefinitions {
            states: std::mem::take(&mut self.states),
            shapes: std::mem::take(&mut self.shapes),
            fluids: std::mem::take(&mut self.fluids),
            blocks: std::mem::take(&mut self.blocks),
            by_identifier,
        })
    }
}

/// Bedrock states a box in sixteenths, from the block centre on X and Z and
/// the block bottom on Y. The engine's own convention is Java's: a 0..1 box
/// from the block's lower corner.
fn to_engine_aabb(value: &BlockBox) -> Aabb {
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

    const COMPONENTS: &str = r##"{
        "minecraft:light_emission": 0,
        "minecraft:light_dampening": 15,
        "minecraft:friction": 0.6,
        "minecraft:map_color": "#707070",
        "minecraft:collision_box": true,
        "minecraft:selection_box": true,
        "minecraft:destructible_by_explosion": { "explosion_resistance": 6 },
        "minecraft:redstone_conductivity": { "redstone_conductor": true },
        "mcrs:hardness": 1.5,
        "mcrs:requires_correct_tool_for_drops": true,
        "mcrs:is_air": false,
        "mcrs:replaceable": false,
        "mcrs:ignited_by_lava": false,
        "mcrs:push_reaction": "push_pull",
        "mcrs:instrument": "basedrum",
        "mcrs:use_shape_for_light_occlusion": false,
        "mcrs:render_shape": "model",
        "mcrs:occlusion_shape": true,
        "mcrs:propagates_skylight_down": false,
        "mcrs:emissive_rendering": false,
        "mcrs:is_solid_render": true,
        "mcrs:is_collision_shape_full_block": true,
        "mcrs:has_block_entity": false,
        "mcrs:is_signal_source": false,
        "mcrs:has_analog_output_signal": false
    }"##;

    fn file(identifier: &str, base: u16, properties: &str, rest: &str) -> String {
        format!(
            r#"{{"format_version": "1.21.130", "minecraft:block": {{
                "description": {{
                    "identifier": "{identifier}",
                    "base_state_id": {base},
                    "default_state_id": {base}
                    {properties}
                }},
                "components": {COMPONENTS}
                {rest}
            }}}}"#
        )
    }

    fn build(files: &[String]) -> Result<BlockDefinitions, LoadError> {
        let mut builder = Builder::new();
        for source in files {
            let file: BlockDefinitionFile = serde_json::from_str(source).expect("valid json");
            let block = file.block.description.identifier.as_str().to_owned();
            builder
                .add(file)
                .map_err(|source| LoadError::Block { block, source })?;
        }
        builder.finish()
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
        source = source.replace(r#""mcrs:hardness": 1.5,"#, "");
        let error = build(&[source]).unwrap_err().to_string();
        assert!(
            error.contains("state 0 states no `mcrs:hardness`"),
            "{error}"
        );
    }

    #[test]
    fn a_short_dense_table_fails_at_load() {
        let source = file(
            "minecraft:test",
            0,
            r#", "properties": { "level": [0, 1, 2] }"#,
            r#", "mcrs:state_components": [{}, {}]"#,
        );
        let error = build(&[source]).unwrap_err().to_string();
        assert!(error.contains("holds 2 entries for 3 states"), "{error}");
    }

    #[test]
    fn a_component_stated_twice_over_fails_at_load() {
        let source = file(
            "minecraft:test",
            0,
            r#", "properties": { "level": [0, 1] }"#,
            r#", "permutations": [{
                "condition": "q.block_state('level') == 0",
                "components": { "minecraft:light_emission": 7 }
            }],
            "mcrs:state_components": [
                { "minecraft:light_emission": 1 },
                { "minecraft:light_emission": 2 }
            ]"#,
        );
        let error = build(&[source]).unwrap_err().to_string();
        assert!(
            error.contains(
                "`minecraft:light_emission` is stated by both a permutation and \
                 `mcrs:state_components`"
            ),
            "{error}"
        );
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
    fn a_bedrock_box_becomes_a_block_local_box() {
        let torch = BlockBox {
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
            to_engine_aabb(&BlockBox::FULL_CUBE),
            Aabb {
                min: Vec3::ZERO,
                max: Vec3::ONE
            }
        );
    }
}
