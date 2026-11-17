use anyhow::Context;
use mcrs_minecraft_protocol::packets::table::row_of;
use mcrs_minecraft_protocol::{Decode, VarInt};
use mcrs_minecraft_protocol_game::table::game_clientbound;
use mcrs_minecraft_sound::sound_event_keys::SoundEvent;
use serde::{Deserialize, Serialize};

const ROW: i32 = match row_of(game_clientbound::NAMES, "sound") {
    Some(row) => row,
    None => -1,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SoundPlayed {
    pub client: String,
    pub sound: SoundEvent,
    pub source: i32,
    pub position: [i32; 3],
    pub volume: f32,
    pub pitch: f32,
    pub seed: i64,
}

impl SoundPlayed {
    pub(crate) fn is_packet(id: i32) -> bool {
        id == ROW
    }

    pub(crate) fn read(client: &str, mut body: &[u8]) -> anyhow::Result<Self> {
        let reader = &mut body;
        let event = VarInt::decode(reader)?.0;
        let id = event
            .checked_sub(1)
            .and_then(|id| u16::try_from(id).ok())
            .context("the sound packet has an inline sound event")?;
        let sound = SoundEvent::from_protocol_id(id)
            .with_context(|| format!("the sound event {id} is not in the registry"))?;
        let source = VarInt::decode(reader)?.0;
        let position = [
            i32::decode(reader)?,
            i32::decode(reader)?,
            i32::decode(reader)?,
        ];
        let volume = f32::decode(reader)?;
        let pitch = f32::decode(reader)?;
        let seed = i64::decode(reader)?;
        anyhow::ensure!(reader.is_empty(), "the sound packet has trailing bytes");
        Ok(Self {
            client: client.to_owned(),
            sound,
            source,
            position,
            volume,
            pitch,
            seed,
        })
    }
}
