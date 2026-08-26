use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "torch",
    protocol_id: 194,
    base_state_id: 3370,
    block_properties: behaviour::Properties::new()
}
