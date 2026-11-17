use std::time::{Duration, Instant};

use bevy_app::{App, AppExit};
use mcrs_minecraft_engine_server::{
    DataPack, DimensionWorldsPlugin, ServerCorePlugin, ServerNetworkPlugin, ServerThread,
    ServerThreadError,
};

const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");

fn server_app() -> Result<App, &'static str> {
    let mut app = App::new();
    app.add_plugins((
        ServerCorePlugin {
            owns_task_pools: false,
            asset_path: Some(ASSETS.to_owned()),
            data_pack: DataPack::Directory(mcrs_minecraft_fixtures::vanilla_dir().to_owned()),
            ..Default::default()
        },
        DimensionWorldsPlugin::default(),
        ServerNetworkPlugin {
            announce_on_lan: false,
            ..Default::default()
        },
    ));
    Ok(app)
}

fn failing_app() -> Result<App, &'static str> {
    Err("no data pack")
}

#[test]
fn a_server_thread_saves_and_finishes_on_request() {
    type Build = fn() -> Result<App, &'static str>;
    let cases: [(&str, Build, Result<AppExit, ServerThreadError<&str>>); 2] = [
        ("a server", server_app, Ok(AppExit::Success)),
        (
            "a failing build",
            failing_app,
            Err(ServerThreadError::Build("no data pack")),
        ),
    ];
    for (name, build, expected) in cases {
        let mut thread = ServerThread::start(build).unwrap();
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
        assert_eq!(finished, expected, "{name}");
    }
}
