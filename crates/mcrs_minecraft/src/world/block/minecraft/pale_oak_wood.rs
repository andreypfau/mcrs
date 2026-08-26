use crate::world::block::minecraft::note_block::NoteBlockInstrument;
use mcrs_minecraft_block::material::map::MapColor;
use crate::world::block::behaviour::Properties;
use crate::world::block::{Block, BlockState};
use mcrs_protocol::{BlockStateId, ident};

pub const BLOCK: Block = Block {
    identifier: ident!("pale_oak_wood"),
    protocol_id: 20,
    properties: &PROPERTIES,
    default_state: DEFAULT_STATE,
    states: &[X_STATE, Y_STATE, Z_STATE],
};

pub const X_STATE: BlockState = BlockState {
    id: BlockStateId(22),
};

pub const Y_STATE: BlockState = BlockState {
    id: BlockStateId(23),
};

pub const Z_STATE: BlockState = BlockState {
    id: BlockStateId(24),
};

pub const DEFAULT_STATE: &BlockState = &Y_STATE;

// Block type: RotatedPillarBlock - not fully implemented yet
pub const PROPERTIES: Properties = Properties::new();
