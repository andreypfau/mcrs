use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_game_registries::world_preset_keys::NORMAL;
use mcrs_minecraft_save::{NewWorld, NewWorldMode, SaveLock, create_world, seed_from_text};

#[test]
fn a_world_name_gives_the_first_free_folder_id() {
    let cases: [(&str, &[&str], &str); 6] = [
        ("New World", &[], "New World"),
        ("New World", &["New World"], "New World (1)"),
        ("  ", &["New World", "New World (1)"], "New World (2)"),
        ("a.b:c", &[], "a_b_c"),
        ("con", &[], "_con_"),
        ("Map (1)", &["Map (1)"], "Map (1) (1)"),
    ];
    for (name, existing, expected) in cases {
        let saves = std::env::temp_dir().join(format!("mcrs-folder-id-{}", uuid::Uuid::new_v4()));
        for folder in existing {
            std::fs::create_dir_all(saves.join(folder)).unwrap();
        }
        let world = NewWorld {
            name: name.to_owned(),
            game_mode: NewWorldMode::Survival,
            world_type: NORMAL.location().to_buf(),
            seed_text: String::new(),
            random_seed: 0,
        };
        let lock = create_world(&saves, &world, test_registries(), 0).unwrap();
        assert_eq!(lock.dir(), saves.join(expected), "{name:?}");
        assert!(SaveLock::is_held(lock.dir()).unwrap(), "{name:?}");
        drop(lock);
        assert!(
            !SaveLock::is_held(&saves.join(expected)).unwrap(),
            "{name:?}"
        );
        std::fs::remove_dir_all(saves).unwrap();
    }
}

#[test]
fn the_seed_text_gives_its_number_or_its_hash() {
    let cases = [
        ("12345", 12345),
        (" -7 ", -7),
        ("abc", 96354),
        ("9223372036854775808", -1_773_151_197),
        ("0x10", 1_546_855),
    ];
    for (text, expected) in cases {
        assert_eq!(seed_from_text(text), Some(expected), "{text:?}");
    }
}
