use crate::block::Block;
use crate::block::behaviour;
use crate::block::state_properties;

define_block! {
    name: "redstone_ore",
    protocol_id: 271,
    base_state_id: 6881,
    properties: [&state_properties::LIT],
    default: { lit: false },
    block_properties: behaviour::Properties::new()
        .with_xp_range(1, 5)
}
