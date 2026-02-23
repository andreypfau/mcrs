use crate::block::behaviour;
use crate::block::minecraft::note_block::NoteBlockInstrument;
use crate::block::Block;
use crate::material::map::MapColor;

define_block! {
    name: "mangrove_planks",
    protocol_id: 22,
    base_state_id: 26,
    block_properties: behaviour::Properties::new()
        .with_map_color(MapColor::COLOR_RED)
        .with_note_block_instrument(NoteBlockInstrument::Bass)
        .with_hardness(2.0)
        .with_explosion_resistance(3.0)
        .ignited_by_lava()
}
