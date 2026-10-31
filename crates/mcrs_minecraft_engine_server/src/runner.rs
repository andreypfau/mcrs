use bevy_app::{App, AppExit};
use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use mcrs_minecraft_engine::between_ticks::run_between_ticks;
use mcrs_minecraft_engine::bridge::OutboundFlush;
use mcrs_minecraft_engine::dim::{DimSubAppHandle, drain_dim_despawn_queue, pump_channels};
use mcrs_minecraft_engine::sub_app_builder::drain_dim_spawn_queue;
use mcrs_minecraft_level::world::column_drain::ColumnDrain;
use mcrs_minecraft_level::world::sub_app::DimAppLabel;
use std::num::NonZeroU32;

pub const DEFAULT_TPS: NonZeroU32 = match NonZeroU32::new(20) {
    Some(tps) => tps,
    None => NonZeroU32::MIN,
};

pub fn run_server_loop(app: App) -> AppExit {
    mcrs_minecraft_level::server_loop::run_server_loop(
        app,
        DEFAULT_TPS,
        between_ticks,
        drain_columns,
    )
}

/// What the host does after each tick: it takes the dimensions' output, runs what plugins
/// added to `BetweenTicks`, moves the players that changed dimension and spawns or despawns dimensions.
pub fn between_ticks(app: &mut App) {
    pump_channels(app);
    run_between_ticks(app);
    mcrs_minecraft_engine::player_dimension_change::apply_dimension_change_requests(app);
    drain_dim_spawn_queue(app);
    drain_dim_despawn_queue(app);
}

/// Passes on what landed since the tick: every dimension sends the columns whose sections
/// are in, the host bridges them and writes the sockets.
fn drain_columns(app: &mut App) {
    let dims: Vec<Entity> = app
        .world_mut()
        .query_filtered::<Entity, With<DimSubAppHandle>>()
        .iter(app.world())
        .collect();
    for dim in dims {
        if let Some(sub_app) = app.get_sub_app_mut(DimAppLabel(dim)) {
            sub_app.world_mut().run_schedule(ColumnDrain);
        }
    }
    pump_channels(app);
    // Absent without `EnginePlugin`, which adds it.
    let _ = app.world_mut().try_run_schedule(OutboundFlush);
}
