use bevy_app::{App, AppExit};
use bevy_ecs::resource::Resource;
use mcrs_minecraft_engine::between_ticks::run_between_ticks;
use mcrs_minecraft_engine::bridge::run_pending_flush;
use mcrs_minecraft_engine::dim::{drain_dim_despawn_queue, take_dim_outputs};
use mcrs_minecraft_engine::sub_app_builder::drain_dim_spawn_queue;
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
/// added to `BetweenTicks` and spawns or despawns dimensions.
pub fn between_ticks(app: &mut App) {
    take_dim_outputs(app.world_mut());
    run_between_ticks(app);
    drain_dim_spawn_queue(app);
    drain_dim_despawn_queue(app);
}

/// Hands what the tick wrote for the clients to the sockets, then waits for `deadline`. A wake
/// before the deadline hands over again. Nothing here changes what a tick reads.
pub fn drain_until(app: &mut App, deadline: Instant) {
    loop {
        drain(app);
        if !wait_for_wake(app, deadline) {
            return;
        }
    }
}

/// Hands what the host holds for the clients to the sockets.
pub fn drain(app: &mut App) {
    run_pending_flush(app.world_mut());
}

/// Where the server loop waits between ticks. Nothing wakes it yet; the sender it keeps makes
/// the wait last until the deadline instead of ending at once.
#[derive(Resource)]
struct TickWake {
    _sender: flume::Sender<()>,
    receiver: flume::Receiver<()>,
}

impl Default for TickWake {
    fn default() -> Self {
        let (sender, receiver) = flume::bounded(1);
        Self {
            _sender: sender,
            receiver,
        }
    }
}

/// Waits until a wake arrives or `deadline` passes, and tells whether a wake arrived.
fn wait_for_wake(app: &mut App, deadline: Instant) -> bool {
    app.world_mut()
        .get_resource_or_init::<TickWake>()
        .receiver
        .recv_deadline(deadline)
        .is_ok()
}
