use crate::world::block::behaviour::Properties;
use crate::world::block::{Block, BlockState};
use crate::world::material::map::MapColor;
use mcrs_protocol::{BlockStateId, ident};

pub const BLOCK: Block = Block {
    identifier: ident!("grass_block"),
    properties: &PROPERTIES,
    default_state: &DEFAULT_STATE,
    states: &[SNOWY_STATE, DEFAULT_STATE],
};

pub const SNOWY_STATE: BlockState = BlockState {
    id: BlockStateId(8),
};

pub const DEFAULT_STATE: BlockState = BlockState {
    id: BlockStateId(9),
};

// Block type: GrassBlock - not fully implemented yet
pub const PROPERTIES: Properties = Properties::new()
    .with_map_color(MapColor::GRASS)
    .with_strength(0.6)
    .with_random_ticks();
