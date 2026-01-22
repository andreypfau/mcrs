use crate::world::block::behaviour::Properties;
use crate::world::block::minecraft::note_block::NoteBlockInstrument;
use crate::world::block::{Block, BlockState};
use crate::world::material::map::MapColor;
use mcrs_protocol::{BlockStateId, ident};

pub const BLOCK: Block = Block {
    identifier: ident!("dark_oak_planks"),
    properties: &PROPERTIES,
    default_state: &DEFAULT_STATE,
    states: &[DEFAULT_STATE],
};

pub const DEFAULT_STATE: BlockState = BlockState {
    id: BlockStateId(21),
};

pub const PROPERTIES: Properties = Properties::new()
    .with_map_color(MapColor::COLOR_BROWN)
    .with_note_block_instrument(NoteBlockInstrument::BASS)
    .with_hardness(2.0)
    .with_explosion_resistance(3.0)
    .ignited_by_lava();
