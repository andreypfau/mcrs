//! The Minecraft side of entity replication: the codec of the muxer for players, and the pure
//! functions and plain state under it that turn the state of a subject into packets. Each packet
//! is a record: its length, then its id and its body, not framed.

mod delta_base;
mod encode;
mod list_packets;
mod list_policy;
mod player_codec;
mod player_data;
mod player_input;
mod player_list;
mod player_spawn;

pub use delta_base::{KEYFRAME_INTERVAL_TICKS, Pose};
pub use encode::record;
pub use list_packets::{Profile, game_mode_packet};
pub use list_policy::*;
pub use player_codec::*;
pub use player_data::*;
pub use player_input::*;
pub use player_list::*;
pub use player_spawn::*;
