use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "coarse_dirt",
    protocol_id: 10,
    base_state_id: 11,
    block_properties: behaviour::Properties::new()
}
