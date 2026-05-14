use bevy_app::App;
use bevy_app::ScheduleRunnerPlugin;
use bevy_log::{Level, LogPlugin, tracing_subscriber};
use mcrs_minecraft::ServerPlugin;

mod chunk_render_debug;

const LOG_FILTER: &str =
    "mcrs_minecraft=debug,mcrs_minecraft::world::entity::player::digging=trace,mcrs_minecraft::world::entity::player::column_view=trace,mcrs_engine::entity::player::chunk_view=trace,mcrs_engine::world::chunk=debug,mcrs_network=debug";

#[tokio::main]
async fn main() {
    App::new()
        .add_plugins(ScheduleRunnerPlugin::run_loop(
            std::time::Duration::from_secs_f64(1.0 / mcrs_minecraft::DEFAULT_TPS.get() as f64),
        ))
        .add_plugins(LogPlugin {
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
        })
        .add_plugins(ServerPlugin)
        .run();
}
