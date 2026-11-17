use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_game_registries::world_preset_keys::NORMAL;
use mcrs_minecraft_save::{NewWorld, NewWorldMode, SaveLock, create_world};

#[test]
fn a_blank_name_takes_the_first_free_default_folder_and_holds_its_lock() {
    let saves = std::env::temp_dir().join(format!("mcrs-folder-id-{}", uuid::Uuid::new_v4()));
    for folder in ["New World", "New World (1)"] {
        std::fs::create_dir_all(saves.join(folder)).unwrap();
    }
    let world = NewWorld {
        name: "  ".to_owned(),
        game_mode: NewWorldMode::Survival,
        world_type: NORMAL.location().to_buf(),
        seed_text: String::new(),
        random_seed: 0,
    };
    let expected = saves.join("New World (2)");
    let lock = create_world(&saves, &world, test_registries(), 0).unwrap();
    assert_eq!(lock.dir(), expected);
    assert!(SaveLock::is_held(lock.dir()).unwrap());
    drop(lock);
    assert!(!SaveLock::is_held(&expected).unwrap());
    std::fs::remove_dir_all(saves).unwrap();
}
