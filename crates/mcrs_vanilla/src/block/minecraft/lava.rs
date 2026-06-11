use crate::block::behaviour;
use crate::block::state_properties;
use crate::block::Block;
use crate::material::map::MapColor;

define_block! {
    name: "lava",
    protocol_id: 36,
    base_state_id: 102,
    properties: [&state_properties::LEVEL],
    default: { level: 0 },
    block_properties: behaviour::Properties::new()
        .with_map_color(MapColor::FIRE)
        .with_strength(100.0)
        .with_no_loot_table()
}
