use std::path::PathBuf;

use bevy_app::App;
use bevy_log::{Level, LogPlugin, tracing_subscriber};
use mcrs_minecraft_server::MinecraftServerPlugin;

const LOG_FILTER: &str = "mcrs_minecraft_server=debug,mcrs_minecraft_server::world::entity::player::digging=trace,mcrs_minecraft_server::world::entity::player::column_view=trace,mcrs_minecraft_level::entity::player::chunk_view=trace,mcrs_minecraft_level::world::storage::chunk=debug,mcrs_minecraft_network=debug,mcrs_lighting::case_a_cave=warn,mcrs_lighting::chimney_to_floor=warn,mcrs_lighting::needs_full_reseed=warn,mcrs_lighting::consume_reseed=warn";

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
    #[cfg(feature = "telemetry-diagnostics")]
    app.add_plugins((
        bevy_diagnostic::FrameTimeDiagnosticsPlugin::default(),
        bevy_diagnostic::EntityCountDiagnosticsPlugin::default(),
    ));
    app.add_plugins(MinecraftServerPlugin {
        world: world_folder(),
        announce_on_lan: lan_announce(std::env::var("MCRS_LAN_ANNOUNCE").ok().as_deref()),
        ..Default::default()
    });
    mcrs_minecraft_server::run_server_loop(app);
}

/// The first argument names a world folder whose saved chunks the server reads; without one it
/// generates.
fn world_folder() -> Option<PathBuf> {
    let path = std::env::args_os().nth(1).map(PathBuf::from)?;
    if !path.is_dir() {
        eprintln!("not a world folder: {}", path.display());
        std::process::exit(1);
    }
    Some(path)
}

/// `MCRS_LAN_ANNOUNCE=off` turns the LAN announcement off; `0`, `false` and `no` do the same, in
/// any case. Any other value leaves it on and is logged when it is neither on nor off.
fn lan_announce(value: Option<&str>) -> bool {
    match value
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        None | Some("" | "on" | "1" | "true" | "yes") => true,
        Some("off" | "0" | "false" | "no") => false,
        Some(other) => {
            eprintln!("MCRS_LAN_ANNOUNCE={other} is neither on nor off; announcing");
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::lan_announce;

    #[test]
    fn the_lan_setting_reads_its_spellings_in_any_case() {
        for off in ["OFF", "Off", " False ", "NO", "0", "fAlSe"] {
            assert!(!lan_announce(Some(off)), "{off:?}");
        }
        for on in [
            "ON", "On", " TRUE ", "Yes", "1", "disabled", "of f", "2", "",
        ] {
            assert!(lan_announce(Some(on)), "{on:?}");
        }
        assert!(lan_announce(None));
    }
}
