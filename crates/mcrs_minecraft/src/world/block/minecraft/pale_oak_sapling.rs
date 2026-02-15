use crate::world::block::behaviour::Properties;
use crate::world::block::{Block, BlockState};
use crate::world::material::PushReaction;
use crate::world::material::map::MapColor;
use mcrs_protocol::{BlockStateId, ident};

pub const BLOCK: Block = Block {
    identifier: ident!("pale_oak_sapling"),
    protocol_id: 32,
    properties: &PROPERTIES,
    default_state: &DEFAULT_STATE,
    states: &[STAGE_0, STAGE_1_STATE],
};

pub const STAGE_0: BlockState = BlockState {
    id: BlockStateId(43),
};

pub const STAGE_1_STATE: BlockState = BlockState {
    id: BlockStateId(44),
};

pub const DEFAULT_STATE: &BlockState = &STAGE_0;

// Block type: SaplingBlock - not fully implemented yet
pub const PROPERTIES: Properties = Properties::new()
    .with_map_color(MapColor::METAL)
    .with_strength(0.0)
    .with_random_ticks()
    .no_collision()
    .instant_break()
    .with_push_reaction(PushReaction::Destroy);
