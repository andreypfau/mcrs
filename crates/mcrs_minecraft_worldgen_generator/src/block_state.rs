use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_block_predicate::block_state::BlockState as ProtoBlockState;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::BlockStateId;

pub(crate) fn states(blocks: &BlockDefinitions) -> impl Iterator<Item = BlockStateId> {
    let count = blocks.state_count();
    (0..count).filter_map(move |number| BlockStateId::checked(u16::try_from(number).ok()?, count))
}

pub(crate) fn state_of(blocks: &BlockDefinitions, voxel: VoxelId) -> Option<BlockStateId> {
    BlockStateId::checked(voxel.0, blocks.state_count())
}

pub fn try_resolve_state(
    blocks: &BlockDefinitions,
    state: &ProtoBlockState,
) -> Option<BlockStateId> {
    let block = blocks.block(state.name.as_str())?;
    let mut id = block.default_state_id;
    for (property, value) in state.properties.iter().flatten() {
        id = block.with_text(id, property, value)?;
    }
    Some(id)
}
