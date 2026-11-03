use bevy_app::{App, AppExit, SubApp};
use mcrs_minecraft_engine::between_ticks::run_between_ticks;
use mcrs_minecraft_engine::bridge::OutboundFlush;
use mcrs_minecraft_engine::dim::{drain_dim_despawn_queue, pump_channels};
use mcrs_minecraft_engine::sub_app_builder::drain_dim_spawn_queue;
use mcrs_minecraft_engine_world::tick_threading;
use mcrs_minecraft_engine_world::world::column_drain::ColumnDrain;
use std::num::NonZeroU32;

pub const DEFAULT_TPS: NonZeroU32 = match NonZeroU32::new(20) {
    Some(tps) => tps,
    None => NonZeroU32::MIN,
};

pub fn run_server_loop(app: App) -> AppExit {
    mcrs_minecraft_engine_world::server_loop::run_server_loop(
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
pub fn drain_columns(app: &mut App) {
    tick_threading::for_each_sub_app(app, drain_dimension_columns);
    pump_channels(app);
    // Absent without `EnginePlugin`, which adds it.
    let _ = app.world_mut().try_run_schedule(OutboundFlush);
}

// Every sub-app of the server is a dimension world, and each has `ColumnDrain`.
fn drain_dimension_columns(dimension: &mut SubApp) {
    let _ = dimension.world_mut().try_run_schedule(ColumnDrain);
}
