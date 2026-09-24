pub mod after_place;
pub mod buried_treasure;
pub mod canvas;
pub mod end_city;
pub mod fortress;
pub mod jungle_temple;
pub mod mineshaft;
pub mod nether_fossil;
pub mod ocean_monument;
pub mod portal;
pub mod scattered;
pub mod stronghold;
pub mod template;
pub mod template_piece;
pub mod woodland_mansion;

use bevy_math::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::{BoundingBox, HolderSet, Mirror, ResourceLocation, Rotation};
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::worldgen::WorldgenRandom;
use mcrs_minecraft_random::{Random, block_pos_seed};
use mcrs_minecraft_worldgen_density::proto::BlockState;
use mcrs_minecraft_worldgen_feature::compile::{
    BlockResolver, FeatureCompileError, StateQuery, state_of, states_of,
};
use mcrs_minecraft_worldgen_feature::placer::{StateMask, WorldGenVolume, WorldStates};
use mcrs_minecraft_worldgen_feature::template::FrozenTemplate;
use mcrs_minecraft_worldgen_feature_place::block_entity::GeneratedBlockEntity;
use mcrs_minecraft_worldgen_feature_place::entity::GeneratedEntity;
use mcrs_minecraft_worldgen_feature_place::template::{
    CompiledChain, Placement, SettingsRandom, mirror_state, place_template, rotate_state,
};
use mcrs_minecraft_worldgen_structure::orient::Orientation;

/// One block state as `StructurePiece.placeBlock` mirrors and rotates it
/// under each orientation, resolved once so a painter indexes instead of
/// walking properties by name per block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Oriented(pub [VoxelId; 4]);

impl Oriented {
    pub fn of(world: &WorldStates, state: VoxelId) -> Self {
        Oriented(Orientation::ALL.map(|orientation| {
            let (mirror, rotation) = orientation.mirror_rotation();
            rotate_state(world, mirror_state(world, state, mirror), rotation)
        }))
    }

    pub fn named(
        world: &WorldStates,
        blocks: &dyn BlockResolver,
        block: &str,
        properties: &[(&str, &str)],
    ) -> Result<Self, FeatureCompileError> {
        Ok(Self::of(world, state(blocks, block, properties)?))
    }

    pub fn get(&self, orientation: Orientation) -> VoxelId {
        self.0[orientation as usize]
    }

    /// The state as given, under the orientation that transforms nothing.
    pub fn unoriented(&self) -> VoxelId {
        self.get(Orientation::North)
    }
}

pub fn state(
    blocks: &dyn BlockResolver,
    block: &str,
    properties: &[(&str, &str)],
) -> Result<VoxelId, FeatureCompileError> {
    state_of(
        blocks,
        &BlockState {
            name: ResourceLocation::parse(block).expect("a literal id"),
            properties: (!properties.is_empty()).then(|| {
                properties
                    .iter()
                    .map(|(name, value)| (name.to_string(), value.to_string()))
                    .collect()
            }),
        },
    )
}

pub fn block_mask(
    blocks: &dyn BlockResolver,
    names: &[&str],
) -> Result<StateMask, FeatureCompileError> {
    let ids = names
        .iter()
        .map(|name| ResourceLocation::parse(name).expect("a literal id"))
        .collect();
    states_of(blocks, StateQuery::Blocks(&HolderSet::List(ids)))
}

/// `TemplateStructurePiece.postProcess`: the palette drawn from the piece's
/// position, then the template placed with positional randomness. The palette
/// comes back when anything was placed.
#[allow(clippy::too_many_arguments)]
pub fn place_positional<W: WorldGenVolume>(
    template: &FrozenTemplate,
    position: IVec3,
    rotation: Rotation,
    mirror: Mirror,
    pivot: IVec3,
    clip: BoundingBox,
    chain: &CompiledChain,
    waterlog: bool,
    place_entities: bool,
    reference: IVec3,
    volume: &mut W,
    rng: &mut WorldgenRandom,
    entities: &mut Vec<GeneratedBlockEntity>,
    spawns: &mut Vec<GeneratedEntity>,
) -> Option<usize> {
    if template.palettes.is_empty() {
        return None;
    }
    let palette = LegacyRandom::new(block_pos_seed(position))
        .next_i32_bound(template.palettes.len() as i32) as usize;
    place_template(
        &Placement {
            template,
            jigsaws: &[],
            palette,
            position,
            reference,
            rotation,
            mirror,
            pivot,
            random: SettingsRandom::Positional,
            clip: Some(clip),
            chain,
            waterlog,
            place_entities,
        },
        volume,
        rng,
        entities,
        spawns,
    )
    .then_some(palette)
}
