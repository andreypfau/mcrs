//! A player file written by vanilla reads,
//! re-writes, and comes back structurally equal, with every unmodelled root key
//! intact. Compound key order is not compared: vanilla's own compound is a hash map.

use std::path::Path;
use std::sync::LazyLock;

use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_fixtures::fixture;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::nbt_compress::from_gzip_bytes;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_save::{PlayerDat, read_player_dat, write_player_dat};
use uuid::Uuid;

static VANILLA: LazyLock<Vec<u8>> = LazyLock::new(|| fixture("vanilla_player").bytes());

fn temp_world(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("mcrs_player_dat_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("players/data")).unwrap();
    dir
}

fn sorted(tag: &NbtTag) -> NbtTag {
    match tag {
        NbtTag::Compound(compound) => {
            let mut entries: Vec<(String, NbtTag)> = compound
                .child_tags
                .iter()
                .map(|(key, value)| (key.clone(), sorted(value)))
                .collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            NbtTag::Compound(NbtCompound {
                child_tags: entries,
            })
        }
        NbtTag::List(items) => NbtTag::List(items.iter().map(sorted).collect()),
        other => other.clone(),
    }
}

#[test]
fn the_vanilla_file_reads_and_rewrites_with_every_key() {
    let world = temp_world("round_trip");
    let uuid = Uuid::from_u128(0x1234);
    std::fs::write(
        world
            .join("players/data")
            .join(format!("{}.dat", uuid.hyphenated())),
        VANILLA.as_slice(),
    )
    .unwrap();

    let dat = read_player_dat(&world, uuid)
        .unwrap()
        .expect("the file exists");
    assert_eq!(dat.data_version, VERSION.world_version);
    assert_eq!(dat.pos(), Some([12.5, 64.0, -7.25]));
    assert_eq!(dat.rotation(), Some([90.0, -12.5]));
    assert_eq!(
        dat.dimension().as_ref(),
        Some(&mcrs_minecraft_dimension::dimension_keys::OVERWORLD.to_buf())
    );
    assert_eq!(
        dat.get("SelectedItemSlot").and_then(NbtTag::extract_int),
        Some(3)
    );
    let inventory = dat
        .get("Inventory")
        .and_then(NbtTag::extract_list)
        .expect("the vanilla file has an inventory");
    let slots: Vec<i8> = inventory
        .iter()
        .filter_map(|entry| entry.extract_compound()?.get_byte("Slot"))
        .collect();
    assert_eq!(slots, [0, 3, 9, 10, 20, 35]);
    let ids: Vec<&str> = inventory
        .iter()
        .filter_map(|entry| entry.extract_compound()?.get_string("id"))
        .collect();
    assert_eq!(ids[0], "minecraft:diamond_sword");
    assert_eq!(
        inventory[2].extract_compound().and_then(|entry| entry
            .get_int("count")
            .or_else(|| entry.get_byte("count").map(i32::from))),
        Some(64)
    );
    let equipment = dat
        .get("equipment")
        .and_then(NbtTag::extract_compound)
        .expect("the vanilla file has equipment");
    let mut equipment_keys: Vec<&str> = equipment
        .child_tags
        .iter()
        .map(|(key, _)| key.as_str())
        .collect();
    equipment_keys.sort_unstable();
    assert_eq!(equipment_keys, ["chest", "offhand"]);
    assert_eq!(
        equipment
            .get_compound("chest")
            .and_then(|entry| entry.get_string("id")),
        Some("minecraft:leather_chestplate")
    );
    for key in [
        "EnderItems",
        "Fire",
        "Motion",
        "OnGround",
        "UUID",
        "XpLevel",
        "abilities",
        "foodLevel",
        "foodSaturationLevel",
    ] {
        assert!(dat.get(key).is_some(), "the key {key} is kept");
    }
    assert_eq!(dat.get("Fire").and_then(NbtTag::extract_short), Some(-20));

    let other = Uuid::from_u128(0x5678);
    write_player_dat(&world, other, &dat).unwrap();
    let rewritten = std::fs::read(
        world
            .join("players/data")
            .join(format!("{}.dat", other.hyphenated())),
    )
    .unwrap();

    let original: NbtCompound = from_gzip_bytes(VANILLA.as_slice()).unwrap();
    let ours: NbtCompound = from_gzip_bytes(rewritten.as_slice()).unwrap();
    assert_eq!(
        sorted(&NbtTag::Compound(original.clone())),
        sorted(&NbtTag::Compound(ours.clone()))
    );

    let again = read_player_dat(&world, other).unwrap().unwrap();
    assert_eq!(again, dat);
    let _ = std::fs::remove_dir_all(world);
}

#[test]
fn a_missing_file_is_none_and_a_fresh_one_stamps_the_world_version() {
    let world = temp_world("fresh");
    let uuid = Uuid::from_u128(0x9);
    assert!(read_player_dat(&world, uuid).unwrap().is_none());
    write_player_dat(&world, uuid, &PlayerDat::default()).unwrap();
    let written = read_player_dat(&world, uuid).unwrap().unwrap();
    assert_eq!(written.data_version, VERSION.world_version);
    assert_eq!(written.pos(), Some([0.0; 3]));
    assert!(written.get("Inventory").is_none());
    assert!(
        !Path::new(
            &world
                .join("players/data")
                .join(format!("{}.dat.tmp", uuid.hyphenated()))
        )
        .exists()
    );
    let _ = std::fs::remove_dir_all(world);
}
