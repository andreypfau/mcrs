use crate::ability::{PlayerFlying, PlayerGameMode, PlayerOpLevel};
use crate::game_mode::ListedGameMode;
use crate::op_level::AnnouncedOpLevel;
use crate::teleport::TeleportState;
use crate::tracked_by::TrackedBy;
use bevy_ecs::bundle::Bundle;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::entity::player::chunk_view::PlayerViewDistance;

#[derive(Bundle, Default)]
pub struct PlayerBundle {
    pub teleport_state: TeleportState,
    pub view_distance: PlayerViewDistance,
    pub game_mode: PlayerGameMode,
    pub flying: PlayerFlying,
    pub op_level: PlayerOpLevel,
    pub listed_game_mode: ListedGameMode,
    pub announced_op_level: AnnouncedOpLevel,
    pub tracked_by: TrackedBy,
    pub marker: Player,
}
