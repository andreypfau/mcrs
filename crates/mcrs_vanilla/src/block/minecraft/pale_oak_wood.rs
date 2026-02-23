use crate::block::behaviour;
use crate::block::minecraft::note_block::NoteBlockInstrument;
use crate::block::state_properties;
use crate::block::Block;
use crate::material::map::MapColor;

// Block type: RotatedPillarBlock - not fully implemented yet
define_block! {
    name: "pale_oak_wood",
    protocol_id: 20,
    base_state_id: 22,
    properties: [&state_properties::AXIS],
    default: { axis: y },
    block_properties: behaviour::Properties::new()
        .with_map_color(MapColor::STONE)
        .with_note_block_instrument(NoteBlockInstrument::Bass)
        .with_strength(2.0)
        .ignited_by_lava()
}
