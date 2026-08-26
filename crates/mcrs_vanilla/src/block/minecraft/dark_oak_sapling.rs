use crate::block::behaviour;
use crate::block::state_properties;
use crate::block::Block;

// Block type: SaplingBlock - not fully implemented yet
define_block! {
    name: "dark_oak_sapling",
    protocol_id: 31,
    base_state_id: 41,
    properties: [&state_properties::STAGE],
    default: { stage: 0 },
    block_properties: behaviour::Properties::new()
}
