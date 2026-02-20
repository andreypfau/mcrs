use crate::block::behaviour::Properties;
use crate::block::minecraft::note_block::NoteBlockInstrument;
use crate::block::{Block, BlockState};
use crate::material::map::MapColor;
use mcrs_protocol::BlockStateId;

pub const BLOCK: Block = Block {
    identifier: mcrs_core::rl!("polished_diorite"),
    protocol_id: 5,
    properties: &PROPERTIES,
    default_state: &DEFAULT_STATE,
    states: &[DEFAULT_STATE],
};

pub const DEFAULT_STATE: BlockState = BlockState {
    id: BlockStateId(5),
};

pub const PROPERTIES: Properties = Properties::new()
    .with_map_color(MapColor::QUARTZ)
    .with_note_block_instrument(NoteBlockInstrument::BASEDRUM)
    .with_hardness(1.5)
    .with_explosion_resistance(6.0)
    .requires_correct_tool_for_drops();
