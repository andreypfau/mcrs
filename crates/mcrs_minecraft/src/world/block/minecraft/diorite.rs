use crate::world::block::behaviour::Properties;
use crate::world::block::minecraft::note_block::NoteBlockInstrument;
use crate::world::block::{Block, BlockState};
use crate::world::material::map::MapColor;
use mcrs_protocol::{BlockStateId, ident};

pub const BLOCK: Block = Block {
    identifier: ident!("diorite"),
    protocol_id: 4,
    properties: &PROPERTIES,
    default_state: &DEFAULT_STATE,
    states: &[DEFAULT_STATE],
};

pub const DEFAULT_STATE: BlockState = BlockState {
    id: BlockStateId(4),
};

pub const PROPERTIES: Properties = Properties::new()
    .with_map_color(MapColor::QUARTZ)
    .with_note_block_instrument(NoteBlockInstrument::BASEDRUM)
    .with_hardness(1.5)
    .with_explosion_resistance(6.0)
    .requires_correct_tool_for_drops();
