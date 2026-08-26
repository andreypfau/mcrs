use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "polished_andesite",
    protocol_id: 7,
    base_state_id: 7,
    block_properties: behaviour::Properties::new()
}
