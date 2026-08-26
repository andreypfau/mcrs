use crate::block::Block;
use crate::block::behaviour;
use crate::block::state_properties;

define_block! {
    name: "wall_torch",
    protocol_id: 195,
    base_state_id: 3371,
    properties: [&state_properties::FACING_HORIZONTAL],
    default: { facing: north },
    block_properties: behaviour::Properties::new()
}
