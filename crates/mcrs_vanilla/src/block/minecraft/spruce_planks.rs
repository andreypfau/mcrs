use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "spruce_planks",
    protocol_id: 14,
    base_state_id: 16,
    block_properties: behaviour::Properties::new()
}
