use bevy_ecs::resource::Resource;
use mcrs_minecraft_game_mode::GameMode;

/// Game mode given to players joining for the first time.
#[derive(Resource, Clone, Copy, Debug)]
pub struct DefaultGameMode(pub GameMode);

impl Default for DefaultGameMode {
    fn default() -> Self {
        Self(GameMode::Creative)
    }
}
