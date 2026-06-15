use crate::block::behaviour;
use crate::block::Block;
use crate::material::map::MapColor;

define_block! {
    name: "clay",
    protocol_id: 281,
    base_state_id: 6946,
    block_properties: behaviour::Properties::new()
        .with_map_color(MapColor::CLAY)
        .with_hardness(0.6)
        .with_explosion_resistance(0.6)
}
