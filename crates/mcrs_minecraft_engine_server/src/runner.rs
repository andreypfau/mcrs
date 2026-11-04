use bevy_app::{App, AppExit, SubApp};
use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use mcrs_minecraft_engine::between_ticks::run_between_ticks;
use mcrs_minecraft_engine::bridge::run_pending_flush;
use mcrs_minecraft_engine::dim::{drain_dim_despawn_queue, pump_channels, pump_dim};
use mcrs_minecraft_engine::sub_app_builder::drain_dim_spawn_queue;
use mcrs_minecraft_engine_world::world::outbox_drain::{DrainWakes, OutboxDrain};
use mcrs_minecraft_engine_world::world::sub_app::DimAppLabel;
use std::num::NonZeroU32;
use std::time::Instant;

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

/// Passes on, up to `deadline`, what the tick wrote for the clients. The host hands it to the
/// sockets at once. A dimension whose full channel held packets back asks for a drain, and has
/// it as soon as the host has emptied the channel. Nothing here changes what a tick reads:
/// generation and light land only at the start of a tick.
pub fn drain_until(app: &mut App, deadline: Instant) {
    loop {
        drain_woken(app);
        if !wait_for_wake(app, deadline) {
            return;
        }
    }
}

/// The labels of the dimensions that asked for a drain and have not had it.
#[derive(Resource, Default)]
struct WokenDimensions(Vec<Entity>);

/// What one [`drain_woken`] took in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Drained {
    /// The wakes that arrived since the last drain. A dimension can wake more than once
    /// before it drains, and it drains once for all of them.
    pub wakes: usize,
    pub dimensions: usize,
}

/// Drains the outboxes of the dimensions that asked since the last drain, then hands what they
/// sent to the sockets.
pub fn drain_woken(app: &mut App) -> Drained {
    let mut woken = app
        .world_mut()
        .remove_resource::<WokenDimensions>()
        .unwrap_or_default();
    if let Some(wakes) = app.world().get_resource::<DrainWakes>() {
        woken.0.extend(wakes.receiver().try_iter());
    }
    let wakes = woken.0.len();
    woken.0.sort_unstable();
    woken.0.dedup();
    let dimensions = woken.0.len();
    for &dim in &woken.0 {
        if let Some(sub_app) = app.get_sub_app_mut(DimAppLabel(dim)) {
            drain_outbox(sub_app);
        }
    }
    let host = app.world_mut();
    for &dim in &woken.0 {
        pump_dim(host, dim);
    }
    // Also after a drain that woke nobody: what the tick left in the host goes out with it.
    run_pending_flush(host);
    woken.0.clear();
    app.world_mut().insert_resource(woken);
    Drained { wakes, dimensions }
}

/// Waits until a dimension asks for a drain or `deadline` passes, and tells whether one asked.
pub fn wait_for_wake(app: &mut App, deadline: Instant) -> bool {
    let wakes = app.world_mut().get_resource_or_init::<DrainWakes>();
    let Ok(dim) = wakes.receiver().recv_deadline(deadline) else {
        return false;
    };
    app.world_mut()
        .get_resource_or_init::<WokenDimensions>()
        .0
        .push(dim);
    true
}

fn drain_outbox(dimension: &mut SubApp) {
    if let Err(error) = dimension.world_mut().try_run_schedule(OutboxDrain) {
        tracing::error!(%error, "a dimension has no outbox drain");
    }
}
