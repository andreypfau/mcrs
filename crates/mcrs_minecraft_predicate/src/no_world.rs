use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_block::definition::FluidState;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_entity::spawn_condition::Structure;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_resource::Id;
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::location_view::LocationView;

/// A host without a world: each value is absent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoWorld;

impl LocationView for NoWorld {
    fn block_state(&self, _: BlockPos) -> Option<BlockStateId> {
        None
    }

    fn is_loaded(&self, _: BlockPos) -> bool {
        false
    }

    fn fluid_state(&self, _: BlockPos) -> Option<FluidState> {
        None
    }

    fn block_light(&self, _: BlockPos) -> Option<u8> {
        None
    }

    fn sky_light(&self, _: BlockPos) -> Option<u8> {
        None
    }

    fn sky_light_level(&self) -> Option<f32> {
        None
    }

    fn biome(&self, _: BlockPos) -> Option<Id<Biome>> {
        None
    }

    fn inside_structure(&self, _: BlockPos, _: Id<Structure>) -> Option<bool> {
        None
    }

    fn dimension(&self) -> Option<Id<Dimension>> {
        None
    }

    fn block_entity_data(&self, _: BlockPos, _: &mut NbtCompound) -> bool {
        false
    }
}
