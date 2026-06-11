use crate::block::behaviour;
use crate::block::Block;
use crate::material::map::MapColor;

define_block! {
    name: "sand",
    protocol_id: 37,
    base_state_id: 118,
    block_properties: behaviour::Properties::new()
        .with_map_color(MapColor::SAND)
        .with_strength(0.5)
}
