use crate::block::behaviour;
use crate::block::minecraft::note_block::NoteBlockInstrument;
use crate::block::Block;
use crate::material::map::MapColor;

define_block! {
    name: "bedrock",
    protocol_id: 34,
    base_state_id: 85,
    block_properties: behaviour::Properties::new()
        .with_map_color(MapColor::STONE)
        .with_note_block_instrument(NoteBlockInstrument::Basedrum)
        .with_hardness(-1.0)
        .with_explosion_resistance(3600000.0)
        .with_no_loot_table()
}
