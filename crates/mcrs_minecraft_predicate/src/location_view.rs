use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_block::definition::FluidState;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_entity::spawn_condition::Structure;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_resource::Id;
use mcrs_voxel::{BlockPos, BlockStateId};

pub trait LocationView {
    fn block_state(&self, pos: BlockPos) -> Option<BlockStateId>;

    fn is_loaded(&self, pos: BlockPos) -> bool {
        self.block_state(pos).is_some()
    }

    fn fluid_state(&self, _pos: BlockPos) -> Option<FluidState> {
        None
    }

    fn block_light(&self, _pos: BlockPos) -> Option<u8> {
        None
    }

    fn sky_light(&self, _pos: BlockPos) -> Option<u8> {
        None
    }

    fn sky_light_level(&self) -> Option<f32> {
        None
    }

    fn biome(&self, _pos: BlockPos) -> Option<Id<Biome>> {
        None
    }

    fn inside_structure(&self, _pos: BlockPos, _structure: Id<Structure>) -> Option<bool> {
        None
    }

    fn dimension(&self) -> Option<Id<Dimension>> {
        None
    }

    fn block_entity_data(&self, _pos: BlockPos, _data: &mut NbtCompound) -> bool {
        false
    }
}
