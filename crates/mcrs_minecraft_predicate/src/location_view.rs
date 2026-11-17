use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_block::definition::FluidState;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_entity::spawn_condition::Structure;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_resource::Id;
use mcrs_voxel::{BlockPos, BlockStateId};

pub trait LocationView {
    fn block_state(&self, pos: BlockPos) -> Option<BlockStateId>;

    fn is_loaded(&self, pos: BlockPos) -> bool;

    fn fluid_state(&self, pos: BlockPos) -> Option<FluidState>;

    fn block_light(&self, pos: BlockPos) -> Option<u8>;

    fn sky_light(&self, pos: BlockPos) -> Option<u8>;

    fn sky_light_level(&self) -> Option<f32>;

    fn biome(&self, pos: BlockPos) -> Option<Id<Biome>>;

    fn inside_structure(&self, pos: BlockPos, structure: Id<Structure>) -> Option<bool>;

    fn dimension(&self) -> Option<Id<Dimension>>;

    fn block_entity_data(&self, pos: BlockPos, data: &mut NbtCompound) -> bool;
}
