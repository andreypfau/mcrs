use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bevy::ecs::message::Messages;
use bevy::prelude::*;
use mcrs_minecraft_client::local_server::LocalServerPlugin;
use mcrs_minecraft_client::world_list::WorldListPlugin;
use mcrs_minecraft_client_world::session::{LocalWorldOrigin, OpenLocalWorld};
use mcrs_minecraft_client_world::world_list::{
    CreateWorld, DeleteWorld, PlayWorld, ReadWorldList, WorldList, WorldStoreError,
    WorldStoreFailed, WorldTypes,
};
use mcrs_minecraft_lang::{EN_US, Language};
use mcrs_minecraft_model::resource_set::ResourceSet;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_save::{
    NewWorld, NewWorldMode, SaveLock, WorldSummary, read_level_dat, write_level_dat,
};
use mcrs_minecraft_server_settings::ServerSettings;

const TIMEOUT: Duration = Duration::from_secs(60);

const WORLD_TYPES: [&str; 5] = [
    "Default",
    "Superflat",
    "Large Biomes",
    "AMPLIFIED",
    "Single Biome",
];

fn location(name: &str) -> ResourceLocationBuf {
    ResourceLocationBuf::read(name).unwrap()
}

fn update_until(app: &mut App, done: impl Fn(&App) -> bool) {
    let start = Instant::now();
    while !done(app) {
        assert!(
            start.elapsed() < TIMEOUT,
            "the store did not finish in time"
        );
        std::thread::sleep(Duration::from_millis(5));
        app.update();
    }
}

fn read_list(app: &mut App) -> Vec<WorldSummary> {
    app.world_mut().write_message(ReadWorldList);
    app.update();
    update_until(app, |app| {
        matches!(app.world().resource::<WorldList>(), WorldList::Ready(_))
    });
    ready(app)
}

fn ready(app: &App) -> Vec<WorldSummary> {
    match app.world().resource::<WorldList>() {
        WorldList::Ready(worlds) => worlds.clone(),
        other => panic!("{other:?}"),
    }
}

fn new_world(name: &str, game_mode: NewWorldMode) -> NewWorld {
    NewWorld {
        name: name.to_owned(),
        game_mode,
        world_type: location("minecraft:normal"),
        seed_text: "12345".to_owned(),
        random_seed: 0,
    }
}

fn server() -> LocalServerPlugin {
    LocalServerPlugin {
        asset_path: concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets").to_owned(),
        data_pack: mcrs_minecraft_fixtures::vanilla_dir().to_owned(),
        ops_file: None,
        username: "Player".to_owned(),
        open_to_lan: false,
        settings: ServerSettings::default(),
        column_traces: None,
        extend_server: |_| {},
    }
}

fn assert_world_types(app: &App) {
    let WorldTypes::Ready(types) = app.world().resource::<WorldTypes>() else {
        panic!("the world types are not loaded");
    };
    let language = Language::from_json(ResourceSet::corpus().read(EN_US).unwrap()).unwrap();
    let texts: Vec<String> = types
        .iter()
        .map(|preset| {
            let key = format!("generator.{}.{}", preset.namespace(), preset.path());
            language.get(&key).to_owned()
        })
        .collect();
    assert_eq!(texts, WORLD_TYPES);
}

struct Listed {
    folder_id: &'static str,
    name: &'static str,
    game_type: i32,
    hardcore: bool,
    allow_commands: bool,
}

