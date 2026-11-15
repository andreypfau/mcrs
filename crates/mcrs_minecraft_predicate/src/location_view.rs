use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_block::definition::FluidState;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_entity::spawn_condition::Structure;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_resource::Id;
use mcrs_voxel::{BlockPos, BlockStateId};

/// The world as a location predicate reads it, in the dimension that the caller evaluates in.
/// `None` means that the host does not have the value, for example at a position that is not
/// loaded; the test that reads it fails.
pub trait LocationView {
    /// The block state at `pos`.
    fn block_state(&self, pos: BlockPos) -> Option<BlockStateId>;

    /// Whether `pos` is inside the build height and the coordinate limits, in a chunk that the
    /// host holds with its full data. A host that cannot tell answers `false`.
    fn is_loaded(&self, pos: BlockPos) -> bool;

    /// The fluid state at `pos`; a position with no fluid has the fluid `minecraft:empty`.
    fn fluid_state(&self, pos: BlockPos) -> Option<FluidState>;

    /// The block light at `pos`, from 0 to 15.
    fn block_light(&self, pos: BlockPos) -> Option<u8>;

    /// The sky light at `pos`, from 0 to 15, without the darkening of the time of day. A dimension
    /// without sky light answers `Some(0)`; `None` only when the host does not have the value.
    fn sky_light(&self, pos: BlockPos) -> Option<u8>;

    /// The value of the environment attribute `minecraft:gameplay/sky_light_level` of the
    /// dimension.
    fn sky_light_level(&self) -> Option<f32>;

    /// The biome at `pos`.
    fn biome(&self, pos: BlockPos) -> Option<Id<Biome>>;

    /// Whether `pos` is inside one piece of a structure of the kind `structure`.
    fn inside_structure(&self, pos: BlockPos, structure: Id<Structure>) -> Option<bool>;

    /// The id of the dimension in the dimension registry of the registry set of the bake.
    fn dimension(&self) -> Option<Id<Dimension>>;

    /// Writes the saved data of the block entity at `pos` into `data`, which the caller passes
    /// empty. `false` when the position has no block entity or the host does not have its data.
    fn block_entity_data(&self, pos: BlockPos, data: &mut NbtCompound) -> bool;
}
