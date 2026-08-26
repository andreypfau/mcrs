use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "oak_planks",
    protocol_id: 13,
    base_state_id: 15,
    block_properties: behaviour::Properties::new()
}
