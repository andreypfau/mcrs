use bevy_app::App;
use bevy_log::{tracing_subscriber, Level, LogPlugin};
use mcrs_minecraft::ServerPlugin;
use mcrs_telemetry::TelemetryPlugin;

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
    app.add_plugins(TelemetryPlugin);
    app.add_plugins(ServerPlugin);
    mcrs_minecraft::run_server_loop(app);
}
