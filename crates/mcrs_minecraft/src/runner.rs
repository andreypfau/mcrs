use crate::world::sub_app_builder::{drain_dim_despawn_queue, drain_dim_spawn_queue};
use crate::DEFAULT_TPS;
use bevy_app::App;
use std::time::{Duration, Instant};

pub fn run_server_loop(mut app: App) {
    let tick = Duration::from_secs_f64(1.0 / DEFAULT_TPS.get() as f64);
    app.finish();
    app.cleanup();
    loop {
        let start = Instant::now();
        app.update();
        drain_dim_spawn_queue(&mut app);
        drain_dim_despawn_queue(&mut app);
        if app.should_exit().is_some() {
            break;
        }
        let elapsed = start.elapsed();
        if elapsed < tick {
            std::thread::sleep(tick - elapsed);
        }
    }
}
