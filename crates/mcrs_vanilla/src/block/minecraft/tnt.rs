use crate::block::behaviour;
use crate::block::state_properties;
use crate::block::Block;
use crate::material::map::MapColor;

define_block! {
    name: "tnt",
    protocol_id: 176,
    base_state_id: 2140,
    properties: [&state_properties::UNSTABLE],
    default: { unstable: false },
    block_properties: behaviour::Properties::new()
        .with_map_color(MapColor::FIRE)
        .with_strength(0.0)
        .ignited_by_lava()
        .instant_break()
}
