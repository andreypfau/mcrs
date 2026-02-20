use crate::block::behaviour::Properties;
use crate::block::{Block, BlockState};
use crate::material::map::MapColor;
use mcrs_protocol::BlockStateId;

pub const BLOCK: Block = Block {
    identifier: mcrs_core::rl!("coarse_dirt"),
    protocol_id: 10,
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