#[test]
fn created_worlds_are_listed_newest_first_and_a_held_one_survives_a_delete() {
    let saves = std::env::temp_dir().join(format!("mcrs-world-list-{}", std::process::id()));
    std::fs::remove_dir_all(&saves).ok();
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(WorldListPlugin {
            saves: saves.clone(),
            server: server(),
        });
    assert_eq!(read_list(&mut app), []);
    app.world_mut().write_message(ReadWorldList);
    app.update();
    assert_eq!(*app.world().resource::<WorldList>(), WorldList::Reading);

    for world in [
        new_world("Hard one", NewWorldMode::Hardcore),
        new_world("  Creative  ", NewWorldMode::Creative),
    ] {
        app.world_mut().write_message(CreateWorld(world));
    }
    let mut created = Vec::new();
    let start = Instant::now();
    while created.len() < 2 {
        assert!(
            start.elapsed() < TIMEOUT,
            "the worlds were not created in time"
        );
        app.update();
        created.extend(drain::<OpenLocalWorld>(&mut app).into_iter().map(handed));
        std::thread::sleep(Duration::from_millis(5));
    }
    let opened = ["Hard one", "Creative"].map(|folder| (saves.join(folder), true));
    assert_eq!(created, opened);

    assert_world_types(&app);

    let expected = [
        Listed {
            folder_id: "Creative",
            name: "Creative",
            game_type: 1,
            hardcore: false,
            allow_commands: true,
        },
        Listed {
            folder_id: "Hard one",
            name: "Hard one",
            game_type: 0,
            hardcore: true,
            allow_commands: false,
        },
    ];
    let worlds = read_list(&mut app);
    assert_eq!(worlds.len(), expected.len(), "{worlds:?}");
    for (world, expected) in worlds.iter().zip(&expected) {
        let level = world.level.as_ref().unwrap();
        assert_eq!(world.folder_id, expected.folder_id);
        assert_eq!(world.name(), expected.name);
        assert_eq!(level.game_type, expected.game_type, "{}", world.folder_id);
        assert_eq!(level.hardcore, expected.hardcore, "{}", world.folder_id);
        assert_eq!(
            level.allow_commands, expected.allow_commands,
            "{}",
            world.folder_id
        );
        assert!(!world.locked && !world.is_disabled(), "{}", world.folder_id);
    }

    assert_a_play_hands_the_host_the_world_holds_under_its_lock(&mut app, &saves.join("Creative"));

    let held = SaveLock::take(&saves.join("Hard one")).unwrap();
    let worlds = read_list(&mut app);
    assert!(
        worlds
            .iter()
            .all(|world| world.locked == (world.folder_id == "Hard one"))
    );

    assert_held_and_unlisted_plays_are_refused(&mut app);

    let target = move_behind_link(&saves.join("Creative"));
    let target_files = file_names(&target);

    for folder_id in ["Hard one", "Creative"] {
        app.world_mut().write_message(DeleteWorld {
            folder_id: folder_id.to_owned(),
        });
    }
    app.update();
    update_until(&mut app, |app| {
        matches!(app.world().resource::<WorldList>(), WorldList::Ready(_))
    });
    let worlds = ready(&app);
    assert_eq!(worlds.len(), 1, "{worlds:?}");
    assert_eq!(worlds[0].folder_id, "Hard one");
    assert!(worlds[0].locked);
    assert!(saves.join("Hard one/level.dat").exists());
    assert!(std::fs::symlink_metadata(saves.join("Creative")).is_err());
    assert_eq!(file_names(&target), target_files);
    assert!(target_files.iter().any(|name| name == "level.dat"));
    drop(held);
    std::fs::remove_dir_all(saves).unwrap();
    std::fs::remove_dir_all(target).unwrap();
}

fn handed(open: OpenLocalWorld) -> (PathBuf, bool) {
    let handed = open.host.is_none()
        && open.origin == LocalWorldOrigin::Created
        && open.lock.is_some_and(|lock| lock.dir() == open.folder);
    (open.folder, handed)
}

fn assert_a_play_hands_the_host_the_world_holds_under_its_lock(app: &mut App, folder: &Path) {
    for (case, host, backup_only) in [
        ("the host written after the listing", 7, false),
        ("the host only the backup holds", 8, true),
    ] {
        let host = Uuid::from_u128(host);
        let mut level = read_level_dat(folder).unwrap();
        level.singleplayer_uuid = Some(host);
        write_level_dat(folder, &level).unwrap();
        if backup_only {
            std::fs::rename(folder.join("level.dat"), folder.join("level.dat_old")).unwrap();
        }
        let open = play_creative(app);
        assert_eq!(open.host, Some(host), "{case}");
        assert_eq!(open.origin, LocalWorldOrigin::Opened, "{case}");
        assert!(open.lock.is_some_and(|lock| lock.dir() == folder), "{case}");
    }
}

