pub mod after_place;
pub mod canvas;
pub mod scattered;

use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::{HolderSet, ResourceLocation};
use mcrs_minecraft_worldgen_density::proto::BlockState;
use mcrs_minecraft_worldgen_feature::compile::{
    BlockResolver, FeatureCompileError, StateQuery, state_of, states_of,
};
use mcrs_minecraft_worldgen_feature::placer::{StateMask, WorldStates};
use mcrs_minecraft_worldgen_feature_place::template::{mirror_state, rotate_state};
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

pub fn block_mask(blocks: &dyn BlockResolver, names: &[&str]) -> Result<StateMask, FeatureCompileError> {
    let ids = names
        .iter()
        .map(|name| ResourceLocation::parse(name).expect("a literal id"))
        .collect();
    states_of(blocks, StateQuery::Blocks(&HolderSet::List(ids)))
}
