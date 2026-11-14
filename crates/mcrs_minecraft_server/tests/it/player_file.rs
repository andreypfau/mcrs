//! The server holds the item stacks of a vanilla player file and writes them back.

use bevy_ecs::world::World;
use mcrs_minecraft_engine::ability::{PlayerFlying, PlayerGameMode};
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_engine::persisted_registry::CaptureCause;
use mcrs_minecraft_engine::{UnreadablePlayerDat, load_player, write_player};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::world::world_save::WorldSave;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_save::{PlayerDat, read_player_dat, write_player_dat};

use crate::host_app::{label_of, world as dimension_world};
use crate::world_server::{OVERWORLD, WorldServer};

const KEYS: [&str; 4] = ["Inventory", "EnderItems", "SelectedItemSlot", "equipment"];

fn stack(id: &str, count: i32) -> NbtCompound {
    let mut stack = NbtCompound::new();
    stack.put_string("id", id.to_owned());
    stack.put_int("count", count);
    stack
}

fn slot_stack(slot: i8, id: &str, count: i32) -> NbtTag {
    let mut stack = stack(id, count);
    stack.put_byte("Slot", slot);
    NbtTag::Compound(stack)
}

fn vanilla_player() -> PlayerDat {
    let mut dat = PlayerDat::default();
    dat.root.put_list(
        "Inventory",
        vec![
            slot_stack(0, "minecraft:diamond_sword", 1),
            slot_stack(8, "minecraft:torch", 64),
            slot_stack(20, "minecraft:cobblestone", 32),
        ],
    );
    dat.root.put_list(
        "EnderItems",
        vec![
            slot_stack(0, "minecraft:ender_pearl", 16),
            slot_stack(26, "minecraft:emerald", 5),
        ],
    );
    dat.root.put_int("SelectedItemSlot", 8);
    let mut equipment = NbtCompound::new();
    equipment.put_component("offhand", stack("minecraft:shield", 1));
    equipment.put_component("chest", stack("minecraft:iron_chestplate", 1));
    equipment.put_component("saddle", stack("minecraft:saddle", 1));
    dat.root.put_component("equipment", equipment);
    dat
}

#[test]
fn a_vanilla_player_file_with_items_loads_in_the_server_and_writes_back_equal() {
    let dir = std::env::temp_dir().join(format!("mcrs-player-file-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let id = Uuid::from_u128(0xabcd);
    let input = vanilla_player();
    write_player_dat(&dir, id, &input).unwrap();

    let mut app = WorldServer::default().start();
    let label = label_of(&app, OVERWORLD);
    let world: &mut World = dimension_world(&mut app, label);
    world.insert_resource(WorldSave(dir.clone()));
    let player = world
        .spawn((
            GameProfile {
                id,
                username: "vanilla".to_owned(),
                properties: Vec::new(),
            },
            Transform::default(),
            PlayerGameMode::default(),
            PlayerFlying(false),
        ))
        .id();

    load_player(world, player);
    assert!(
        world.get::<UnreadablePlayerDat>(player).is_none(),
        "the load rejected the file"
    );
    write_player(world, player, CaptureCause::Autosave);
    let output = read_player_dat(&dir, id).unwrap().unwrap();

    for key in KEYS {
        assert_eq!(output.get(key), input.get(key), "{key} came back different");
    }
    let _ = std::fs::remove_dir_all(dir);
}
