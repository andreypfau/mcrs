use crate::world::block::behaviour::Properties;
use crate::world::block::{Block, BlockState};
use crate::world::material::map::MapColor;
use mcrs_protocol::{BlockStateId, ident};

pub const BLOCK: Block = Block {
    identifier: ident!("podzol"),
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
