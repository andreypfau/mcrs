use bevy_app::{App, AppExit};
use mcrs_minecraft_engine::between_ticks::{
    drain_dim_despawn_queue, drain_dim_spawn_queue, take_dim_outputs,
};
use mcrs_minecraft_engine::muxer::bridge::run_pending_flush;
use mcrs_minecraft_engine::muxer::entity_replication::{self, Answers};
use mcrs_minecraft_engine::muxer::tick_wake::TickWake;
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

/// What the host does after each tick: it takes the dimensions' output and spawns or despawns
/// dimensions. A muxer on a thread of its own
/// works on the entity bytes of the tick meanwhile, and [`drain_until`] hands them over as they
/// arrive.
pub fn between_ticks(app: &mut App) {
    take_dim_outputs(app.world_mut());
    drain_dim_spawn_queue(app);
    drain_dim_despawn_queue(app);
}

/// Hands what the tick wrote for the clients to the sockets, then waits for `deadline`. A wake
/// before the deadline, such as the muxer handing out bytes, hands over again. Nothing here
/// changes what a tick reads.
pub fn drain_until(app: &mut App, deadline: Instant) {
    loop {
        drain(app);
        let woken = app
            .world_mut()
            .get_resource_or_init::<TickWake>()
            .wait(deadline);
        if !woken {
            return;
        }
    }
}

/// Hands what the host holds for the clients to the sockets, with the entity bytes the muxer
/// has handed out so far.
pub fn drain(app: &mut App) {
    let world = app.world_mut();
    entity_replication::deliver(world, Answers::Ready);
    run_pending_flush(world);
}
