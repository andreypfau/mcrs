use std::sync::LazyLock;

use mcrs_minecraft_fixtures::fixture;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_game_registries::world_preset_keys::NORMAL;
use mcrs_minecraft_save::{NewWorld, NewWorldMode, create_world, seed_from_text};
use serde::Deserialize;

#[derive(Deserialize)]
struct Capture {
    folder_ids: Vec<FolderIdRow>,
    seeds: Vec<SeedRow>,
}

#[derive(Deserialize)]
struct FolderIdRow {
    name: String,
    existing: Vec<String>,
    folder_id: String,
}

#[derive(Deserialize)]
struct SeedRow {
    text: String,
    seed: Option<i64>,
}

static CAPTURE: LazyLock<Capture> =
    LazyLock::new(|| serde_json::from_str(&fixture("local_worlds").text()).unwrap());

#[test]
fn a_new_world_gets_the_folder_id_of_the_game() {
    for row in &CAPTURE.folder_ids {
        let saves = std::env::temp_dir().join(format!("mcrs-folder-id-{}", uuid::Uuid::new_v4()));
        for folder in &row.existing {
            std::fs::create_dir_all(saves.join(folder)).unwrap();
        }
        let world = NewWorld {
            name: row.name.clone(),
            game_mode: NewWorldMode::Survival,
            world_type: NORMAL.location().to_buf(),
            seed_text: String::new(),
            random_seed: 0,
        };
        let lock = create_world(&saves, &world, test_registries(), 0).unwrap();
        assert_eq!(lock.dir(), saves.join(&row.folder_id), "{:?}", row.name);
        drop(lock);
        std::fs::remove_dir_all(saves).unwrap();
    }
}

#[test]
fn the_seed_text_gives_the_seed_of_the_game() {
    for row in &CAPTURE.seeds {
        assert_eq!(seed_from_text(&row.text), row.seed, "{:?}", row.text);
    }
}
