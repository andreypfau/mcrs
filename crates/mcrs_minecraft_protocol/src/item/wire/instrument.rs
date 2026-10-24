use mcrs_minecraft_registries::Wire;
use std::io::Write;

use mcrs_minecraft_core::codec;

use crate::item::component::instrument::*;
use crate::item::wire::newtype_wire;
use crate::{Decode, Encode, VarInt};

newtype_wire!(Instrument<Wire>, JukeboxPlayable<Wire>);

impl Encode for InstrumentValue<Wire> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.sound_event.encode(&mut w)?;
        self.use_duration.encode(&mut w)?;
        self.range.encode(&mut w)?;
        VarInt(self.durability_damage.0).encode(&mut w)?;
        self.description.encode(w)
    }
}

impl Decode<'_> for InstrumentValue<Wire> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(InstrumentValue {
            sound_event: Decode::decode(r)?,
            use_duration: f32::decode(r)?,
            range: f32::decode(r)?,
            durability_damage: codec::Bounded(VarInt::decode(r)?.0),
            description: Decode::decode(r)?,
        })
    }
}

impl Encode for JukeboxSong<Wire> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.sound_event.encode(&mut w)?;
        self.description.encode(&mut w)?;
        self.length_in_seconds.encode(&mut w)?;
        VarInt(self.comparator_output.0).encode(w)
    }
}

impl Decode<'_> for JukeboxSong<Wire> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(JukeboxSong {
            sound_event: Decode::decode(r)?,
            description: Decode::decode(r)?,
            length_in_seconds: f32::decode(r)?,
            comparator_output: codec::Bounded(VarInt::decode(r)?.0),
        })
    }
}
