use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "dark_oak_planks",
    protocol_id: 19,
    base_state_id: 21,
    block_properties: behaviour::Properties::new()
}
