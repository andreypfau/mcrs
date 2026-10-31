use mcrs_minecraft_block::definition::schema::BlockProperty;
use mcrs_minecraft_command::{Lookup, NoLookup};
use mcrs_minecraft_resource::ResourceLocation;

/// What a block predicate reads from the world of the source besides its registries.
pub trait BlockLookup: Lookup {
    /// The properties of a block in their order, or `None` when no block has that identifier.
    fn block_properties(&self, block: ResourceLocation<'_>) -> Option<&[BlockProperty]>;

    /// Whether the states of a block have a block entity.
    fn has_block_entity(&self, block: ResourceLocation<'_>) -> bool;
}

impl BlockLookup for NoLookup {
    fn block_properties(&self, _: ResourceLocation<'_>) -> Option<&[BlockProperty]> {
        None
    }

    fn has_block_entity(&self, _: ResourceLocation<'_>) -> bool {
        false
    }
}
