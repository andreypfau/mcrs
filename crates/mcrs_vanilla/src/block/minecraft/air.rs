use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "air",
    protocol_id: 0,
    base_state_id: 0,
    block_properties: behaviour::Properties::new()
}
