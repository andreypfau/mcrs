use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "polished_granite",
    protocol_id: 3,
    base_state_id: 3,
    block_properties: behaviour::Properties::new()
}
