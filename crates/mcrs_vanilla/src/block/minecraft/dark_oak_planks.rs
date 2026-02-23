use crate::block::behaviour;
use crate::block::minecraft::note_block::NoteBlockInstrument;
use crate::block::Block;
use crate::material::map::MapColor;

define_block! {
    name: "dark_oak_planks",
    protocol_id: 19,
    base_state_id: 21,
    block_properties: behaviour::Properties::new()
        .with_map_color(MapColor::COLOR_BROWN)
        .with_note_block_instrument(NoteBlockInstrument::Bass)
        .with_hardness(2.0)
        .with_explosion_resistance(3.0)
        .ignited_by_lava()
}
