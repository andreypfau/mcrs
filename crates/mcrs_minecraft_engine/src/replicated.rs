use bevy_ecs::component::Component;
use mcrs_engine_muxer::{EntityId, TileKey};
use mcrs_minecraft_game_mode::GameMode;

/// On a player that its dimension publishes: its id in the tile deltas, its tile and the game
/// mode that its profile entry shows, as published.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Replicated {
    pub id: EntityId,
    pub tile: TileKey,
    pub game_mode: GameMode,
}
