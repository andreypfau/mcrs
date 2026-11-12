use crate::ability::{PlayerFlying, PlayerGameMode, PlayerOpLevel};
use crate::client_on_ground::ClientOnGround;
use crate::game_mode::ListedGameMode;
use crate::op_level::AnnouncedOpLevel;
use crate::teleport::TeleportState;
use bevy_ecs::bundle::Bundle;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::entity::player::chunk_view::PlayerViewDistance;

#[derive(Bundle, Default)]
pub struct PlayerBundle {
    pub teleport_state: TeleportState,
    pub view_distance: PlayerViewDistance,
    pub game_mode: PlayerGameMode,
    pub flying: PlayerFlying,
    pub on_ground: ClientOnGround,
    pub op_level: PlayerOpLevel,
    pub listed_game_mode: ListedGameMode,
    pub announced_op_level: AnnouncedOpLevel,
    pub marker: Player,
}
