use bevy_app::{App, AppExit};
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_engine::coordinator::default_game_mode::DefaultGameMode;
use mcrs_minecraft_engine_server::world_options::{FailedStart, WorldLock};
use mcrs_minecraft_engine_server::{
    DataPack, DimensionWorldsPlugin, GameMode, ServerCorePlugin, WorldOpenError,
};
use mcrs_minecraft_engine_world::world::dimension_list::DimensionList;
use mcrs_minecraft_engine_world::world_seed::WorldSeed;
use mcrs_minecraft_game_registries::dimension::Dimensions;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_game_registries::worldgen::world_preset::WorldPreset;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::nbt_compress::write_gzip_compound_tag_to_bytes;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_save::{
    Difficulty, DifficultySettings, LevelDat, RespawnData, SaveError, SaveLock, WorldGenSettings,
    read_level_dat, write_level_dat, write_world_gen_settings,
};
use std::path::Path;

const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");

fn started(worlds: DimensionWorldsPlugin) -> App {
    started_in(App::new(), worlds)
}

fn started_in(mut app: App, worlds: DimensionWorldsPlugin) -> App {
    app.add_plugins((
        ServerCorePlugin {
            owns_task_pools: false,
            asset_path: Some(ASSETS.to_owned()),
            data_pack: DataPack::Directory(mcrs_minecraft_fixtures::vanilla_dir().to_owned()),
            ..Default::default()
        },
        worlds,
    ));
    app.finish();
    app.cleanup();
    app.update();
    app
}

fn dimension_keys(app: &App) -> Vec<String> {
    app.world()
        .resource::<DimensionList>()
        .keys()
        .iter()
        .map(|key| key.as_str().to_owned())
        .collect()
}

fn scratch(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir()
        .join(format!("mcrs-new-world-{tag}-{}", Uuid::new_v4()))
        .join("My World")
}

fn written_by_a_creator(dir: &Path) {
    let level = LevelDat {
        level_name: "Named by the player".to_owned(),
        game_type: 1,
        allow_commands: true,
        difficulty_settings: DifficultySettings {
            difficulty: Difficulty::Hard,
            ..DifficultySettings::default()
        },
        initialized: false,
        ..LevelDat::default()
    };
    write_level_dat(dir, &level).unwrap();
    write_world_gen_settings(
        dir,
        &WorldGenSettings {
            seed: 42,
            dimensions: preset_dimensions(),
            bonus_chest: false,
            generate_structures: true,
            legacy_custom_options: None,
        },
        test_registries(),
    )
    .unwrap();
}

fn preset_dimensions() -> Dimensions {
    test_registries()
        .registry::<WorldPreset>()
        .and_then(|presets| {
            presets.get(presets.by_name(
                &mcrs_minecraft_resource::ResourceLocationBuf::read("minecraft:normal").ok()?,
            )?)
        })
        .unwrap()
        .dimensions
        .clone()
}

fn empty(_: &Path) {}

fn only_the_backup(dir: &Path) {
    written_by_a_creator(dir);
    std::fs::rename(dir.join("level.dat"), dir.join("level.dat_old")).unwrap();
}

#[derive(Clone, Copy)]
struct Expected {
    level_name: &'static str,
    game_type: i32,
    game_mode: GameMode,
    allow_commands: bool,
    difficulty: Difficulty,
}

#[test]
fn a_world_created_in_an_empty_folder_opens_again_with_its_seed() {
    let creator = Expected {
        level_name: "Named by the player",
        game_type: 1,
        game_mode: GameMode::Creative,
        allow_commands: true,
        difficulty: Difficulty::Hard,
    };
    let cases: [(&str, fn(&Path), Expected); 3] = [
        (
            "a folder that a creator wrote",
            written_by_a_creator,
            creator,
        ),
        ("a folder with only level.dat_old", only_the_backup, creator),
        (
            "an empty folder",
            empty,
            Expected {
                level_name: "My World",
                game_type: 0,
                game_mode: GameMode::Survival,
                allow_commands: false,
                difficulty: Difficulty::Normal,
            },
        ),
    ];
    for (name, prepare, expected) in cases {
        let dir = scratch("created");
        std::fs::create_dir_all(&dir).unwrap();
        prepare(&dir);
        let host = Uuid::from_u128(7);
        let first = started(DimensionWorldsPlugin {
            world: Some(dir.clone()),
            seed: 42,
            default_game_mode: GameMode::Survival,
            host: Some(host),
            ..Default::default()
        });
        assert_eq!(first.should_exit(), None, "{name}");
        let dimensions = dimension_keys(&first);
        drop(first);

        let second = started(DimensionWorldsPlugin {
            world: Some(dir.clone()),
            seed: 9,
            default_game_mode: GameMode::Adventure,
            ..Default::default()
        });
        assert_eq!(second.should_exit(), None, "{name}");
        assert_eq!(second.world().resource::<WorldSeed>().0, 42, "{name}");
        assert_eq!(dimension_keys(&second), dimensions, "{name}");
        assert_eq!(
            second.world().resource::<DefaultGameMode>().0,
            expected.game_mode,
            "{name}"
        );
        let level = read_level_dat(&dir).unwrap();
        assert_eq!(level.level_name, expected.level_name, "{name}");
        assert_eq!(level.game_type, expected.game_type, "{name}");
        assert_eq!(level.allow_commands, expected.allow_commands, "{name}");
        assert_eq!(
            level.difficulty_settings.difficulty, expected.difficulty,
            "{name}"
        );
        assert!(level.initialized, "{name}");
        assert_eq!(level.spawn, RespawnData::default(), "{name}");
        assert_eq!(level.singleplayer_uuid, Some(host), "{name}");
        assert!(!dir.join("level.dat_old").exists(), "{name}");
        std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }
}

