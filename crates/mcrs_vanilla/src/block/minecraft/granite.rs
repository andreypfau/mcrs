use crate::block::behaviour::Properties;
use crate::block::minecraft::note_block::NoteBlockInstrument;
use crate::block::{Block, BlockState};
use crate::material::map::MapColor;
use mcrs_protocol::{BlockStateId, ident};

pub const BLOCK: Block = Block {
    identifier: ident!("granite"),
    protocol_id: 2,
    properties: &PROPERTIES,
    default_state: &DEFAULT_STATE,
    states: &[DEFAULT_STATE],
};

pub const DEFAULT_STATE: BlockState = BlockState {
    id: BlockStateId(2),
};

pub const PROPERTIES: Properties = Properties::new()
    .with_map_color(MapColor::DIRT)
    .with_note_block_instrument(NoteBlockInstrument::BASEDRUM)
    .with_hardness(1.5)
    .with_explosion_resistance(6.0)
    .requires_correct_tool_for_drops();
