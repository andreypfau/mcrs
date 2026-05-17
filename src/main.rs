use bevy_app::App;
use bevy_log::{tracing_subscriber, Level, LogPlugin};
use mcrs_minecraft::world::sub_app_builder::{drain_dim_despawn_queue, drain_dim_spawn_queue};
use mcrs_minecraft::{ServerPlugin, DEFAULT_TPS};
use std::time::{Duration, Instant};

mod chunk_render_debug;

const LOG_FILTER: &str =
    "mcrs_minecraft=debug,mcrs_minecraft::world::entity::player::digging=trace,mcrs_minecraft::world::entity::player::column_view=trace,mcrs_engine::entity::player::chunk_view=trace,mcrs_engine::world::chunk=debug,mcrs_network=debug,mcrs_lighting::case_a_cave=warn,mcrs_lighting::chimney_to_bedrock=warn,mcrs_lighting::needs_full_reseed=warn,mcrs_lighting::consume_reseed=warn";

#[tokio::main]
async fn main() {
    let mut app = App::new();
    app.add_plugins(LogPlugin {
        filter: LOG_FILTER.to_string(),
        level: Level::INFO,
        fmt_layer: |_| {
            Some(Box::new(
                tracing_subscriber::fmt::Layer::default()
                    .with_writer(std::io::stderr)
                    .with_ansi(false),
            ))
        },
        ..Default::default()
    });
    app.add_plugins(ServerPlugin);
    run_server_loop(app);
}

/// Manual tick loop. Owns `&mut App` between ticks so the sub-app spawn and
/// despawn drains can call `App::insert_sub_app` and `App::remove_sub_app`.
fn run_server_loop(mut app: App) {
    let tick = Duration::from_secs_f64(1.0 / DEFAULT_TPS.get() as f64);
    app.finish();
    app.cleanup();
    loop {
        let start = Instant::now();
        app.update();
        drain_dim_spawn_queue(&mut app);
        drain_dim_despawn_queue(&mut app);
        let elapsed = start.elapsed();
        if elapsed < tick {
            std::thread::sleep(tick - elapsed);
        }
    }
}
