use crate::block::behaviour;
use crate::block::minecraft::note_block::NoteBlockInstrument;
use crate::block::Block;
use crate::material::map::MapColor;

define_block! {
    name: "polished_diorite",
    protocol_id: 5,
    base_state_id: 5,
    block_properties: behaviour::Properties::new()
        .with_map_color(MapColor::QUARTZ)
        .with_note_block_instrument(NoteBlockInstrument::Basedrum)
        .with_hardness(1.5)
        .with_explosion_resistance(6.0)
        .requires_correct_tool_for_drops()
}
