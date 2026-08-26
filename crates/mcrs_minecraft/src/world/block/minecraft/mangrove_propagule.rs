use crate::block_state_idents;
use crate::sound::SoundType;
use mcrs_minecraft_block::material::PushReaction;
use mcrs_minecraft_block::material::map::MapColor;
use mcrs_protocol::{BlockStateId, ident};
use crate::world::block::{Block, BlockState};
use crate::generate_block_states;
use crate::world::block::behaviour::Properties;

generate_block_states! {
    base_id: 45,
    block_name: "mangrove_propagule",
    protocol_id: 33,
    state_properties: {
        age: [0, 1, 2, 3, 4],
        hanging: [true, false],
        stage: [0, 1],
        waterlogged: [true, false]
    },
    default: { age:0, hanging:false, stage:0, waterlogged:false },
    block_properties: Properties::new()
}
