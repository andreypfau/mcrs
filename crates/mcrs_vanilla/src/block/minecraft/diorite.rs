use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "diorite",
    protocol_id: 4,
    base_state_id: 4,
    block_properties: behaviour::Properties::new()
}
