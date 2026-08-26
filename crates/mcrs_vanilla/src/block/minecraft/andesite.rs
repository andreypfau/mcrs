use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "andesite",
    protocol_id: 6,
    base_state_id: 6,
    block_properties: behaviour::Properties::new()
}
