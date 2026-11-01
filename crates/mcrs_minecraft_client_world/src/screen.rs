use bevy::prelude::*;
use mcrs_minecraft_game_mode::GameMode;

#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    #[default]
    None,
    Inventory,
    GameModeSwitcher(GameMode),
    Chat,
}
