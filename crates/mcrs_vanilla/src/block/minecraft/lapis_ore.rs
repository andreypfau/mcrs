use crate::block::behaviour;
use crate::block::minecraft::note_block::NoteBlockInstrument;
use crate::block::Block;
use crate::material::map::MapColor;

define_block! {
    name: "lapis_ore",
    protocol_id: 102,
    base_state_id: 563,
    block_properties: behaviour::Properties::new()
        .with_map_color(MapColor::LAPIS)
        .with_note_block_instrument(NoteBlockInstrument::Basedrum)
        .with_hardness(3.0)
        .with_explosion_resistance(3.0)
        .requires_correct_tool_for_drops()
        .with_xp_range(2, 5)
}
