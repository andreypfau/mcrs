use crate::block::behaviour;
use crate::block::state_properties;
use crate::block::Block;
use crate::material::map::MapColor;
use crate::sound::SoundType;
use mcrs_core::block_state::{Property, PropertyValue};

define_block! {
    name: "note_block",
    protocol_id: 109,
    base_state_id: 581,
    properties: [&state_properties::INSTRUMENT, &state_properties::NOTE, &state_properties::POWERED],
    default: { instrument: harp, note: 0, powered: false },
    block_properties: behaviour::Properties::new()
        .with_map_color(MapColor::WOOD)
        .with_note_block_instrument(NoteBlockInstrument::Bass)
        .with_sound(&SoundType::WOOD)
        .with_strength(0.8)
        .ignited_by_lava()
}

pub static INSTRUMENT_PROP: Property<NoteBlockInstrument> =
    Property::new(&state_properties::INSTRUMENT);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum NoteBlockInstrument {
    Harp = 0,
    Basedrum = 1,
    Snare = 2,
    Hat = 3,
    Bass = 4,
    Flute = 5,
    Bell = 6,
    Guitar = 7,
    Chime = 8,
    Xylophone = 9,
    IronXylophone = 10,
    CowBell = 11,
    Didgeridoo = 12,
    Bit = 13,
    Banjo = 14,
    Pling = 15,
    Zombie = 16,
    Skeleton = 17,
    Creeper = 18,
    Dragon = 19,
    WitherSkeleton = 20,
    Piglin = 21,
    CustomHead = 22,
}

impl PropertyValue for NoteBlockInstrument {
    fn from_index(index: u8) -> Option<Self> {
        if index <= 22 {
            // SAFETY: repr(u8) and all values 0..=22 are valid variants.
            Some(unsafe { std::mem::transmute(index) })
        } else {
            None
        }
    }
    fn to_index(self) -> u8 {
        self as u8
    }
}
