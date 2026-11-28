use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::nbt_compress::write_gzip_compound_tag_to_bytes;
use mcrs_minecraft_save::{LEVEL_FORMAT_VERSION, WorldSummary, read_saves};

fn listed(file: &str, data: NbtCompound) -> Vec<WorldSummary> {
    let saves = std::env::temp_dir().join(format!("mcrs-world-list-{}", uuid::Uuid::new_v4()));
    let world = saves.join("world");
    std::fs::create_dir_all(&world).unwrap();
    let mut root = NbtCompound::new();
    root.put_component("Data", data);
    std::fs::write(
        world.join(file),
        write_gzip_compound_tag_to_bytes(&root).unwrap(),
    )
    .unwrap();
    let worlds = read_saves(&saves).unwrap();
    std::fs::remove_dir_all(&saves).unwrap();
    worlds
}

#[test]
fn an_unknown_game_type_lists_as_survival() {
    let mut data = NbtCompound::new();
    data.put_int("version", LEVEL_FORMAT_VERSION);
    data.put_int("GameType", 7);
    let worlds = listed("level.dat", data);
    let level = worlds[0].level.as_ref().unwrap();
    assert_eq!((level.game_type, level.allow_commands), (0, false));
}

#[test]
fn a_folder_with_only_the_backup_lists_as_unreadable() {
    let mut data = NbtCompound::new();
    data.put_int("version", LEVEL_FORMAT_VERSION);
    data.put_string("LevelName", "Backup".to_owned());
    let worlds = listed("level.dat_old", data);
    assert_eq!(worlds.len(), 1);
    assert_eq!(worlds[0].level, None);
    assert_eq!(worlds[0].name(), "world");
}
