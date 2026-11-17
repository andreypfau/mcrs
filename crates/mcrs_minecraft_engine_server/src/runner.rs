use bevy_app::{App, AppExit};
use bevy_ecs::world::World;
use mcrs_minecraft_engine::between_ticks::{
    drain_dim_despawn_queue, drain_dim_spawn_queue, take_dim_outputs,
};
use mcrs_minecraft_engine::muxer::bridge::run_pending_flush;
use mcrs_minecraft_engine::muxer::muxer_link::{self, Answers};
use mcrs_minecraft_engine::muxer::tick_wake::TickWake;
use std::num::NonZeroU32;
use std::time::Instant;

pub const DEFAULT_TPS: NonZeroU32 = match NonZeroU32::new(20) {
    Some(tps) => tps,
    None => NonZeroU32::MIN,
};

pub fn run_server_loop(app: App) -> AppExit {
    run_server_loop_with(app, |_| {})
}

pub fn run_server_loop_with(app: App, after_tick: fn(&mut World)) -> AppExit {
    mcrs_minecraft_engine_world::server_loop::run_server_loop(
        app,
        DEFAULT_TPS,
        |app| {
            between_ticks(app);
            after_tick(app.world_mut());
        },
        drain_until,
    )
}

pub fn between_ticks(app: &mut App) {
    take_dim_outputs(app.world_mut());
    drain_dim_spawn_queue(app);
    drain_dim_despawn_queue(app);
}

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

pub fn drain(app: &mut App) {
    let world = app.world_mut();
    muxer_link::deliver(world, Answers::Ready);
    run_pending_flush(world);
}
