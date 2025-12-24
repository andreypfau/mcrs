use crate::world::block::behaviour::Properties;
use crate::world::block::{Block, BlockState};
use mcrs_protocol::{BlockStateId, ident};

pub const BLOCK: Block = Block {
    identifier: ident!("bedrock"),
    properties: &PROPERTIES,
    default_state: &DEFAULT_STATE,
    states: &[DEFAULT_STATE],
};

pub const DEFAULT_STATE: BlockState = BlockState {
    id: BlockStateId(85),
};

pub const PROPERTIES: Properties = Properties::new()
    .with_hardness(-1.0)
    .with_explosion_resistance(3600000.0)
    .with_no_loot_table()
    .with_is_valid_spawn(false);
