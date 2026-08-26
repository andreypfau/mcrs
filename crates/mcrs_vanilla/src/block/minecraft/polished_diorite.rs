use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "polished_diorite",
    protocol_id: 5,
    base_state_id: 5,
    block_properties: behaviour::Properties::new()
}
