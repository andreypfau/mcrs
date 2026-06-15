use crate::block::behaviour;
use crate::block::state_properties;
use crate::block::Block;
use crate::material::map::MapColor;

define_block! {
    name: "redstone_ore",
    protocol_id: 271,
    base_state_id: 6881,
    properties: [&state_properties::LIT],
    default: { lit: false },
    block_properties: behaviour::Properties::new()
        .with_map_color(MapColor::STONE)
        .with_hardness(3.0)
        .with_explosion_resistance(3.0)
        .requires_correct_tool_for_drops()
        .with_xp_range(1, 5)
}
