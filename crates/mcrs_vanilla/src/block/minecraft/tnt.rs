use crate::block::behaviour::Properties;
use crate::block::{Block, BlockState};
use crate::material::map::MapColor;
use mcrs_protocol::BlockStateId;

pub const BLOCK: Block = Block {
    identifier: mcrs_core::rl!("tnt"),
    protocol_id: 176,
    properties: &PROPERTIES,
    default_state: &DEFAULT_STATE,
    states: &[UNSTABLE_STATE, DEFAULT_STATE],
};

pub const UNSTABLE_STATE: BlockState = BlockState {
    id: BlockStateId(2140),
};

pub const DEFAULT_STATE: BlockState = BlockState {
    id: BlockStateId(2141),
};

pub const PROPERTIES: Properties = Properties::new()
    .with_map_color(MapColor::FIRE)
    .with_strength(0.0)
    .ignited_by_lava()
    .instant_break();
