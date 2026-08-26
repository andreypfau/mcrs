use crate::block::Block;
use crate::block::behaviour;

define_block! {
    name: "gold_ore",
    protocol_id: 42,
    base_state_id: 129,
    block_properties: behaviour::Properties::new()
        .with_xp_range(0, 0)
}
