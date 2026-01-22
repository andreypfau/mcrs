use crate::world::block::behaviour::Properties;
use crate::world::block::{Block, BlockState};
use crate::world::material::map::MapColor;
use mcrs_protocol::{BlockStateId, ident};

pub const BLOCK: Block = Block {
    identifier: ident!("coarse_dirt"),
    properties: &PROPERTIES,
    default_state: &DEFAULT_STATE,
    states: &[DEFAULT_STATE],
};

pub const DEFAULT_STATE: BlockState = BlockState {
    id: BlockStateId(11),
};

pub const PROPERTIES: Properties = Properties::new()
    .with_map_color(MapColor::DIRT)
    .with_strength(0.5);
