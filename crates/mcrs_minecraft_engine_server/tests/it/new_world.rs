use bevy_app::{App, AppExit};
use mcrs_minecraft_engine::coordinator::default_game_mode::DefaultGameMode;
use mcrs_minecraft_engine_server::world_options::WorldLock;
use mcrs_minecraft_engine_server::{DataPack, DimensionWorldsPlugin, GameMode, ServerCorePlugin};
use mcrs_minecraft_engine_world::world::dimension_list::DimensionList;
use mcrs_minecraft_engine_world::world_seed::WorldSeed;
use mcrs_minecraft_game_registries::dimension::Dimensions;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_game_registries::worldgen::world_preset::WorldPreset;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_save::{
    Difficulty, DifficultySettings, LevelDat, RespawnData, SaveLock, WorldGenSettings,
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

struct Expected {
    level_name: &'static str,
    game_type: i32,
    game_mode: GameMode,
    allow_commands: bool,
    difficulty: Difficulty,
}

#[test]
fn a_world_created_in_an_empty_folder_opens_again_with_its_seed() {
    let cases: [(&str, fn(&Path), Expected); 2] = [
        (
            "a folder that a creator wrote",
            written_by_a_creator,
            Expected {
                level_name: "Named by the player",
                game_type: 1,
                game_mode: GameMode::Creative,
                allow_commands: true,
                difficulty: Difficulty::Hard,
            },
        ),
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
        std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }
}

#[test]
fn a_world_with_a_level_and_no_seed_does_not_start() {
    let dir = scratch("no-seed");
    std::fs::create_dir_all(&dir).unwrap();
    write_level_dat(&dir, &LevelDat::default()).unwrap();

    let app = started(DimensionWorldsPlugin {
        world: Some(dir.clone()),
        ..Default::default()
    });

    assert_eq!(app.should_exit(), Some(AppExit::error()));
    assert!(!dir.join("data/minecraft/world_gen_settings.dat").exists());
    std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
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
