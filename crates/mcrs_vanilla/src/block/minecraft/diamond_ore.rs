use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "diamond_ore",
    protocol_id: 202,
    base_state_id: 5106,
    block_properties: behaviour::Properties::new()
        .with_xp_range(3, 7)
}
