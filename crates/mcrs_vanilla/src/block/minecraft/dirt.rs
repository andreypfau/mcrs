use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "dirt",
    protocol_id: 9,
    base_state_id: 10,
    block_properties: behaviour::Properties::new()
}