fn level_with_data(dir: &Path, data: NbtCompound) {
    let mut root = NbtCompound::new();
    root.put_component("Data", data);
    std::fs::write(
        dir.join("level.dat"),
        write_gzip_compound_tag_to_bytes(&root).unwrap(),
    )
    .unwrap();
}

fn another_data_version(dir: &Path) {
    written_by_a_creator(dir);
    let mut data = NbtCompound::new();
    data.put_int("DataVersion", 1);
    level_with_data(dir, data);
}

fn no_data_version(dir: &Path) {
    written_by_a_creator(dir);
    level_with_data(dir, NbtCompound::new());
}

fn a_damaged_level(dir: &Path) {
    written_by_a_creator(dir);
    std::fs::write(dir.join("level.dat"), b"damaged").unwrap();
}

fn a_backup_of_another_data_version(dir: &Path) {
    written_by_a_creator(dir);
    let mut data = NbtCompound::new();
    data.put_int("DataVersion", 1);
    data.put_int("KeyOfAnotherVersion", 1);
    level_with_data(dir, data);
    std::fs::rename(dir.join("level.dat"), dir.join("level.dat_old")).unwrap();
}

fn a_damaged_backup(dir: &Path) {
    std::fs::write(dir.join("level.dat_old"), b"damaged").unwrap();
}

fn a_level_and_no_seed(dir: &Path) {
    write_level_dat(dir, &LevelDat::default()).unwrap();
}

fn files(dir: &Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_owned()];
    while let Some(next) = pending.pop() {
        for entry in std::fs::read_dir(next).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else if path.file_name().unwrap() != "session.lock" {
                let bytes = std::fs::read(&path).unwrap();
                found.push((path, bytes));
            }
        }
    }
    found.sort();
    found
}

#[test]
fn a_world_that_does_not_read_does_not_start_and_keeps_its_files() {
    let cases: [(&str, fn(&Path), fn(&WorldOpenError) -> bool); 6] = [
        (
            "another DataVersion",
            another_data_version,
            |error| matches!(error, WorldOpenError::DataVersion { found: Some(1), expected, .. } if *expected == VERSION.world_version),
        ),
        (
            "a level.dat_old of another DataVersion",
            a_backup_of_another_data_version,
            |error| matches!(error, WorldOpenError::DataVersion { found: Some(1), .. }),
        ),
        ("no DataVersion", no_data_version, |error| {
            matches!(error, WorldOpenError::DataVersion { found: None, .. })
        }),
        (
            "a damaged level.dat",
            a_damaged_level,
            |error| matches!(error, WorldOpenError::Save(SaveError::Nbt { path, .. }) if path.ends_with("level.dat")),
        ),
        (
            "a damaged level.dat_old",
            a_damaged_backup,
            |error| matches!(error, WorldOpenError::Save(SaveError::Nbt { path, .. }) if path.ends_with("level.dat_old")),
        ),
        (
            "a level.dat and no seed",
            a_level_and_no_seed,
            |error| matches!(error, WorldOpenError::Save(SaveError::Missing { path }) if path.ends_with("world_gen_settings.dat")),
        ),
    ];
    for (name, prepare, expected) in cases {
        let dir = scratch("refused");
        std::fs::create_dir_all(&dir).unwrap();
        prepare(&dir);
        let before = files(&dir);

        let app = started(DimensionWorldsPlugin {
            world: Some(dir.clone()),
            ..Default::default()
        });

        assert_eq!(app.should_exit(), Some(AppExit::error()), "{name}");
        let error = &app.world().resource::<FailedStart>().0;
        assert!(expected(error), "{name}: {error:?}");
        assert!(
            error.to_string().contains(dir.to_str().unwrap()),
            "{name}: {error}"
        );
        assert_eq!(files(&dir), before, "{name}");
        assert!(!SaveLock::is_held(&dir).unwrap(), "{name}");
        drop(app);
        std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }
}

#[test]
fn a_held_world_does_not_start_unless_its_lock_is_handed_in() {
    let dir = scratch("held");
    let lock = SaveLock::take(&dir).unwrap();
    let plugin = || DimensionWorldsPlugin {
        world: Some(dir.clone()),
        ..Default::default()
    };

    let refused = started(plugin());
    assert_eq!(refused.should_exit(), Some(AppExit::error()));
    assert!(matches!(
        refused.world().resource::<FailedStart>().0,
        WorldOpenError::Locked { .. }
    ));
    assert!(!dir.join("level.dat").exists());
    drop(refused);

    let mut host = App::new();
    host.insert_resource(WorldLock(lock));
    let opened = started_in(host, plugin());
    assert_eq!(opened.should_exit(), None);
    assert!(dir.join("level.dat").exists());
    assert!(SaveLock::is_held(&dir).unwrap());
    drop(opened);
    assert!(!SaveLock::is_held(&dir).unwrap());
    std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
}
