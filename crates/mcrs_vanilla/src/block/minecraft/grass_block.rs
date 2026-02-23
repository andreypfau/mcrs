use crate::block::behaviour;
use crate::block::state_properties;
use crate::block::Block;
use crate::material::map::MapColor;

// Block type: GrassBlock - not fully implemented yet
define_block! {
    name: "grass_block",
    protocol_id: 8,
    base_state_id: 8,
    properties: [&state_properties::SNOWY],
    default: { snowy: false },
    block_properties: behaviour::Properties::new()
        .with_map_color(MapColor::GRASS)
        .with_strength(0.6)
        .with_random_ticks()
}
