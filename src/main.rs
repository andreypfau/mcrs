use std::path::PathBuf;
use std::time::Duration;

use bevy_app::App;
use bevy_log::{Level, LogPlugin, tracing_subscriber};
use mcrs_minecraft_server::{GameMode, Lighting, MinecraftServerPlugin};

const LOG_FILTER: &str = "mcrs_minecraft_server=debug,mcrs_minecraft_server::world::entity::player::digging=trace,mcrs_minecraft_server::world::entity::player::column_view=trace,mcrs_minecraft_level::entity::player::chunk_view=trace,mcrs_minecraft_network=debug";

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
        announce_on_lan: on_off(
            "MCRS_LAN_ANNOUNCE",
            variable("MCRS_LAN_ANNOUNCE").as_deref(),
        ),
        offer_known_packs: on_off("MCRS_KNOWN_PACKS", variable("MCRS_KNOWN_PACKS").as_deref()),
        seed: world_seed(variable("MCRS_WORLD_SEED").as_deref()),
        preset: world_preset(variable("MCRS_WORLD_PRESET").as_deref()),
        lighting: lighting(variable("MCRS_NO_LIGHTING").as_deref()),
        default_game_mode: game_mode(variable("MCRS_DEFAULT_GAMEMODE").as_deref()),
        slow_column_threshold: slow_column_threshold(variable("MCRS_SLOW_CHUNK_MS").as_deref()),
        ..Default::default()
    });
    mcrs_minecraft_server::run_server_loop(app);
}

fn variable(name: &str) -> Option<String> {
    std::env::var(name).ok()
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

/// `off` turns a setting off; `0`, `false` and `no` do the same, in any case. Any other value
/// leaves it on and is logged when it is neither on nor off.
fn on_off(name: &str, value: Option<&str>) -> bool {
    match value
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        None | Some("" | "on" | "1" | "true" | "yes") => true,
        Some("off" | "0" | "false" | "no") => false,
        Some(other) => {
            eprintln!("{name}={other} is neither on nor off; leaving it on");
            true
        }
    }
}

/// `MCRS_WORLD_SEED` is written signed, as the Java long it is, or unsigned; the router hashes
/// the same bits either way.
fn world_seed(value: Option<&str>) -> u64 {
    let Some(raw) = value.map(str::trim) else {
        return 0;
    };
    if let Ok(seed) = raw.parse::<i64>() {
        return seed as u64;
    }
    match raw.parse::<u64>() {
        Ok(seed) => seed,
        Err(error) => {
            bevy_log::error!(%error, raw, "MCRS_WORLD_SEED is not a number; generating with seed 0");
            0
        }
    }
}

/// `MCRS_WORLD_PRESET` takes a short name (`normal`) or a namespaced one (`minecraft:normal`).
fn world_preset(value: Option<&str>) -> String {
    value
        .map(|name| name.trim().to_lowercase())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "normal".to_owned())
}

/// `MCRS_NO_LIGHTING=1` turns propagation off.
fn lighting(value: Option<&str>) -> Lighting {
    match value {
        Some("1" | "true" | "on" | "yes") => Lighting::FullSky,
        _ => Lighting::Propagated,
    }
}

/// `MCRS_DEFAULT_GAMEMODE` is `survival`, `creative`, `adventure` or `spectator`, in any case;
/// unset or anything else is creative.
fn game_mode(value: Option<&str>) -> GameMode {
    let Some(value) = value else {
        return GameMode::Creative;
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "survival" => GameMode::Survival,
        "creative" => GameMode::Creative,
        "adventure" => GameMode::Adventure,
        "spectator" => GameMode::Spectator,
        other => {
            bevy_log::warn!(
                value = other,
                "MCRS_DEFAULT_GAMEMODE unrecognized, defaulting to creative"
            );
            GameMode::Creative
        }
    }
}

/// `MCRS_SLOW_CHUNK_MS` is the latency in milliseconds above which a column is logged.
fn slow_column_threshold(value: Option<&str>) -> Duration {
    Duration::from_millis(value.and_then(|ms| ms.parse().ok()).unwrap_or(250))
}

#[cfg(test)]
mod tests {
    use super::{game_mode, on_off, world_seed};
    use mcrs_minecraft_server::GameMode;

    #[test]
    fn the_on_off_settings_read_their_spellings_in_any_case() {
        for name in ["MCRS_LAN_ANNOUNCE", "MCRS_KNOWN_PACKS"] {
            for off in ["OFF", "Off", " False ", "NO", "0", "fAlSe"] {
                assert!(!on_off(name, Some(off)), "{name}={off:?}");
            }
            for on in [
                "ON", "On", " TRUE ", "Yes", "1", "disabled", "of f", "2", "",
            ] {
                assert!(on_off(name, Some(on)), "{name}={on:?}");
            }
            assert!(on_off(name, None), "{name} unset");
        }
    }

    #[test]
    fn a_seed_reads_as_a_signed_or_an_unsigned_long() {
        for (value, seed) in [
            (None, 0),
            (Some(" 42 "), 42),
            (Some("-1"), u64::MAX),
            (Some("18446744073709551615"), u64::MAX),
            (Some("seed"), 0),
        ] {
            assert_eq!(world_seed(value), seed, "{value:?}");
        }
    }

    #[test]
    fn the_game_mode_reads_its_names_in_any_case_and_falls_back_to_creative() {
        for (value, mode) in [
            (None, GameMode::Creative),
            (Some(" Survival "), GameMode::Survival),
            (Some("ADVENTURE"), GameMode::Adventure),
            (Some("spectator"), GameMode::Spectator),
            (Some("hardcore"), GameMode::Creative),
        ] {
            assert_eq!(game_mode(value), mode, "{value:?}");
        }
    }
}
