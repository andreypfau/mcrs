use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "ice",
    protocol_id: 277,
    base_state_id: 6927,
    block_properties: behaviour::Properties::new()
}
