use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_worldgen_density::proto::BlockState as ProtoBlockState;

pub fn try_resolve_state(
    blocks: &BlockDefinitions,
    state: &ProtoBlockState,
) -> Option<mcrs_minecraft_registry::BlockStateId> {
    let block = blocks.block(state.name.as_str())?;
    let mut id = block.default_state_id;
    for (property, value) in state.properties.iter().flatten() {
        id = block.with_text(id, property, value)?;
    }
    Some(id)
}
