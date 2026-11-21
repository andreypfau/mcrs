use bytes::Bytes;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_protocol_game::{Hand, SwingAnimationKind};

use crate::attribute_state::AttributeState;
use crate::delta_base::Pose;
use crate::player_entries::PlayerEntries;
use crate::player_spawn::PlayerSpawn;

#[derive(Clone, Debug, PartialEq)]
pub enum PlayerData {
    Spawn(PlayerSpawn),
    Pose(Pose),
    GameMode(GameMode),
    Joined(Bytes),
    Left(Bytes),
    Attribute(AttributeState),
    Entries(PlayerEntries),
    Swing {
        hand: Hand,
        animation: SwingAnimationKind,
        duration: i32,
    },
}
