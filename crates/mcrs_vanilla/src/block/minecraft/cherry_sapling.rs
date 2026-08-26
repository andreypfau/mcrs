use crate::block::behaviour;
use crate::block::state_properties;
use crate::block::Block;

// Block type: SaplingBlock - not fully implemented yet
// .sound(SoundType.CHERRY_SAPLING) - not implemented yet
define_block! {
    name: "cherry_sapling",
    protocol_id: 30,
    base_state_id: 39,
    properties: [&state_properties::STAGE],
    default: { stage: 0 },
    block_properties: behaviour::Properties::new()
}
