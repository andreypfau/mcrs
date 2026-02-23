use crate::block::behaviour;
use crate::block::Block;
use crate::material::map::MapColor;

define_block! {
    name: "dirt",
    protocol_id: 9,
    base_state_id: 10,
    block_properties: behaviour::Properties::new()
        .with_map_color(MapColor::DIRT)
        .with_strength(0.5)
}
