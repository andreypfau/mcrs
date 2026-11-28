use bevy_ecs::resource::Resource;
use mcrs_minecraft_game_mode::GameMode;

#[derive(Resource, Clone, Copy, Debug)]
pub struct DefaultGameMode(pub GameMode);

impl Default for DefaultGameMode {
    fn default() -> Self {
        Self(GameMode::Creative)
    }
}
