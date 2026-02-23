use crate::block::behaviour;
use crate::block::Block;
use crate::material::map::MapColor;

define_block! {
    name: "coarse_dirt",
    protocol_id: 10,
    base_state_id: 11,
    block_properties: behaviour::Properties::new()
        .with_map_color(MapColor::DIRT)
        .with_strength(0.5)
}
