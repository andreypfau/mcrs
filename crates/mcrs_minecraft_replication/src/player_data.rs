use crate::delta_base::Pose;
use crate::mob_signal::MobSignal;
use crate::mob_spawn::MobSpawn;
use crate::mob_state::MobState;
use crate::player_spawn::PlayerSpawn;
use bytes::Bytes;
use mcrs_minecraft_game_mode::GameMode;

/// What a world publishes about a player or a mob in a tile delta. A spawn carries
/// [`PlayerData::Spawn`] or [`PlayerData::MobSpawn`], a move [`PlayerData::Pose`] or
/// [`PlayerData::MobMove`], and a data event any other variant. One muxer link has one codec, so
/// the two kinds of subject share this type. The codec ignores a variant on an event that does
/// not carry it.
#[derive(Clone, Debug, PartialEq)]
pub enum PlayerData {
    Spawn(PlayerSpawn),
    Pose(Pose),
    /// The game mode that the profile entry of the player shows.
    GameMode(GameMode),
    /// A message that the player joined, as a record. It goes to the player and to every viewer
    /// that holds the player at the end of the tick.
    Joined(Bytes),
    /// A message that the player left, as a record. It goes to every viewer that holds the
    /// player at that event.
    Left(Bytes),
    MobSpawn(MobSpawn),
    /// The state of a mob at the end of a tick in which it changed.
    MobMove(MobState),
    MobSignal(MobSignal),
}
