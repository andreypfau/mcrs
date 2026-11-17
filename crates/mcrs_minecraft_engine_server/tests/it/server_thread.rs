use std::path::PathBuf;
use std::time::{Duration, Instant};

use bevy_app::{App, AppExit};
use mcrs_minecraft_engine::server_edit::Weather;
use mcrs_minecraft_engine_server::{
    DataPack, DimensionWorldsPlugin, ServerCorePlugin, ServerNetworkPlugin, ServerThread,
    ServerThreadError, WorldOpenError,
};

const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");

fn server_app(world: PathBuf) -> Result<App, &'static str> {
    let mut app = App::new();
    app.add_plugins((
        ServerCorePlugin {
            owns_task_pools: false,
            asset_path: Some(ASSETS.to_owned()),
            data_pack: DataPack::Directory(mcrs_minecraft_fixtures::vanilla_dir().to_owned()),
            ..Default::default()
        },
        DimensionWorldsPlugin {
            world: Some(world),
            ..Default::default()
        },
        ServerNetworkPlugin {
            announce_on_lan: false,
            ..Default::default()
        },
    ));
    app.insert_resource(Weather {
        raining: true,
        ..Weather::default()
    });
    Ok(app)
}

fn held_server_app(world: PathBuf) -> Result<App, &'static str> {
    let lock = mcrs_minecraft_save::SaveLock::take(&world).unwrap();
    let app = server_app(world);
    drop(lock);
    app
}

fn failing_app(_: PathBuf) -> Result<App, &'static str> {
    Err("no data pack")
}

fn app_without_shutdown(_: PathBuf) -> Result<App, &'static str> {
    Ok(App::new())
}

#[test]
fn a_server_thread_saves_and_finishes_on_request() {
    type Build = fn(PathBuf) -> Result<App, &'static str>;
    type Finished = Result<AppExit, ServerThreadError<&'static str>>;
    let cases: [(&str, Build, fn(&Finished) -> bool); 4] = [
        ("a server", server_app, |finished| {
            matches!(finished, Ok(AppExit::Success))
        }),
        ("a failing build", failing_app, |finished| {
            matches!(finished, Err(ServerThreadError::Build("no data pack")))
        }),
        (
            "an app without a shutdown plugin",
            app_without_shutdown,
            |finished| matches!(finished, Err(ServerThreadError::NoShutdownPlugin)),
        ),
        ("a held world", held_server_app, |finished| {
            matches!(
                finished,
                Err(ServerThreadError::World(WorldOpenError::Locked { .. }))
            )
        }),
    ];
    for (name, build, expected) in cases {
        let world = std::env::temp_dir().join(format!(
            "mcrs-server-thread-{}",
            mcrs_minecraft_protocol::uuid::Uuid::new_v4()
        ));
        let folder = world.clone();
        let mut thread = ServerThread::start(move || build(folder)).unwrap();
        let deadline = Instant::now() + Duration::from_secs(60);
        let finished = loop {
            if thread.address().is_some() {
                thread.request_shutdown();
            }
            if let Some(finished) = thread.try_finish() {
                break finished;
            }
            assert!(
                Instant::now() < deadline,
                "{name}: the thread did not finish"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        let saved = mcrs_minecraft_save::read_weather(&world).is_ok_and(|weather| weather.raining);
        assert_eq!(saved, finished.is_ok(), "{name}: the weather on disk");
        assert!(expected(&finished), "{name}: {finished:?}");
        let _ = std::fs::remove_dir_all(world);
    }
}