fn play_creative(app: &mut App) -> OpenLocalWorld {
    drain::<OpenLocalWorld>(app);
    app.world_mut().write_message(PlayWorld {
        folder_id: "Creative".to_owned(),
    });
    let start = Instant::now();
    loop {
        assert!(
            start.elapsed() < TIMEOUT,
            "the world was not opened in time"
        );
        app.update();
        if let Some(open) = drain::<OpenLocalWorld>(app).pop() {
            return open;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn assert_held_and_unlisted_plays_are_refused(app: &mut App) {
    drain::<WorldStoreFailed>(app);
    drain::<OpenLocalWorld>(app);
    for folder_id in ["Hard one", "Gone"] {
        app.world_mut().write_message(PlayWorld {
            folder_id: folder_id.to_owned(),
        });
    }
    app.update();
    let refused = |folder_id: &str, error| WorldStoreFailed::Play {
        folder_id: folder_id.to_owned(),
        error,
    };
    assert_eq!(
        drain::<WorldStoreFailed>(app),
        [
            refused("Hard one", WorldStoreError::Held),
            refused("Gone", WorldStoreError::Io(std::io::ErrorKind::NotFound)),
        ]
    );
    assert!(drain::<OpenLocalWorld>(app).is_empty());
}

fn drain<M: Message>(app: &mut App) -> Vec<M> {
    app.world_mut()
        .resource_mut::<Messages<M>>()
        .drain()
        .collect()
}

#[test]
fn a_job_queued_behind_the_data_pack_load_waits_for_it_and_no_second_load_runs() {
    let saves = std::env::temp_dir().join(format!("mcrs-world-queue-{}", std::process::id()));
    std::fs::remove_dir_all(&saves).ok();
    let pack = std::env::temp_dir().join(format!("mcrs-world-queue-pack-{}", std::process::id()));
    std::fs::remove_dir_all(&pack).ok();
    link_folder(mcrs_minecraft_fixtures::vanilla_dir(), &pack);
    let store = |server: LocalServerPlugin| {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(WorldListPlugin {
                saves: saves.clone(),
                server,
            });
        app
    };
    let mut first = store(server());
    first
        .world_mut()
        .write_message(CreateWorld(new_world("Doomed", NewWorldMode::Survival)));
    update_until(&mut first, |app| {
        !app.world()
            .resource::<Messages<OpenLocalWorld>>()
            .is_empty()
    });
    drop(first);

    let mut app = store(LocalServerPlugin {
        data_pack: pack.clone(),
        ..server()
    });
    let listed = read_list(&mut app);
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(
        *app.world().resource::<WorldTypes>(),
        WorldTypes::Loading,
        "the load still runs"
    );
    app.world_mut().write_message(DeleteWorld {
        folder_id: "Doomed".to_owned(),
    });
    app.world_mut()
        .write_message(CreateWorld(new_world("Behind", NewWorldMode::Survival)));
    app.update();
    assert_eq!(
        *app.world().resource::<WorldList>(),
        WorldList::Reading,
        "a world being deleted is not listed"
    );

    update_until(&mut app, |app| {
        matches!(app.world().resource::<WorldTypes>(), WorldTypes::Ready(_))
    });
    std::fs::remove_dir_all(&pack).unwrap();
    update_until(&mut app, |app| {
        !app.world()
            .resource::<Messages<OpenLocalWorld>>()
            .is_empty()
            || !app
                .world()
                .resource::<Messages<WorldStoreFailed>>()
                .is_empty()
    });
    assert_eq!(drain::<WorldStoreFailed>(&mut app), [], "a second load ran");
    assert!(
        matches!(app.world().resource::<WorldTypes>(), WorldTypes::Ready(_)),
        "a second load ran"
    );
    update_until(&mut app, |app| {
        matches!(app.world().resource::<WorldList>(), WorldList::Ready(_))
    });
    let worlds = ready(&app);
    assert_eq!(
        worlds
            .iter()
            .map(|world| world.folder_id.as_str())
            .collect::<Vec<_>>(),
        ["Behind"]
    );
    std::fs::remove_dir_all(saves).unwrap();
}

#[test]
fn a_load_queued_after_a_failed_one_shows_the_world_types_loading() {
    let saves = std::env::temp_dir().join(format!("mcrs-world-retry-{}", std::process::id()));
    std::fs::remove_dir_all(&saves).ok();
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(WorldListPlugin {
            saves: saves.clone(),
            server: LocalServerPlugin {
                data_pack: saves.join("no data pack"),
                ..server()
            },
        });
    app.world_mut().write_message(ReadWorldList);
    update_until(&mut app, |app| {
        matches!(app.world().resource::<WorldTypes>(), WorldTypes::Failed(_))
    });
    app.world_mut().write_message(ReadWorldList);
    app.update();
    assert_eq!(*app.world().resource::<WorldTypes>(), WorldTypes::Loading);
    std::fs::remove_dir_all(saves).ok();
}

fn file_names(dir: &Path) -> Vec<std::ffi::OsString> {
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    names.sort();
    names
}

fn move_behind_link(folder: &Path) -> PathBuf {
    let target = std::env::temp_dir().join(format!("mcrs-linked-world-{}", std::process::id()));
    std::fs::remove_dir_all(&target).ok();
    std::fs::rename(folder, &target).unwrap();
    link_folder(&target, folder);
    target
}

#[cfg(unix)]
fn link_folder(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[cfg(windows)]
fn link_folder(target: &Path, link: &Path) {
    std::os::windows::fs::symlink_dir(target, link).unwrap();
}
