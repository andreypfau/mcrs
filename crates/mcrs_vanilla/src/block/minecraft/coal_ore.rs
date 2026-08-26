use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "coal_ore",
    protocol_id: 46,
    base_state_id: 133,
    block_properties: behaviour::Properties::new()
        .with_xp_range(0, 2)
}
