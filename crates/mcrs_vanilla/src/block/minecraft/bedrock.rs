use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "bedrock",
    protocol_id: 34,
    base_state_id: 85,
    block_properties: behaviour::Properties::new()
}
