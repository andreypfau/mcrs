use bevy_app::{App, AppExit, SubApp};
use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;
use mcrs_minecraft_engine::between_ticks::run_between_ticks;
use mcrs_minecraft_engine::bridge::OutboundFlush;
use mcrs_minecraft_engine::dim::{drain_dim_despawn_queue, pump_channels, pump_host_channels};
use mcrs_minecraft_engine::sub_app_builder::drain_dim_spawn_queue;
use mcrs_minecraft_engine_world::world::column_drain::{ColumnDrain, DrainWakes};
use mcrs_minecraft_engine_world::world::sub_app::DimAppLabel;
use std::num::NonZeroU32;
use std::time::{Duration, Instant};

pub const DEFAULT_TPS: NonZeroU32 = match NonZeroU32::new(20) {
    Some(tps) => tps,
    None => NonZeroU32::MIN,
};

pub fn run_server_loop(app: App) -> AppExit {
    mcrs_minecraft_engine_world::server_loop::run_server_loop(
        app,
        DEFAULT_TPS,
        between_ticks,
        drain_until,
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

/// The least time between two drains. Under load the generation finishes work every few
/// microseconds, and a drain for each finish costs more in running the drain than it moves.
pub const DRAIN_INTERVAL: Duration = Duration::from_millis(5);

/// Passes on, up to `deadline`, what the dimensions finish off the tick: a dimension that asks
/// for a drain has one within [`DRAIN_INTERVAL`], and the host hands what it sent to the
/// sockets. A server with nothing finishing does not drain at all.
pub fn drain_until(app: &mut App, deadline: Instant) {
    loop {
        let drained = Instant::now();
        drain_woken(app);
        if !wait_for_wake(app, deadline) {
            return;
        }
        sleep_until((drained + DRAIN_INTERVAL).min(deadline));
    }
}

pub fn sleep_until(at: Instant) {
    if let Some(rest) = at.checked_duration_since(Instant::now()) {
        std::thread::sleep(rest);
    }
}

/// The labels of the dimensions that asked for a drain and have not had it.
#[derive(Resource, Default)]
struct WokenDimensions(Vec<Entity>);

/// Drains the dimensions that asked since the last drain, then hands what they sent to the
/// sockets.
pub fn drain_woken(app: &mut App) {
    let mut woken = app
        .world_mut()
        .remove_resource::<WokenDimensions>()
        .unwrap_or_default();
    if let Some(wakes) = app.world().get_resource::<DrainWakes>() {
        woken.0.extend(wakes.receiver().try_iter());
    }
    woken.0.sort_unstable();
    woken.0.dedup();
    if !woken.0.is_empty() {
        for &dim in &woken.0 {
            if let Some(sub_app) = app.get_sub_app_mut(DimAppLabel(dim)) {
                drain_dimension_columns(sub_app);
            }
        }
    }
    woken.0.clear();
    app.world_mut().insert_resource(woken);
    flush_host(app.world_mut());
}

/// Waits until a dimension asks for a drain or `deadline` passes, and tells whether one asked.
pub fn wait_for_wake(app: &mut App, deadline: Instant) -> bool {
    let Some(wakes) = app.world().get_resource::<DrainWakes>() else {
        return false;
    };
    let Ok(dim) = wakes.receiver().recv_deadline(deadline) else {
        return false;
    };
    app.world_mut()
        .get_resource_or_init::<WokenDimensions>()
        .0
        .push(dim);
    true
}

/// Takes what the dimensions sent and writes it to the sockets.
pub fn flush_host(host: &mut World) {
    pump_host_channels(host);
    // Absent without `EnginePlugin`, which adds it.
    let _ = host.try_run_schedule(OutboundFlush);
}

fn drain_dimension_columns(dimension: &mut SubApp) {
    if let Err(error) = dimension.world_mut().try_run_schedule(ColumnDrain) {
        tracing::error!(%error, "a dimension has no column drain");
    }
}
