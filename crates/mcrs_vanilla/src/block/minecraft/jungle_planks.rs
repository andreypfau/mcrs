use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "jungle_planks",
    protocol_id: 16,
    base_state_id: 18,
    block_properties: behaviour::Properties::new()
}
