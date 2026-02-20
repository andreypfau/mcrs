use crate::block::behaviour::Properties;
use crate::block::{Block, BlockState};
use crate::material::PushReaction;
use crate::material::map::MapColor;
use mcrs_protocol::BlockStateId;

pub const BLOCK: Block = Block {
    identifier: mcrs_core::rl!("jungle_sapling"),
    protocol_id: 28,
    properties: &PROPERTIES,
    default_state: &DEFAULT_STATE,
    states: &[STAGE_0_STATE, STAGE_1_STATE],
};

pub const STAGE_0_STATE: BlockState = BlockState {
    id: BlockStateId(35),
};

pub const STAGE_1_STATE: BlockState = BlockState {
    id: BlockStateId(36),
};

pub const DEFAULT_STATE: &BlockState = &STAGE_0_STATE;

// Block type: SaplingBlock - not fully implemented yet
pub const PROPERTIES: Properties = Properties::new()
    .with_map_color(MapColor::PLANT)
    .with_strength(0.0)
    .with_random_ticks()
    .no_collision()
    .instant_break()
    .with_push_reaction(PushReaction::Destroy);
