use bytes::Bytes;
use mcrs_minecraft_game_mode::GameMode;

use crate::attribute_state::AttributeState;
use crate::delta_base::Pose;
use crate::player_spawn::PlayerSpawn;

#[derive(Clone, Debug, PartialEq)]
pub enum PlayerData {
    Spawn(PlayerSpawn),
    Pose(Pose),
    GameMode(GameMode),
    Joined(Bytes),
    Left(Bytes),
    Attribute(AttributeState),
}
