use crate::block::behaviour;
use crate::block::minecraft::note_block::NoteBlockInstrument;
use crate::block::Block;
use crate::material::map::MapColor;

// .sound(SoundType.BAMBOO_WOOD) - not implemented yet
define_block! {
    name: "bamboo_mosaic",
    protocol_id: 24,
    base_state_id: 28,
    block_properties: behaviour::Properties::new()
        .with_map_color(MapColor::COLOR_YELLOW)
        .with_note_block_instrument(NoteBlockInstrument::Bass)
        .with_hardness(2.0)
        .with_explosion_resistance(3.0)
        .ignited_by_lava()
}
