use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "sandstone",
    protocol_id: 48,
    base_state_id: 120,
    block_properties: behaviour::Properties::new()
}
