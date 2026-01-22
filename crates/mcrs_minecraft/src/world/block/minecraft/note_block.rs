use crate::generate_block_states;
use crate::sound::SoundType;
use crate::world::block::behaviour::Properties;
use crate::world::block::{Block, BlockState};
use crate::world::material::map::MapColor;
use mcrs_protocol::{BlockStateId, ident};

generate_block_states! {
    base_id: 581,
    block_name: "note_block",
    // TODO: try different approach; IDE and compiler blow up with 100% CPU usage
    // state_properties: {
    //     instrument: [harp, basedrum, snare, hat, bass, flute, bell, guitar, chime, xylophone,
    //         iron_xylophone, cow_bell, didgeridoo, bit, banjo, pling,
    //         zombie, skeleton, creeper, dragon, wither_skeleton, piglin, custom_head ],
    //     note: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
    //         13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24],
    //     powered: [true, false],
    // },
    // default: { instrument:harp, note:0, powered:false },
    block_properties: Properties::new()
        .with_map_color(MapColor::WOOD)
        .with_note_block_instrument(NoteBlockInstrument::BASS)
        .with_sound(&SoundType::WOOD)
        .with_strength(0.8)
        .ignited_by_lava()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum NoteBlockInstrument {
    HARP,
    BASEDRUM,
    SNARE,
    HAT,
    BASS,
    FLUTE,
    BELL,
    GUITAR,
    CHIME,
    XYLOPHONE,
    IRON_XYLOPHONE,
    COW_BELL,
    DIDGERIDOO,
    BIT,
    BANJO,
    PLING,
    ZOMBIE,
    SKELETON,
    CREEPER,
    DRAGON,
    WITHER_SKELETON,
    PIGLIN,
    CUSTOM_HEAD,
}
