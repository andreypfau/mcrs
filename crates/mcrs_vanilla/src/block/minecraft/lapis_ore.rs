use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "lapis_ore",
    protocol_id: 102,
    base_state_id: 563,
    block_properties: behaviour::Properties::new()
        .with_xp_range(2, 5)
}
