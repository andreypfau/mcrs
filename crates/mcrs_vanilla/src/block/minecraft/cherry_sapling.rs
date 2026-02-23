use crate::block::behaviour;
use crate::block::state_properties;
use crate::block::Block;
use crate::material::map::MapColor;
use crate::material::PushReaction;

// Block type: SaplingBlock - not fully implemented yet
// .sound(SoundType.CHERRY_SAPLING) - not implemented yet
define_block! {
    name: "cherry_sapling",
    protocol_id: 30,
    base_state_id: 39,
    properties: [&state_properties::STAGE],
    default: { stage: 0 },
    block_properties: behaviour::Properties::new()
        .with_map_color(MapColor::COLOR_PINK)
        .with_strength(0.0)
        .with_random_ticks()
        .no_collision()
        .instant_break()
        .with_push_reaction(PushReaction::Destroy)
}
