use bevy_ecs::component::Component;
use mcrs_engine_muxer::{EntityId, TileKey};
use mcrs_minecraft_game_mode::GameMode;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Replicated {
    pub id: EntityId,
    pub tile: TileKey,
    pub game_mode: GameMode,
    pub visibility: Visibility,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    Shown,
    Hidden,
}
