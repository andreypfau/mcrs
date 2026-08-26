use crate::block::behaviour;
use crate::block::state_properties;
use crate::block::Block;

// Block type: SaplingBlock - not fully implemented yet
define_block! {
    name: "pale_oak_sapling",
    protocol_id: 32,
    base_state_id: 43,
    properties: [&state_properties::STAGE],
    default: { stage: 0 },
    block_properties: behaviour::Properties::new()
}
