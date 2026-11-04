#![expect(
    clippy::disallowed_methods,
    reason = "the server binary reads its settings from the environment"
)]

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use bevy_app::{App, AppExit, PluginGroup, Update};
use bevy_ecs::message::MessageWriter;
use bevy_log::{Level, LogPlugin, tracing_subscriber};
use mcrs_minecraft_engine::coordinator::ops::{OPS_FILE, OpList};
use mcrs_minecraft_engine_server::{
    DimensionWorldsPlugin, GameMode, NetworkCompressionThreshold, Placement, ServerCorePlugin,
    ServerNetworkPlugin,
};
use mcrs_minecraft_server::MinecraftServerPlugins;
use mcrs_minecraft_vanilla_light::LightingPlugin;
use mcrs_minecraft_vanilla_worldgen::WorldGenPlugin;

const LOG_FILTER: &str = "mcrs_minecraft_server=debug,mcrs_minecraft_engine=debug,mcrs_minecraft_engine::muxer::column_views=trace,mcrs_minecraft_engine_world::entity::player::chunk_view=trace,mcrs_minecraft_network=debug";

#[tokio::main]
async fn main() -> AppExit {
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
    let ops = match OpList::read(Path::new(OPS_FILE)) {
        Ok(ops) => ops,
        Err(error) => {
            bevy_log::error!("{error}");
            return AppExit::error();
        }
    };
    let mut server = MinecraftServerPlugins {
        core: ServerCorePlugin {
            worlds: worlds(variable("MCRS_WORLDS").as_deref()),
            ..MinecraftServerPlugins::default().core
        },
        worlds: DimensionWorldsPlugin {
            world: world_folder(),
            ops,
            seed: world_seed(variable("MCRS_WORLD_SEED").as_deref()),
            preset: world_preset(variable("MCRS_WORLD_PRESET").as_deref()),
            default_game_mode: game_mode(variable("MCRS_DEFAULT_GAMEMODE").as_deref()),
            ..Default::default()
        },
        world_gen: WorldGenPlugin {
            slow_column_threshold: slow_column_threshold(variable("MCRS_SLOW_CHUNK_MS").as_deref()),
        },
        network: ServerNetworkPlugin {
            announce_on_lan: on_off(
                "MCRS_LAN_ANNOUNCE",
                variable("MCRS_LAN_ANNOUNCE").as_deref(),
                true,
            ),
            offer_known_packs: on_off(
                "MCRS_KNOWN_PACKS",
                variable("MCRS_KNOWN_PACKS").as_deref(),
                true,
            ),
            compression_threshold: compression_threshold(
                variable("MCRS_COMPRESSION_THRESHOLD").as_deref(),
            ),
            accepts_transfers: on_off(
                "MCRS_ACCEPT_TRANSFERS",
                variable("MCRS_ACCEPT_TRANSFERS").as_deref(),
                false,
            ),
            hide_online_players: on_off(
                "MCRS_HIDE_ONLINE_PLAYERS",
                variable("MCRS_HIDE_ONLINE_PLAYERS").as_deref(),
                false,
            ),
            enable_status: on_off(
                "MCRS_ENABLE_STATUS",
                variable("MCRS_ENABLE_STATUS").as_deref(),
                true,
            ),
            send_buffer_bytes: positive_count(
                "MCRS_SEND_BUFFER_BYTES",
                variable("MCRS_SEND_BUFFER_BYTES").as_deref(),
                ServerNetworkPlugin::default().send_buffer_bytes,
            ),
            accept_backlog: positive_count(
                "MCRS_ACCEPT_BACKLOG",
                variable("MCRS_ACCEPT_BACKLOG").as_deref(),
                ServerNetworkPlugin::default().accept_backlog,
            ),
            ..Default::default()
        },
    }
    .build();
    if !propagates_light(variable("MCRS_NO_LIGHTING").as_deref()) {
        server = server.disable::<LightingPlugin>();
    }
    app.add_plugins(server);
    let (signal_send, mut signal_recv) = tokio::sync::mpsc::channel(1);
    tokio::spawn(async move {
        shutdown_signal().await;
        let _ = signal_send.send(()).await;
        shutdown_signal().await;
        std::process::exit(1);
    });
    app.add_systems(
        Update,
        move |mut requests: MessageWriter<ShutdownRequest>| {
            if signal_recv.try_recv().is_ok() {
                requests.write(ShutdownRequest);
            }
        },
    );
    mcrs_minecraft_engine_server::run_server_loop(app)
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

/// `on`, `1`, `true` and `yes` turn a setting on; `off`, `0`, `false` and `no` turn it off, in any
/// case. An unset, empty or unrecognised value leaves the default, and the last two are logged.
fn on_off(name: &str, value: Option<&str>, default: bool) -> bool {
    let Some(raw) = value else {
        return default;
    };
    match raw.trim().to_ascii_lowercase().as_str() {
        "on" | "1" | "true" | "yes" => true,
        "off" | "0" | "false" | "no" => false,
        other => {
            let state = if default { "on" } else { "off" };
            eprintln!("{name}={other:?} is neither on nor off; leaving it {state}");
            default
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

/// `MCRS_WORLDS` is `inline` or `thread`, in any case; unset or anything else keeps the
/// default placement.
fn worlds(value: Option<&str>) -> Placement {
    let default = ServerCorePlugin::default().worlds;
    let Some(value) = value else {
        return default;
    };
    value.trim().parse().unwrap_or_else(|error| {
        bevy_log::warn!(value, worlds = ?default, "MCRS_WORLDS: {error}; keeping the default");
        default
    })
}

/// `MCRS_NO_LIGHTING=1` turns propagation off.
fn propagates_light(value: Option<&str>) -> bool {
    !matches!(value, Some("1" | "true" | "on" | "yes"))
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

/// `MCRS_COMPRESSION_THRESHOLD` is the packet size in bytes from which packets are compressed;
/// a negative value turns compression off.
fn compression_threshold(value: Option<&str>) -> NetworkCompressionThreshold {
    let Some(raw) = value.map(str::trim) else {
        return NetworkCompressionThreshold::default();
    };
    match raw.parse() {
        Ok(threshold) => NetworkCompressionThreshold(threshold),
        Err(error) => {
            bevy_log::error!(%error, raw, "MCRS_COMPRESSION_THRESHOLD is not a number; using the default");
            NetworkCompressionThreshold::default()
        }
    }
}

/// A positive whole number; unset or anything else keeps `default`.
fn positive_count<T: FromStr + Default + PartialOrd>(
    name: &str,
    value: Option<&str>,
    default: T,
) -> T {
    let Some(raw) = value.map(str::trim) else {
        return default;
    };
    match raw.parse() {
        Ok(count) if count > T::default() => count,
        _ => {
            bevy_log::error!(raw, "{name} is not a positive number; using the default");
            default
        }
    }
}

/// `MCRS_SLOW_CHUNK_MS` is the latency in milliseconds above which a column is logged.
fn slow_column_threshold(value: Option<&str>) -> Duration {
    Duration::from_millis(value.and_then(|ms| ms.parse().ok()).unwrap_or(250))
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut terminate) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = terminate.recv() => {}
                }
                return;
            }
            Err(error) => bevy_log::error!("SIGTERM is not handled: {error}"),
        }
    }
    if let Err(error) = tokio::signal::ctrl_c().await {
        bevy_log::error!("Ctrl+C is not handled: {error}");
        std::future::pending::<()>().await;
    }
}

#[cfg(test)]
mod tests {
    use super::{game_mode, on_off, world_seed};
    use mcrs_minecraft_engine_server::GameMode;

    #[test]
    fn the_on_off_settings_read_their_spellings_in_any_case() {
        for default in [true, false] {
            for off in ["OFF", "Off", " False ", "NO", "0", "fAlSe"] {
                assert!(!on_off("MCRS_TEST", Some(off), default), "{off:?}");
            }
            for on in ["ON", "On", " TRUE ", "Yes", "1"] {
                assert!(on_off("MCRS_TEST", Some(on), default), "{on:?}");
            }
            for other in ["disabled", "of f", "2", ""] {
                assert_eq!(
                    on_off("MCRS_TEST", Some(other), default),
                    default,
                    "{other:?}"
                );
            }
            assert_eq!(on_off("MCRS_TEST", None, default), default);
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
