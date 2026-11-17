use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bevy::ecs::message::Messages;
use bevy::ecs::resource::IsResource;
use bevy::prelude::*;
use mcrs_minecraft_client::local_server::LocalServerPlugin;
use mcrs_minecraft_client_entity::ClientEntityPlugin;
use mcrs_minecraft_client_world::chat::ClientChatPlugin;
use mcrs_minecraft_client_world::columns::ColumnCachePlugin;
use mcrs_minecraft_client_world::game_mode::GameModePlugin;
use mcrs_minecraft_client_world::game_packets::GAME_CLIENTBOUND;
use mcrs_minecraft_client_world::inventory_packets::InventoryPacketsPlugin;
use mcrs_minecraft_client_world::joined_game::JoinedGame;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::player_view::RequestedViewDistance;
use mcrs_minecraft_client_world::session::{
    LocalServer, LocalServerError, LocalServerStopped, LocalWorldOrigin, OpenLocalWorld,
};
use mcrs_minecraft_network::client::{ClientNetworkPlugin, ClientSession, LeaveRequest};
use mcrs_minecraft_save::SaveLock;
use mcrs_minecraft_server_settings::ServerSettings;

use crate::boot::boot;

const TIMEOUT: Duration = Duration::from_secs(40);

fn client() -> App {
    boot(|app| {
        app.add_plugins((
            ClientNetworkPlugin {
                holds_vanilla_pack: true,
                play: Some(GAME_CLIENTBOUND),
            },
            ColumnCachePlugin::default(),
            GameModePlugin,
            InventoryPacketsPlugin,
            ClientEntityPlugin,
            ClientChatPlugin {
                command_history: None,
            },
            LocalServerPlugin {
                asset_path: concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets").to_owned(),
                data_pack: mcrs_minecraft_fixtures::vanilla_dir().to_owned(),
                ops_file: None,
                username: "Player".to_owned(),
                open_to_lan: false,
                settings: ServerSettings::default(),
                column_traces: None,
            },
        ))
        .init_resource::<Time<Real>>()
        .insert_resource(RequestedViewDistance(2));
    })
}

fn world_folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("mcrs-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&folder).ok();
    folder
}

fn open(app: &mut App, folder: &Path) {
    app.world_mut().write_message(OpenLocalWorld {
        folder: folder.to_owned(),
        host: None,
        origin: LocalWorldOrigin::Opened,
        lock: None,
    });
}

fn update_until(app: &mut App, what: &str, mut done: impl FnMut(&mut App) -> bool) {
    let start = Instant::now();
    while !done(app) {
        assert!(start.elapsed() < TIMEOUT, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(5));
        app.update();
    }
}

fn joined(app: &mut App) -> bool {
    app.world_mut()
        .query_filtered::<(), (With<ClientSession>, With<JoinedGame>)>()
        .iter(app.world())
        .next()
        .is_some()
}

fn stopped(app: &mut App) -> Option<Result<(), LocalServerError>> {
    app.world_mut()
        .resource_mut::<Messages<LocalServerStopped>>()
        .drain()
        .last()
        .map(|LocalServerStopped(result)| result)
}

fn resources_and_entities(app: &mut App) -> (BTreeSet<String>, usize) {
    let world = app.world_mut();
    let resources = world
        .iter_resources()
        .map(|(_, info, _)| info.name().to_string())
        .collect();
    let entities = world
        .query_filtered::<(), Without<IsResource>>()
        .iter(world)
        .count();
    (resources, entities)
}

#[test]
fn leaving_a_local_world_restores_the_title_world() {
    let folder = world_folder("leave");
    let mut app = client();
    app.update();
    let title = resources_and_entities(&mut app);

    open(&mut app, &folder);
    update_until(&mut app, "the join", joined);
    app.world_mut().write_message(LeaveRequest);
    let mut result = None;
    update_until(&mut app, "the save", |app| {
        result = stopped(app);
        result.is_some()
    });
    assert_eq!(result, Some(Ok(())));
    app.update();
    assert_eq!(
        resources_and_entities(&mut app),
        title,
        "a session leaves nothing behind"
    );

    open(&mut app, &folder);
    update_until(&mut app, "the second join", joined);
    let players = app
        .world_mut()
        .query_filtered::<(), With<Player>>()
        .iter(app.world())
        .count();
    assert_eq!(players, 1);

    app.world_mut().write_message(AppExit::Success);
    app.update();
    let lock = SaveLock::take(&folder).expect("the server released the world");
    drop(lock);
    std::fs::remove_dir_all(&folder).ok();
}

#[test]
fn a_held_world_folder_starts_no_server() {
    let folder = world_folder("held");
    let lock = SaveLock::take(&folder).expect("the test takes the lock");
    let mut app = client();
    open(&mut app, &folder);
    let mut result = None;
    update_until(&mut app, "the failed start", |app| {
        result = stopped(app);
        result.is_some()
    });
    assert_eq!(result, Some(Err(LocalServerError::Held)));
    app.update();
    assert!(app.world().get_resource::<LocalServer>().is_none());
    assert!(
        !std::fs::exists(folder.join("level.dat")).unwrap(),
        "no server wrote the folder"
    );
    drop(lock);
    std::fs::remove_dir_all(&folder).ok();
}

#[test]
fn an_open_waits_for_the_running_server_and_a_further_open_is_refused() {
    let folders = ["wait-first", "wait-second", "wait-third"].map(world_folder);
    let locks = folders
        .each_ref()
        .map(|folder| SaveLock::take(folder).expect("the test takes the lock"));
    let mut app = client();
    let [first, second, third] = &folders;
    open(&mut app, first);
    app.update();
    open(&mut app, second);
    open(&mut app, third);
    let mut results = Vec::new();
    update_until(&mut app, "an answer to each open", |app| {
        results.extend(
            app.world_mut()
                .resource_mut::<Messages<LocalServerStopped>>()
                .drain()
                .map(|LocalServerStopped(result)| result),
        );
        results.len() >= 3 && app.world().get_resource::<LocalServer>().is_none()
    });
    results.sort_by_key(|result| format!("{result:?}"));
    assert_eq!(
        results,
        [
            Err(LocalServerError::Busy),
            Err(LocalServerError::Held),
            Err(LocalServerError::Held),
        ]
    );
    drop(locks);
    for folder in &folders {
        std::fs::remove_dir_all(folder).ok();
    }
}
