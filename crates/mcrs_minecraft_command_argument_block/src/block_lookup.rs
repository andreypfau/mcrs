use mcrs_minecraft_block::definition::schema::BlockProperty;
use mcrs_minecraft_command::{Lookup, NoLookup};
use mcrs_minecraft_resource::ResourceLocation;

pub trait BlockLookup: Lookup {
    fn block_properties(&self, block: ResourceLocation<'_>) -> Option<&[BlockProperty]>;

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
