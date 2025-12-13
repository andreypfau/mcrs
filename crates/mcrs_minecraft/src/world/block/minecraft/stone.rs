use crate::world::block::behaviour::Properties;
use crate::world::block::{Block, BlockState};
use mcrs_protocol::{BlockStateId, ident};

pub const BLOCK: Block = Block {
    identifier: ident!("stone"),
    properties: &PROPERTIES,
    default_state: &DEFAULT_STATE,
    states: &[DEFAULT_STATE],
};

pub const DEFAULT_STATE: BlockState = BlockState {
    id: BlockStateId(1),
};

pub const PROPERTIES: Properties = Properties::new()
    .requires_correct_tool_for_drops()
    .hardness(1.5)
    .explosion_resistance(6.0);
