#[cfg(feature = "registry")]
mod block_sound_set;
mod sound_event_value;
#[cfg(feature = "registry")]
#[rustfmt::skip]
pub mod block_sound_set_keys;
#[cfg(feature = "registry")]
#[rustfmt::skip]
pub mod block_sound_set_tag_keys;
#[cfg(feature = "registry")]
#[rustfmt::skip]
pub mod registry_keys;
#[cfg(feature = "registry")]
#[rustfmt::skip]
pub mod sound_event_keys;

#[cfg(feature = "registry")]
pub use block_sound_set::BlockSoundSet;
#[cfg(feature = "registry")]
pub use sound_event_value::Sound;
pub use sound_event_value::SoundEventValue;
