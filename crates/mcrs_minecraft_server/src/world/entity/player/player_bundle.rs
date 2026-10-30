use crate::world::aoi::TrackedBy;
use crate::world::entity::player::game_mode::ListedGameMode;
use crate::world::entity::player::op_level::AnnouncedOpLevel;
use bevy_ecs::bundle::Bundle;
use mcrs_minecraft_engine::ability::{PlayerFlying, PlayerGameMode, PlayerOpLevel};
use mcrs_minecraft_engine::teleport::TeleportState;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::entity::player::chunk_view::PlayerViewDistance;

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
