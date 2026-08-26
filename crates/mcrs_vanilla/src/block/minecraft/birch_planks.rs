use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "birch_planks",
    protocol_id: 15,
    base_state_id: 17,
    block_properties: behaviour::Properties::new()
}
