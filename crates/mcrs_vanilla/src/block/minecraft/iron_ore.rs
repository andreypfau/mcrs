use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "iron_ore",
    protocol_id: 44,
    base_state_id: 131,
    block_properties: behaviour::Properties::new()
        .with_xp_range(0, 0)
}
