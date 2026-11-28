use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::schema::PropertyValue;
use mcrs_minecraft_block::definition::{BlockDefinitions, BlockStateFlags};
use mcrs_minecraft_resource::Id;
use mcrs_voxel::BlockStateId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spot {
    Clicked,
    Adjacent,
}

pub fn replaceable(
    definitions: &BlockDefinitions,
    state: BlockStateId,
    held: Id<Block>,
    spot: Spot,
) -> bool {
    let Some(block) = definitions.block_index(state) else {
        return false;
    };
    if block == held {
        return false;
    }
    if block == Block::Light.id() {
        return spot == Spot::Adjacent;
    }
    if block == Block::Snow.id() {
        let layers = definitions
            .owner(state)
            .and_then(|entry| entry.value_of(state, "layers"));
        return layers == Some(&PropertyValue::Int(1));
    }
    if block == Block::SculkVein.id() {
        return true;
    }
    definitions
        .state(state)
        .flags
        .contains(BlockStateFlags::REPLACEABLE)
}
