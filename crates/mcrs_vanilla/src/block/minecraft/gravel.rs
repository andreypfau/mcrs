use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "gravel",
    protocol_id: 38,
    base_state_id: 119,
    block_properties: behaviour::Properties::new()
}
