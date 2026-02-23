use crate::block::behaviour;
use crate::block::state_properties;
use crate::block::Block;
use crate::material::map::MapColor;
use crate::material::PushReaction;

// Block type: SaplingBlock - not fully implemented yet
define_block! {
    name: "spruce_sapling",
    protocol_id: 26,
    base_state_id: 31,
    properties: [&state_properties::STAGE],
    default: { stage: 0 },
    block_properties: behaviour::Properties::new()
        .with_map_color(MapColor::PLANT)
        .with_strength(0.0)
        .with_random_ticks()
        .no_collision()
        .instant_break()
        .with_push_reaction(PushReaction::Destroy)
}
