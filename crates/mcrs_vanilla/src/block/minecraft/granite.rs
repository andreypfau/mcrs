use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "granite",
    protocol_id: 2,
    base_state_id: 2,
    block_properties: behaviour::Properties::new()
}
