use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::nbt_compress::write_gzip_compound_tag_to_bytes;
use mcrs_minecraft_save::{LEVEL_FORMAT_VERSION, read_saves};

#[test]
fn an_unknown_game_type_lists_as_survival() {
    let saves = std::env::temp_dir().join(format!("mcrs-world-list-{}", uuid::Uuid::new_v4()));
    let world = saves.join("world");
    std::fs::create_dir_all(&world).unwrap();
    let mut data = NbtCompound::new();
    data.put_int("version", LEVEL_FORMAT_VERSION);
    data.put_int("GameType", 7);
    let mut root = NbtCompound::new();
    root.put_component("Data", data);
    std::fs::write(
        world.join("level.dat"),
        write_gzip_compound_tag_to_bytes(&root).unwrap(),
    )
    .unwrap();

    let worlds = read_saves(&saves).unwrap();
    std::fs::remove_dir_all(&saves).unwrap();
    let level = worlds[0].level.as_ref().unwrap();
    assert_eq!((level.game_type, level.allow_commands), (0, false));
}
