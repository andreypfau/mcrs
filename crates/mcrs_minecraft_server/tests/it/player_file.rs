//! The server holds the item stacks of a vanilla player file and writes them back.

use bevy_ecs::world::World;
use mcrs_minecraft_engine::ability::{PlayerFlying, PlayerGameMode};
use mcrs_minecraft_engine::game_profile::GameProfile;
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
    write_player(world, player);
    let output = read_player_dat(&dir, id).unwrap().unwrap();

    for key in KEYS {
        assert_eq!(output.get(key), input.get(key), "{key} came back different");
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the test covers every key the combat codecs own"
)]
fn combat_keys_load_into_components_and_write_back() {
    let dir = std::env::temp_dir().join(format!("mcrs-combat-file-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let id = Uuid::from_u128(0xdcba);
    let mut input = PlayerDat::default();
    input.root.put_float("Health", 7.5);
    input.root.put_short("HurtTime", 3);
    input.root.put_short("DeathTime", 0);
    input.root.put_float("AbsorptionAmount", 2.0);
    input.root.put_short("Fire", 20);
    input.root.put_short("Air", 100);
    input.root.put_int("TicksFrozen", 4);
    input.root.put_double("fall_distance", 2.5);
    input.root.put_byte("OnGround", 0);
    input.root.put_int("foodLevel", 4);
    input.root.put_int("foodTickTimer", 7);
    input.root.put_float("foodSaturationLevel", 1.5);
    input.root.put_float("foodExhaustionLevel", 0.25);
    input.root.put_int("XpLevel", 5);
    input.root.put_int("XpTotal", 100);
    input.root.put_int("Score", 3);
    input.root.put_float("XpP", 0.25);
    let mut attribute = NbtCompound::new();
    attribute.put_string("id", "minecraft:max_health".to_owned());
    attribute.put_double("base", 30.0);
    input
        .root
        .put_list("attributes", vec![NbtTag::Compound(attribute)]);
    let mut hidden = NbtCompound::new();
    hidden.put_byte("amplifier", 0);
    hidden.put_int("duration", 100);
    hidden.put_byte("ambient", 0);
    hidden.put_byte("show_particles", 1);
    hidden.put_byte("show_icon", 1);
    let mut effect = NbtCompound::new();
    effect.put_string("id", "minecraft:speed".to_owned());
    effect.put_byte("amplifier", 1);
    effect.put_int("duration", 200);
    effect.put_byte("ambient", 0);
    effect.put_byte("show_particles", 1);
    effect.put_byte("show_icon", 1);
    effect.put_component("hidden_effect", hidden);
    input
        .root
        .put_list("active_effects", vec![NbtTag::Compound(effect)]);
    let mut respawn = NbtCompound::new();
    respawn.put_string("dimension", "minecraft:overworld".to_owned());
    respawn.put("pos", NbtTag::IntArray(vec![1, 64, 2]));
    respawn.put_float("yaw", 90.0);
    respawn.put_float("pitch", 0.0);
    respawn.put_byte("forced", 1);
    input.root.put_component("respawn", respawn);
    write_player_dat(&dir, id, &input).unwrap();

    let mut app = WorldServer::default().start();
    let label = label_of(&app, OVERWORLD);
    let world: &mut World = dimension_world(&mut app, label);
    world.insert_resource(WorldSave(dir.clone()));
    let player = world
        .spawn((
            GameProfile {
                id,
                username: "combat".to_owned(),
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
    assert_eq!(
        world
            .get::<mcrs_minecraft_combat::PlayerVitals>(player)
            .unwrap()
            .0
            .health,
        7.5
    );
    assert!(
        world
            .get::<mcrs_minecraft_combat::PlayerEffects>(player)
            .unwrap()
            .has(mcrs_minecraft_combat::MobEffect::Speed)
    );
    assert_eq!(
        world
            .get::<mcrs_minecraft_combat::PlayerAttributes>(player)
            .unwrap()
            .max_health_base(),
        30.0
    );
    let point = world
        .get::<mcrs_minecraft_combat::RespawnPoint>(player)
        .expect("the respawn point loads");
    assert_eq!(point.position.translation.x, 1.0);
    assert_eq!(point.position.translation.y, 64.0);
    assert!(point.forced);

    write_player(world, player);
    let output = read_player_dat(&dir, id).unwrap().unwrap();
    for key in [
        "Health",
        "HurtTime",
        "DeathTime",
        "AbsorptionAmount",
        "Fire",
        "Air",
        "TicksFrozen",
        "fall_distance",
        "OnGround",
        "foodLevel",
        "foodTickTimer",
        "foodSaturationLevel",
        "foodExhaustionLevel",
        "XpLevel",
        "XpTotal",
        "Score",
        "XpP",
        "attributes",
        "active_effects",
        "respawn",
    ] {
        assert_eq!(output.get(key), input.get(key), "{key} came back different");
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_missing_health_uses_the_saved_maximum() {
    let dir = std::env::temp_dir().join(format!("mcrs-health-default-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let id = Uuid::from_u128(0x5a1);
    let mut input = PlayerDat::default();
    input.root.put_short("HurtTime", 3);
    let mut attribute = NbtCompound::new();
    attribute.put_string("id", "minecraft:max_health".to_owned());
    attribute.put_double("base", 30.0);
    input
        .root
        .put_list("attributes", vec![NbtTag::Compound(attribute)]);
    write_player_dat(&dir, id, &input).unwrap();

    let mut app = WorldServer::default().start();
    let label = label_of(&app, OVERWORLD);
    let world: &mut World = dimension_world(&mut app, label);
    world.insert_resource(WorldSave(dir.clone()));
    let player = world
        .spawn((
            GameProfile {
                id,
                username: "health".to_owned(),
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
    assert_eq!(
        world
            .get::<mcrs_minecraft_combat::PlayerVitals>(player)
            .unwrap()
            .0
            .health,
        30.0,
        "a save without Health starts at the maximum the attributes give"
    );
    let _ = std::fs::remove_dir_all(dir);
}
