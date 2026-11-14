use bytes::Bytes;
use mcrs_minecraft_game_mode::GameMode;

use crate::attribute_state::AttributeState;
use crate::delta_base::Pose;
use crate::player_spawn::PlayerSpawn;

/// What a world publishes about a player in a tile delta. A spawn carries
/// [`PlayerData::Spawn`], a move [`PlayerData::Pose`], and a data event any other variant.
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
    /// The instance of a syncable attribute of the player after a change (ATR-NET-003). It goes
    /// to every viewer that holds the player at the end of the tick, not to the player.
    Attribute(AttributeState),
}
