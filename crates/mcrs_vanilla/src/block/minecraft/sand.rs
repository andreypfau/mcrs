use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "sand",
    protocol_id: 37,
    base_state_id: 118,
    block_properties: behaviour::Properties::new()
}
