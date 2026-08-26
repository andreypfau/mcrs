use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "cobblestone",
    protocol_id: 12,
    base_state_id: 14,
    block_properties: behaviour::Properties::new()
}
