use bevy::prelude::*;
use mcrs_minecraft_client_entity::ServerTicking;
use mcrs_minecraft_environment::world_clock::WorldClockSystems;

/// Stops the world clocks while the server freezes its ticks, as `/tick freeze` stops the day.
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
