#![expect(
    clippy::disallowed_methods,
    reason = "the server binary reads its settings from the environment"
)]

use std::path::{Path, PathBuf};

use bevy_app::{App, AppExit, Plugin, PluginGroup, Update};
use bevy_ecs::error::{BevyError, ErrorContext, FallbackErrorHandler, Severity};
use bevy_ecs::message::MessageWriter;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::{Res, ResMut};
use bevy_log::{Level, LogPlugin, tracing_subscriber};
use mcrs_minecraft_client_jar::{Directory, Progress, RELEASE, candidates, locate};
use mcrs_minecraft_console::{ConsolePlugin, log_console_answers};
use mcrs_minecraft_engine::banlist::{BANLIST_FILE, BanList};
use mcrs_minecraft_engine::coordinator::ops::{OPS_FILE, OpList};
use mcrs_minecraft_engine::whitelist::{WHITELIST_FILE, Whitelist};
use mcrs_minecraft_engine_server::{
    DimensionWorldsPlugin, EmptyForwardingSecret, ForwardingSecret, NetworkCompressionThreshold,
    ServerCorePlugin, ServerNetworkPlugin, ShutdownRequest,
};
use mcrs_minecraft_engine_world::world::sub_app::AddDimensionPlugins;
use mcrs_minecraft_lang::{EN_US, Language};
use mcrs_minecraft_server::{MinecraftServerPlugins, VanillaMobBrainPlugin};
use mcrs_minecraft_server_settings::ServerSettings;
use mcrs_minecraft_vanilla_worldgen::WorldGenPlugin;

const LOG_FILTER: &str = "mcrs_minecraft_server=debug,mcrs_minecraft_engine=debug,mcrs_minecraft_engine::muxer::column_views=trace,mcrs_minecraft_engine_world::entity::player::chunk_view=trace,mcrs_minecraft_network=debug";

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
    let mut unreadable_proxy_setting = None;
    let settings = ServerSettings::read(
        |name| std::env::var(format!("MCRS_{name}")).ok(),
        |error| {
            if matches!(error.name, "FORWARDING" | "PROXY_UPSTREAMS") {
                unreadable_proxy_setting.get_or_insert(error);
            } else {
                bevy_log::warn!("{error}");
            }
        },
    );
    if let Some(error) = unreadable_proxy_setting {
        bevy_log::error!(
            "MCRS_{}={:?} is not {}; the server does not start",
            error.name,
            error.value,
            error.expected
        );
        return AppExit::error();
    }
    let network = match network(&settings) {
        Ok(network) => network,
        Err(error) => {
            bevy_log::error!("{error}");
            return AppExit::error();
        }
    };
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
        network,
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
    forward_shutdown_signal(&mut app);
    mcrs_minecraft_engine_server::run_server_loop_with(app, log_console_answers)
}

#[derive(Resource)]
struct ShutdownSignal(tokio::sync::mpsc::Receiver<()>);

fn forward_shutdown_signal(app: &mut App) {
    let (signal_send, signal_recv) = tokio::sync::mpsc::channel(1);
    tokio::spawn(async move {
        shutdown_signal().await;
        let _ = signal_send.send(()).await;
        shutdown_signal().await;
        std::process::exit(1);
    });
    app.insert_resource(ShutdownSignal(signal_recv));
    app.add_systems(Update, request_shutdown.run_if(shutdown_signalled));
}

fn shutdown_signalled(signal: Res<ShutdownSignal>) -> bool {
    !signal.0.is_empty()
}

fn request_shutdown(
    mut signal: ResMut<ShutdownSignal>,
    mut requests: MessageWriter<ShutdownRequest>,
) {
    if signal.0.try_recv().is_ok() {
        requests.write(ShutdownRequest);
    }
}

fn log_instead_of_panic(error: BevyError, context: ErrorContext) {
    match error.severity() {
        Severity::Panic => bevy_ecs::error::error(error, context),
        _ => bevy_ecs::error::match_severity(error, context),
    }
}

#[derive(Clone)]
struct LogInsteadOfPanicPlugin;

impl Plugin for LogInsteadOfPanicPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(FallbackErrorHandler(log_instead_of_panic));
    }
}

fn network(settings: &ServerSettings) -> Result<ServerNetworkPlugin, EmptyForwardingSecret> {
    let defaults = ServerNetworkPlugin::default();
    let mut bind_address = defaults.bind_address;
    if let Some(port) = settings.port {
        bind_address.set_port(port);
    }
    let forwarding = settings
        .forwarding
        .then(|| ForwardingSecret::new(&settings.forwarding_secret))
        .transpose()?;
    Ok(ServerNetworkPlugin {
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
        forwarding,
        proxy_upstreams: settings.proxy_upstreams.clone(),
        ..defaults
    })
}

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

fn log_filter(rust_log: Option<&str>) -> String {
    rust_log
        .map(str::trim)
        .filter(|filter| !filter.is_empty())
        .unwrap_or(LOG_FILTER)
        .to_owned()
}

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
