use crate::world::block::minecraft::note_block::NoteBlockInstrument;
use mcrs_minecraft_block::material::map::MapColor;
use crate::world::block::behaviour::Properties;
use crate::world::block::{Block, BlockState};
use mcrs_protocol::{BlockStateId, ident};

pub const BLOCK: Block = Block {
    identifier: ident!("bamboo_planks"),
    protocol_id: 23,
    properties: &PROPERTIES,
    default_state: &DEFAULT_STATE,
    states: &[DEFAULT_STATE],
};

pub const DEFAULT_STATE: BlockState = BlockState {
    id: BlockStateId(27),
};

// .sound(SoundType.BAMBOO_WOOD) - not implemented yet
pub const PROPERTIES: Properties = Properties::new();
