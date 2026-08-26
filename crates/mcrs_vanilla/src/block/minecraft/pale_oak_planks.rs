use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "pale_oak_planks",
    protocol_id: 21,
    base_state_id: 25,
    block_properties: behaviour::Properties::new()
}
