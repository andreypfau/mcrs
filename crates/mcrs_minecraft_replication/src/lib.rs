//! The Minecraft side of entity replication: the codec of the muxer for players and mobs, and the pure
//! functions and plain state under it that turn the state of a subject into packets. Each packet
//! is a record: its length, then its id and its body, not framed.

mod delivery;
mod delta_base;
mod encode;
mod entity_codec;
mod hold_set;
mod list_packets;
mod mob_bundle;
mod mob_report;
mod mob_signal;
mod mob_spawn;
mod mob_state;
mod mob_tracker;
mod mob_trackers;
mod motion;
mod move_choice;
mod nearest;
mod player_data;
mod player_input;
mod player_list;
mod player_spawn;
mod replication_data;
mod wire_id;

pub use delta_base::Pose;
pub use encode::record;
pub use entity_codec::EntityCodec;
pub use mob_report::MobReport;
pub use mob_signal::MobSignal;
pub use mob_spawn::MobSpawn;
pub use mob_state::MobState;
pub use player_data::PlayerData;
pub use player_input::{Backlog, PlayerInput};
pub use player_list::{LIST_INTERVAL_TICKS, LISTED_CAP};
pub use player_spawn::PlayerSpawn;
pub use replication_data::ReplicationData;
pub use wire_id::wire_id;
