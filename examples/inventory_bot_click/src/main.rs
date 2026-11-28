use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use bevy_app::App;
use bevy_app::ScheduleRunnerPlugin;
use bevy_log::LogPlugin;
use mcrs_minecraft_assets::packs::PackRoots;
use mcrs_minecraft_inventory_bot_click::ClickScript;
use mcrs_minecraft_inventory_bot_click::ClickScriptPlugin;
use mcrs_minecraft_inventory_bot_click::HeadlessClientPlugin;
use mcrs_minecraft_inventory_bot_click::JoinServerPlugin;

const USAGE: &str = "usage: mcrs_minecraft_inventory_bot_click <server address> <username> <assets folder> <vanilla data pack folder>";
const FRAME: Duration = Duration::from_millis(5);

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [server, username, assets, vanilla] = arguments.as_slice() else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };
    let server: SocketAddr = match server.parse() {
        Ok(server) => server,
        Err(error) => {
            eprintln!("{server}: {error}\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    if App::new()
        .add_plugins((
            LogPlugin::default(),
            ScheduleRunnerPlugin::run_loop(FRAME),
            HeadlessClientPlugin {
                packs: PackRoots {
                    assets: assets.clone(),
                    vanilla: PathBuf::from(vanilla),
                },
            },
            JoinServerPlugin {
                server,
                username: username.clone(),
            },
            ClickScriptPlugin,
        ))
        .init_resource::<ClickScript>()
        .run()
        .is_success()
    {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
