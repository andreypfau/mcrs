use crate::block::behaviour;
use crate::block::Block;
use crate::material::map::MapColor;

define_block! {
    name: "ice",
    protocol_id: 277,
    base_state_id: 6927,
    block_properties: behaviour::Properties::new()
        .with_map_color(MapColor::ICE)
        .with_strength(0.5)
}
