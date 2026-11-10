use mcrs_minecraft_core::codec::is_default;
use mcrs_minecraft_registries::{Refs, Resolved};
use mcrs_minecraft_sound::Sound;
use mcrs_minecraft_text::{Text, TextSerde, TextTypes};
use serde::{Deserialize, Serialize};

/// An entry of the `instrument` registry. `I` is the item type of the text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(bound(
    serialize = "I: TextSerde, Sound<M>: Serialize",
    deserialize = "I: TextSerde, Sound<M>: Deserialize<'de>"
))]
pub struct Instrument<I: TextTypes, M: Refs = Resolved> {
    pub sound_event: Sound<M>,
    pub use_duration: f32,
    pub range: f32,
    pub description: Text<I>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub durability_damage: i32,
}
