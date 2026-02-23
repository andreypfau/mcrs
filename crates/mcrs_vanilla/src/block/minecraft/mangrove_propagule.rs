use crate::block::behaviour;
use crate::block::state_properties;
use crate::block::Block;
use crate::material::map::MapColor;
use crate::material::PushReaction;
use crate::sound::SoundType;

define_block! {
    name: "mangrove_propagule",
    protocol_id: 33,
    base_state_id: 45,
    properties: [&state_properties::AGE_4, &state_properties::HANGING, &state_properties::STAGE, &state_properties::WATERLOGGED],
    default: { age: 0, hanging: false, stage: 0, waterlogged: false },
    block_properties: behaviour::Properties::new()
        .with_map_color(MapColor::PLANT)
        .no_collision()
        .with_random_ticks()
        .instant_break()
        .with_sound(&SoundType::GRASS)
        .with_push_reaction(PushReaction::Destroy)
}
