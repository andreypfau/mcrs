use crate::block::behaviour;
use crate::block::Block;
use crate::material::map::MapColor;

define_block! {
    name: "gravel",
    protocol_id: 38,
    base_state_id: 119,
    block_properties: behaviour::Properties::new()
        .with_map_color(MapColor::STONE)
        .with_strength(0.6)
}
