#![expect(
    clippy::disallowed_methods,
    reason = "the server binary reads its settings from the environment"
)]

use std::path::{Path, PathBuf};

use bevy_app::{App, AppExit, Plugin, PluginGroup, Update};
use bevy_ecs::error::{BevyError, ErrorContext, FallbackErrorHandler, Severity};
use bevy_ecs::message::MessageWriter;
use bevy_log::{Level, LogPlugin, tracing_subscriber};
use mcrs_minecraft_client_jar::{Directory, Progress, RELEASE, candidates, locate};
use mcrs_minecraft_console::{ConsolePlugin, log_console_answers};
use mcrs_minecraft_engine::banlist::{BANLIST_FILE, BanList};
use mcrs_minecraft_engine::coordinator::ops::{OPS_FILE, OpList};
use mcrs_minecraft_engine::whitelist::{WHITELIST_FILE, Whitelist};
use mcrs_minecraft_engine_server::{
    DimensionWorldsPlugin, LoopbackOps, NetworkCompressionThreshold, ServerCorePlugin,
    ServerNetworkPlugin, ShutdownRequest,
};
use mcrs_minecraft_engine_world::world::sub_app::AddDimensionPlugins;
use mcrs_minecraft_lang::{EN_US, Language};
use mcrs_minecraft_server::{MinecraftServerPlugins, VanillaMobBrainPlugin};
use mcrs_minecraft_server_settings::ServerSettings;
use mcrs_minecraft_vanilla_worldgen::WorldGenPlugin;

const LOG_FILTER: &str = "mcrs_minecraft_server=debug,mcrs_minecraft_engine=debug,mcrs_minecraft_engine::muxer::column_views=trace,mcrs_minecraft_engine_world::entity::player::chunk_view=trace,mcrs_minecraft_network=debug";

// The server loop runs outside `block_on`: the network owns runtimes of its own, and tokio
// panics when one drops inside an async context, as on an exit before the first tick.
fn main() -> AppExit {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("the async runtime does not start: {error}");
            return AppExit::error();
        }
    };
    let _entered = runtime.enter();
    serve()
}

fn serve() -> AppExit {
    let mut app = App::new();
    app.insert_resource(FallbackErrorHandler(log_instead_of_panic));
    app.add_dimension_plugin(LogInsteadOfPanicPlugin);
    app.add_plugins(LogPlugin {
        filter: log_filter(std::env::var("RUST_LOG").ok().as_deref()),
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
    let settings = ServerSettings::read(
        |name| std::env::var(format!("MCRS_{name}")).ok(),
        |error| bevy_log::warn!("{error}"),
    );
    let mut server = MinecraftServerPlugins {
        core: ServerCorePlugin {
            compute_threads: settings.compute_threads,
            ..MinecraftServerPlugins::default().core
        },
        worlds: DimensionWorldsPlugin {
            world: world_folder(),
            ops,
            seed: settings.world_seed,
            preset: settings.world_preset.clone(),
            default_game_mode: settings.default_game_mode,
            ..Default::default()
        },
        world_gen: WorldGenPlugin,
        network: network(&settings),
    }
    .build();
    if !settings.mob_ai {
        server = server.disable::<VanillaMobBrainPlugin>();
    }
    app.add_plugins((
        server,
        ConsolePlugin {
            language: english(),
        },
    ));
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
    mcrs_minecraft_engine_server::run_server_loop_with(app, log_console_answers)
}

/// Logs an error that Bevy would panic on, such as a command for an entity that left the world,
/// so that the server keeps ticking.
fn log_instead_of_panic(error: BevyError, context: ErrorContext) {
    match error.severity() {
        Severity::Panic => bevy_ecs::error::error(error, context),
        _ => bevy_ecs::error::match_severity(error, context),
    }
}

/// [`log_instead_of_panic`] in a dimension world.
#[derive(Clone)]
struct LogInsteadOfPanicPlugin;

impl Plugin for LogInsteadOfPanicPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(FallbackErrorHandler(log_instead_of_panic));
    }
}

fn network(settings: &ServerSettings) -> ServerNetworkPlugin {
    let defaults = ServerNetworkPlugin::default();
    let mut bind_address = defaults.bind_address;
    if let Some(port) = settings.port {
        bind_address.set_port(port);
    }
    ServerNetworkPlugin {
        bind_address,
        announce_on_lan: settings.lan_announce,
        compression_threshold: settings.compression_threshold.map_or_else(
            NetworkCompressionThreshold::default,
            NetworkCompressionThreshold,
        ),
        accepts_transfers: settings.accept_transfers,
        hide_online_players: settings.hide_online_players,
        enable_status: settings.enable_status,
        whitelist: Whitelist::read(Path::new(WHITELIST_FILE)),
        banlist: BanList::read(Path::new(BANLIST_FILE)),
        // A player who joins from this machine is an operator for the session; the ops file
        // names every other operator.
        loopback_ops: LoopbackOps::Grant,
        ..defaults
    }
}

/// The English language of the installed client jar, for the console answers. Without the jar,
/// an answer reads as its translation key and its arguments.
fn english() -> Language {
    let Some(jar) = locate(&candidates(), &RELEASE.jar, &Progress::default()) else {
        bevy_log::warn!("no client jar; the console writes translation keys, not English");
        return Language::default();
    };
    let path = format!("assets/{EN_US}");
    let read = Directory::of(&jar)
        .and_then(|directory| directory.read(&jar, |name| name == path))
        .and_then(|files| {
            let (_, bytes) = files
                .into_iter()
                .next()
                .ok_or_else(|| format!("the client jar holds no {path}"))?;
            Language::from_json(&bytes).map_err(|error| error.to_string())
        });
    read.unwrap_or_else(|error| {
        bevy_log::warn!(%error, "the console writes translation keys, not English");
        Language::default()
    })
}

/// `RUST_LOG` replaces the default filter, so it can lower the levels that the default raises for
/// the server's own crates; bevy alone only adds it on top of them.
fn log_filter(rust_log: Option<&str>) -> String {
    rust_log
        .map(str::trim)
        .filter(|filter| !filter.is_empty())
        .unwrap_or(LOG_FILTER)
        .to_owned()
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
