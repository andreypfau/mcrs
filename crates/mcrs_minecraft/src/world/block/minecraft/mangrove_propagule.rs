use crate::block_state_idents;
use crate::generate_block_states;
use crate::sound::SoundType;
use crate::world::block::behaviour::Properties;
use crate::world::block::{Block, BlockState};
use crate::world::material::PushReaction;
use crate::world::material::map::MapColor;
use mcrs_protocol::{BlockStateId, ident};

generate_block_states! {
    base_id: 45,
    block_name: "mangrove_propagule",
    state_properties: {
        age: [0, 1, 2, 3, 4],
        hanging: [true, false],
        stage: [0, 1],
        waterlogged: [true, false]
    },
    default: { age:0, hanging:false, stage:0, waterlogged:false },
    block_properties: Properties::new()
        .with_map_color(MapColor::PLANT)
        .no_collision()
        .with_random_ticks()
        .instant_break()
        .with_sound(&SoundType::GRASS)
        .with_push_reaction(PushReaction::Destroy)
}
