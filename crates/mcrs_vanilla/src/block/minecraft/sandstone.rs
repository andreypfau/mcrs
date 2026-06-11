use crate::block::behaviour;
use crate::block::Block;
use crate::material::map::MapColor;

define_block! {
    name: "sandstone",
    protocol_id: 48,
    base_state_id: 120,
    block_properties: behaviour::Properties::new()
        .with_map_color(MapColor::SAND)
        .with_hardness(0.8)
        .with_explosion_resistance(0.8)
        .requires_correct_tool_for_drops()
}
