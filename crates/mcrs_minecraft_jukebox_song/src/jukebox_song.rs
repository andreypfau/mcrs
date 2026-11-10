use mcrs_minecraft_registries::{Refs, Resolved};
use mcrs_minecraft_sound::Sound;
use mcrs_minecraft_text::{Text, TextSerde, TextTypes};
use serde::{Deserialize, Serialize};

/// An entry of the `jukebox_song` registry. `I` is the item type of the text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(bound(
    serialize = "I: TextSerde, Sound<M>: Serialize",
    deserialize = "I: TextSerde, Sound<M>: Deserialize<'de>"
))]
pub struct JukeboxSong<I: TextTypes, M: Refs = Resolved> {
    pub sound_event: Sound<M>,
    pub description: Text<I>,
    pub length_in_seconds: f32,
    pub comparator_output: i32,
}
