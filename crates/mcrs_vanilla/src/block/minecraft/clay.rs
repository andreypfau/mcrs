use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "clay",
    protocol_id: 281,
    base_state_id: 6946,
    block_properties: behaviour::Properties::new()
}
