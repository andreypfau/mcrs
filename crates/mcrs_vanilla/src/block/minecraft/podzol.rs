use crate::block::behaviour::Properties;
use crate::block::{Block, BlockState};
use crate::material::map::MapColor;
use mcrs_protocol::BlockStateId;

pub const BLOCK: Block = Block {
    identifier: mcrs_core::rl!("podzol"),
    protocol_id: 11,
    properties: &PROPERTIES,
    default_state: &DEFAULT_STATE,
    states: &[SNOWY_STATE, DEFAULT_STATE],
};

pub const SNOWY_STATE: BlockState = BlockState {
    id: BlockStateId(12),
};

pub const DEFAULT_STATE: BlockState = BlockState {
    id: BlockStateId(13),
};

// Block type: SnowyDirtBlock - not fully implemented yet
pub const PROPERTIES: Properties = Properties::new()
    .with_map_color(MapColor::PODZOL)
    .with_strength(0.5);
