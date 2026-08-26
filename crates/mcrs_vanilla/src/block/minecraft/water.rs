use crate::block::Block;
use crate::block::behaviour;
use crate::block::state_properties;

define_block! {
    name: "water",
    protocol_id: 35,
    base_state_id: 86,
    properties: [&state_properties::LEVEL],
    default: { level: 0 },
    block_properties: behaviour::Properties::new()
}
