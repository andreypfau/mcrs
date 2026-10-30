use mcrs_minecraft_block::definition::schema::BlockProperty;
use mcrs_minecraft_command::{Identifier, Lookup, NoLookup};

/// What a block predicate reads from the world of the source besides its registries.
pub trait BlockLookup: Lookup {
    /// The properties of a block in their order, or `None` when no block has that identifier.
    fn block_properties(&self, block: Identifier<'_>) -> Option<&[BlockProperty]>;

    /// Whether the states of a block have a block entity.
    fn has_block_entity(&self, block: Identifier<'_>) -> bool;
}

impl BlockLookup for NoLookup {
    fn block_properties(&self, _: Identifier<'_>) -> Option<&[BlockProperty]> {
        None
    }

    fn has_block_entity(&self, _: Identifier<'_>) -> bool {
        false
    }
}
