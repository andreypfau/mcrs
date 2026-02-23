use crate::block::behaviour;
use crate::block::Block;
use crate::material::map::MapColor;

define_block! {
    name: "air",
    protocol_id: 0,
    base_state_id: 0,
    block_properties: behaviour::Properties::new()
        .with_map_color(MapColor::NONE)
        .with_strength(0.0)
        .no_collision()
        .replacable()
        .air()
        .with_no_loot_table()
}
