use crate::world::block::behaviour::Properties;
use crate::world::block::minecraft::note_block::NoteBlockInstrument;
use crate::world::block::{Block, BlockState};
use crate::world::material::map::MapColor;
use mcrs_protocol::{BlockStateId, ident};

pub const BLOCK: Block = Block {
    identifier: ident!("pale_oak_wood"),
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
pub const PROPERTIES: Properties = Properties::new()
    .with_map_color(MapColor::STONE)
    .with_note_block_instrument(NoteBlockInstrument::BASS)
    .with_strength(2.0)
    .ignited_by_lava();
