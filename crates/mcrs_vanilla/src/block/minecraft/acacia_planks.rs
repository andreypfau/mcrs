use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "acacia_planks",
    protocol_id: 17,
    base_state_id: 19,
    block_properties: behaviour::Properties::new()
}
