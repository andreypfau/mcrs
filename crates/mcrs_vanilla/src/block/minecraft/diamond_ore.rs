use crate::block::behaviour;
use crate::block::minecraft::note_block::NoteBlockInstrument;
use crate::block::Block;
use crate::material::map::MapColor;

define_block! {
    name: "diamond_ore",
    protocol_id: 202,
    base_state_id: 5106,
    block_properties: behaviour::Properties::new()
        .with_map_color(MapColor::STONE)
        .with_note_block_instrument(NoteBlockInstrument::Basedrum)
        .with_hardness(3.0)
        .with_explosion_resistance(3.0)
        .requires_correct_tool_for_drops()
        .with_xp_range(3, 7)
}
