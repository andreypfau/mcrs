//! The Minecraft side of entity replication: the codec of the muxer for players and mobs, and the pure
//! functions and plain state under it that turn the state of a subject into packets. Each packet
//! is a record: its length, then its id and its body, not framed.

mod delta_base;
mod encode;
mod list_packets;
mod mob_bundle;
mod mob_looks;
mod mob_signal;
mod mob_spawn;
mod mob_state;
mod mob_tracker;
mod mob_trackers;
mod motion;
mod nearest;
mod player_codec;
mod player_data;
mod player_input;
mod player_list;
mod player_spawn;

pub use delta_base::Pose;
pub use encode::record;
pub use mob_looks::{MobLooks, MobVariant};
pub use mob_signal::MobSignal;
pub use mob_spawn::MobSpawn;
pub use mob_state::MobState;
pub use player_codec::{PlayerCodec, wire_id};
pub use player_data::PlayerData;
pub use player_input::PlayerInput;
pub use player_list::{LIST_INTERVAL_TICKS, LISTED_CAP};
pub use player_spawn::PlayerSpawn;
