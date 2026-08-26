use crate::block::behaviour;
use crate::block::state_properties;
use crate::block::Block;

// Block type: GrassBlock - not fully implemented yet
define_block! {
    name: "grass_block",
    protocol_id: 8,
    base_state_id: 8,
    properties: [&state_properties::SNOWY],
    default: { snowy: false },
    block_properties: behaviour::Properties::new()
}
