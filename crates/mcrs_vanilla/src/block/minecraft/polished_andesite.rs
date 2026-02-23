use crate::block::behaviour;
use crate::block::minecraft::note_block::NoteBlockInstrument;
use crate::block::Block;
use crate::material::map::MapColor;

define_block! {
    name: "polished_andesite",
    protocol_id: 7,
    base_state_id: 7,
    block_properties: behaviour::Properties::new()
        .with_map_color(MapColor::STONE)
        .with_note_block_instrument(NoteBlockInstrument::Basedrum)
        .with_hardness(1.5)
        .with_explosion_resistance(6.0)
        .requires_correct_tool_for_drops()
}
