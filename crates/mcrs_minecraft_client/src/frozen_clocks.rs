use bevy::prelude::*;
use mcrs_minecraft_entity_visual::ServerTicking;
use mcrs_minecraft_environment::WorldClockSystems;

pub struct FrozenClocksPlugin;

impl Plugin for FrozenClocksPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            FixedUpdate,
            WorldClockSystems.run_if(|ticking: Option<Res<ServerTicking>>| {
                ticking.is_none_or(|ticking| ticking.runs())
            }),
        );
    }
}
