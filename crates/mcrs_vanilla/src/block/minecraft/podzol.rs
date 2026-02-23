use crate::block::behaviour;
use crate::block::state_properties;
use crate::block::Block;
use crate::material::map::MapColor;

// Block type: SnowyDirtBlock - not fully implemented yet
define_block! {
    name: "podzol",
    protocol_id: 11,
    base_state_id: 12,
    properties: [&state_properties::SNOWY],
    default: { snowy: false },
    block_properties: behaviour::Properties::new()
        .with_map_color(MapColor::PODZOL)
        .with_strength(0.5)
}
