//! A player file the vanilla 26.3-snapshot-10 codecs wrote (`ItemStackWithSlot.CODEC`,
//! `EntityEquipment.CODEC`, DataVersion 5015) reads, re-writes, and comes back
//! structurally equal, with every unmodelled root key intact. Compound key
//! order is not compared: vanilla's own compound is a hash map.

use std::path::Path;

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::nbt_compress::from_gzip_bytes;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_world::save::{
    PlayerDat, WORLD_VERSION, read_player_dat, write_player_dat,
};
use uuid::Uuid;

const VANILLA: &[u8] = include_bytes!("fixtures/vanilla_player_5015.dat");

fn temp_world(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("mcrs_player_dat_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("players/data")).unwrap();
    dir
}

fn get<'a>(compound: &'a NbtCompound, key: &str) -> &'a NbtTag {
    compound
        .get(key)
        .unwrap_or_else(|| panic!("{key} is present"))
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
fn the_vanilla_file_round_trips_through_the_typed_shape() {
    let world = temp_world("round_trip");
    let uuid = Uuid::from_u128(0x1234);
    std::fs::write(
        world.join("players/data").join(format!("{}.dat", uuid.hyphenated())),
        VANILLA,
    )
    .unwrap();

    let dat = read_player_dat(&world, uuid).unwrap().expect("the file exists");
    assert_eq!(dat.data_version, 5015);
    assert_eq!(dat.pos, [12.5, 64.0, -7.25]);
    assert_eq!(dat.rotation, [90.0, -12.5]);
    assert_eq!(dat.dimension, "minecraft:overworld");
    assert_eq!(dat.selected_item_slot, 3);
    assert_eq!(
        dat.inventory.iter().map(|entry| entry.slot).collect::<Vec<_>>(),
        [0, 3, 9, 10, 20, 35]
    );
    assert_eq!(dat.inventory[0].stack.item.as_str(), "minecraft:diamond_sword");
    assert_eq!(dat.inventory[2].stack.count.0, 64);
    assert!(dat.inventory[3].stack.components.is_removed(
        mcrs_minecraft_protocol::item::ItemComponentKind::Lore
    ));
    assert_eq!(
        dat.equipment.keys().cloned().collect::<Vec<_>>(),
        ["chest", "offhand"]
    );
    assert_eq!(dat.equipment["chest"].item.as_str(), "minecraft:leather_chestplate");
    let mut rest: Vec<&str> = dat.rest.child_tags.iter().map(|(key, _)| key.as_str()).collect();
    rest.sort_unstable();
    assert_eq!(
        rest,
        [
            "EnderItems",
            "Fire",
            "Motion",
            "OnGround",
            "UUID",
            "XpLevel",
            "abilities",
            "foodLevel",
            "foodSaturationLevel",
        ]
    );
    assert_eq!(dat.rest.get_short("Fire"), Some(-20));

    let other = Uuid::from_u128(0x5678);
    write_player_dat(&world, other, &dat).unwrap();
    let rewritten = std::fs::read(
        world.join("players/data").join(format!("{}.dat", other.hyphenated())),
    )
    .unwrap();

    let original: NbtCompound = from_gzip_bytes(VANILLA).unwrap();
    let ours: NbtCompound = from_gzip_bytes(rewritten.as_slice()).unwrap();
    assert_eq!(
        sorted(&NbtTag::Compound(original.clone())),
        sorted(&NbtTag::Compound(ours.clone()))
    );
    for key in ["Inventory", "equipment", "EnderItems"] {
        assert_eq!(sorted(get(&original, key)), sorted(get(&ours, key)), "{key}");
    }

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
    assert_eq!(written.data_version, WORLD_VERSION);
    assert!(written.inventory.is_empty());
    assert!(!Path::new(&world.join("players/data").join(format!("{}.dat.tmp", uuid.hyphenated()))).exists());
    let _ = std::fs::remove_dir_all(world);
}
