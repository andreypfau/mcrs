use bevy_app::App;
use bevy_log::{Level, LogPlugin, tracing_subscriber};
use mcrs_minecraft_server::MinecraftServerPlugin;
use mcrs_telemetry::TelemetryPlugin;

mod chunk_render_debug;

const LOG_FILTER: &str = "mcrs_minecraft_server=debug,mcrs_minecraft_server::world::entity::player::digging=trace,mcrs_minecraft_server::world::entity::player::column_view=trace,mcrs_voxel_world::entity::player::chunk_view=trace,mcrs_voxel_world::world::storage::chunk=debug,mcrs_minecraft_network=debug,mcrs_lighting::case_a_cave=warn,mcrs_lighting::chimney_to_floor=warn,mcrs_lighting::needs_full_reseed=warn,mcrs_lighting::consume_reseed=warn";

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
    app.add_plugins(MinecraftServerPlugin::default());
    mcrs_minecraft_server::run_server_loop(app);
}
