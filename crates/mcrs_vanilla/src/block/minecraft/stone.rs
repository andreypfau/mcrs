use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "stone",
    protocol_id: 1,
    base_state_id: 1,
    block_properties: behaviour::Properties::new()
}
