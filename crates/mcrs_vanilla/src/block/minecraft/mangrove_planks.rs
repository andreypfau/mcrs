use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "mangrove_planks",
    protocol_id: 22,
    base_state_id: 26,
    block_properties: behaviour::Properties::new()
}
