use std::env;
use std::fs;
use std::process;
use std::process::Command;

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::nbt_compress::write_gzip_compound_tag_to_bytes;
use mcrs_minecraft_nbt::tag::NbtTag;

const ITEM_DEFINITIONS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/mcrs/item_definition"
);

fn saved(name: &str, count: i32) -> NbtCompound {
    let mut saved = NbtCompound::new();
    saved.put("id", NbtTag::String(format!("minecraft:{name}")));
    saved.put("count", NbtTag::Int(count));
    saved
}

fn slotted(slot: i8, name: &str, count: i32) -> NbtTag {
    let mut saved = saved(name, count);
    saved.put("Slot", NbtTag::Byte(slot));
    NbtTag::Compound(saved)
}

#[test]
fn inv_sav_001_prints_the_stacks_the_selected_slot_and_the_kept_equipment_keys() {
    let mut equipment = NbtCompound::new();
    equipment.put("head", NbtTag::Compound(saved("diamond_helmet", 1)));
    equipment.put("saddle", NbtTag::Compound(saved("saddle", 1)));
    let mut player = NbtCompound::new();
    player.put(
        "Inventory",
        NbtTag::List(vec![slotted(0, "stone", 3), slotted(20, "dirt", 64)]),
    );
    player.put("equipment", NbtTag::Compound(equipment));
    player.put("SelectedItemSlot", NbtTag::Int(4));
    let path = env::temp_dir().join(format!("mcrs-inventory-save-viewer-{}.dat", process::id()));
    fs::write(&path, write_gzip_compound_tag_to_bytes(&player).unwrap()).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_mcrs_minecraft_inventory_save_viewer"))
        .arg(&path)
        .arg(ITEM_DEFINITIONS)
        .output()
        .unwrap();
    fs::remove_file(&path).unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "slot 0: 3 minecraft:stone\n\
         slot 20: 64 minecraft:dirt\n\
         slot 39: 1 minecraft:diamond_helmet\n\
         selected slot: 4\n\
         kept equipment key: saddle\n",
    );
}
