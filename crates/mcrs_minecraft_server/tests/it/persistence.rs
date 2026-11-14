//! A world edited on one server run is read back by the next one.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::query::With;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use mcrs_engine_store::{DiskStore, VersionedChunk};
use mcrs_minecraft_anvil::piece::{GridTag, MonumentBuildingTag};
use mcrs_minecraft_anvil::{SavedPiece, StructureStart};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block_entity::{ContainerData, GeneratedBlockEntity};
use mcrs_minecraft_chat_server::ChatPlugin;
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_engine::persisted_registry::{
    CaptureCause, PersistedAppExt, PersistedRegistry, PersistedSnapshot,
};
use mcrs_minecraft_engine_item::{ItemPersistencePlugin, Slots};
use mcrs_minecraft_engine_registries::{install_item_table, test_item_table};
use mcrs_minecraft_engine_server::{Placement, ShutdownRequest};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::ChangeDimensionRequest;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_engine_world::world::lifecycle::stage::SectionStage;
use mcrs_minecraft_engine_world::world::storage::block_entity::{BlockEntity, BlockEntityPos};
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_engine_world::world::sub_app::AddDimensionPlugins;
use mcrs_minecraft_game_registries::dimension::Dimensions;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_region_disk::{RegionDisk, region_dir};
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_resource::{ResourceKeyBuf, ResourceLocationBuf};
use mcrs_minecraft_save::{WorldGenSettings, write_world_gen_settings};
use mcrs_minecraft_worldgen_structure::hardcoded::ocean_monument;
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos, SectionPos};

use crate::host_app::{self, PlayServer, label_of, world as dimension_world};
use crate::support::preset;
use crate::world_server::{
    OVERWORLD, WorldServer, block_state, dimension_of, place, spawn_viewer, tick_until,
    wait_for_generation,
};

const EDITED: BlockPos = BlockPos::new(8, 64, 8);
const CHEST: BlockPos = BlockPos::new(9, 64, 8);
const PATIENCE: Duration = Duration::from_secs(120);

fn scratch_world(name: &str) -> PathBuf {
    scratch_world_at(name, 4, preset("minecraft:beta"))
}

fn scratch_world_at(name: &str, seed: i64, dimensions: Dimensions) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mcrs-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    write_world_gen_settings(
        &dir,
        &WorldGenSettings {
            seed,
            dimensions,
            bonus_chest: false,
            generate_structures: true,
            legacy_custom_options: None,
        },
        test_registries(),
    )
    .unwrap();
    dir
}

fn block_at(world: &mut World, dimension: Entity, pos: BlockPos) -> Option<BlockStateId> {
    let index = world.get::<SectionIndex>(dimension)?;
    let section = index.get(SectionPos::from(pos))?;
    if world.get::<SectionStage>(section) != Some(&SectionStage::Loaded) {
        return None;
    }
    let blocks = index.blocks(SectionPos::from(pos))?;
    blocks.0.get_cell(
        pos.x.rem_euclid(16) as usize,
        pos.y.rem_euclid(16) as usize,
        pos.z.rem_euclid(16) as usize,
    )
}

fn viewed(app: &mut App, label: Entity) {
    wait_for_generation(app, label);
    let world = dimension_world(app, label);
    spawn_viewer(world, PlayerSession(1), DVec3::new(8.5, 100.0, 8.5), 2);
}

fn landed(app: &mut App, label: Entity, pos: BlockPos) {
    tick_until(app, label, "the edited column landed", |world| {
        let dimension = dimension_of(world);
        block_at(world, dimension, pos).is_some()
    });
}

fn save(app: &mut App) {
    app.world_mut().write_message(ShutdownRequest);
    let deadline = Instant::now() + PATIENCE;
    while app.should_exit().is_none() {
        assert!(Instant::now() < deadline, "the shutdown never exited");
        host_app::tick(app);
    }
    assert_eq!(
        app.should_exit(),
        Some(bevy_app::AppExit::Success),
        "the shutdown did not exit successfully"
    );
}

fn spawn_chest(app: &mut App, label: Entity) {
    let world = dimension_world(app, label);
    let dimension = dimension_of(world);
    world.spawn((
        BlockEntityPos(CHEST),
        InDimension(dimension),
        BlockEntity(GeneratedBlockEntity::Chest(ContainerData::looted(
            CHEST,
            "minecraft:chests/simple_dungeon".to_owned(),
            7,
        ))),
    ));
    host_app::tick(app);
}

fn chest_is_there(world: &mut World) -> bool {
    let dimension = dimension_of(world);
    world
        .query::<(&BlockEntityPos, &BlockEntity, &InDimension)>()
        .iter(world)
        .any(|(pos, entity, held)| {
            held.0 == dimension
                && pos.0 == CHEST
                && matches!(
                    &entity.0,
                    GeneratedBlockEntity::Chest(data)
                        if data.loot_table.as_deref() == Some("minecraft:chests/simple_dungeon")
                            && data.loot_table_seed == 7
                )
        })
}

#[test]
fn a_world_edited_on_one_run_comes_back_on_the_next() {
    let save_dir = scratch_world("restart");

    let mut first = WorldServer {
        worlds: Placement::Thread,
        save: Some(save_dir.clone()),
        ..WorldServer::default()
    }
    .start();
    let label = label_of(&first, OVERWORLD);
    viewed(&mut first, label);
    landed(&mut first, label, EDITED);

    let glowstone = block_state(&first, "minecraft:glowstone");
    let chest = block_state(&first, "minecraft:chest");
    let session = PlayerSession(1);
    place(&mut first, label, session, EDITED, glowstone);
    place(&mut first, label, session, CHEST, chest);
    tick_until(&mut first, label, "the edits landed", |world| {
        let dimension = dimension_of(world);
        block_at(world, dimension, EDITED) == Some(glowstone)
            && block_at(world, dimension, CHEST) == Some(chest)
    });
    spawn_chest(&mut first, label);
    save(&mut first);
    drop(first);

    let region = save_dir.join("dimensions/minecraft/overworld/region");
    let region = std::fs::read_dir(&region)
        .expect("the overworld holds a region folder")
        .filter_map(Result::ok)
        .find(|entry| entry.path().extension().is_some_and(|ext| ext == "mca"));
    assert!(region.is_some(), "the overworld holds no region file");

    let mut second = WorldServer {
        worlds: Placement::Thread,
        save: Some(save_dir.clone()),
        ..WorldServer::default()
    }
    .start();
    let label = label_of(&second, OVERWORLD);
    viewed(&mut second, label);
    landed(&mut second, label, EDITED);
    let world = dimension_world(&mut second, label);
    let dimension = dimension_of(world);
    assert_eq!(
        block_at(world, dimension, EDITED),
        Some(glowstone),
        "the edited block came back as something else"
    );
    assert_eq!(
        block_at(world, dimension, CHEST),
        Some(chest),
        "the chest block came back as something else"
    );
    tick_until(&mut second, label, "the chest came back", chest_is_there);

    drop(second);
    let _ = std::fs::remove_dir_all(save_dir);
}

#[cfg(unix)]
#[test]
fn a_player_file_that_cannot_be_written_fails_the_shutdown() {
    use std::os::unix::fs::PermissionsExt;

    let save_dir = scratch_world("player-write");
    let players = save_dir.join("players/data");
    std::fs::create_dir_all(&players).unwrap();
    std::fs::set_permissions(&players, std::fs::Permissions::from_mode(0o555)).unwrap();
    let probe = players.join("probe");
    if std::fs::write(&probe, b"").is_ok() {
        let _ = std::fs::remove_file(probe);
        std::fs::set_permissions(&players, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _ = std::fs::remove_dir_all(save_dir);
        return;
    }

    let mut app = WorldServer {
        worlds: Placement::Thread,
        save: Some(save_dir.clone()),
        ..WorldServer::default()
    }
    .start();
    let label = label_of(&app, OVERWORLD);
    viewed(&mut app, label);
    let world = dimension_world(&mut app, label);
    let player = world
        .query_filtered::<Entity, With<Player>>()
        .single(world)
        .unwrap();
    world.entity_mut(player).insert(GameProfile {
        id: Uuid::new_v4(),
        username: "walker".into(),
        properties: vec![],
    });

    app.world_mut().write_message(ShutdownRequest);
    let deadline = Instant::now() + PATIENCE;
    while app.should_exit().is_none() {
        assert!(Instant::now() < deadline, "the shutdown never exited");
        host_app::tick(&mut app);
    }
    assert_eq!(app.should_exit(), Some(bevy_app::AppExit::error()));

    std::fs::set_permissions(&players, std::fs::Permissions::from_mode(0o755)).unwrap();
    drop(app);
    let _ = std::fs::remove_dir_all(save_dir);
}

fn slot_stack(slot: i8, id: &str, count: i32) -> NbtTag {
    let mut stack = NbtCompound::new();
    stack.put_string("id", id.to_owned());
    stack.put_int("count", count);
    stack.put_byte("Slot", slot);
    NbtTag::Compound(stack)
}

#[test]
fn an_inventory_crosses_a_dimension_change() {
    let mut app = host_app::make_host_app();
    app.add_persistence_plugin(ItemPersistencePlugin);
    app.add_dimension_plugin(ChatPlugin);
    install_item_table(app.world_mut(), test_item_table()).unwrap();
    let mut server = PlayServer::start_in(
        app,
        &[
            ("minecraft:overworld", "minecraft:overworld"),
            ("minecraft:the_end", "minecraft:the_end"),
        ],
        &[],
        0,
    );
    let (anchor, _) = server.join(Uuid::new_v4(), "Walker");
    let overworld = server.dim;
    let end = label_of(&server.app, "minecraft:the_end");

    let items = vec![
        slot_stack(0, "minecraft:diamond_sword", 1),
        slot_stack(20, "minecraft:cobblestone", 32),
    ];
    let player = server.player(anchor);
    let world = server.world_in(overworld);
    let mut given = PersistedSnapshot::new();
    given.insert("Inventory", NbtTag::List(items.clone()));
    let registry = world.remove_resource::<PersistedRegistry>().unwrap();
    registry.apply(world, player, &given).unwrap();
    world.insert_resource(registry);
    assert_eq!(world.get::<Slots>(player).unwrap().occupied().count(), 2);

    let transform = *world.get::<Transform>(player).unwrap();
    world
        .resource_mut::<Messages<ChangeDimensionRequest>>()
        .write(ChangeDimensionRequest {
            player,
            to: ResourceKeyBuf::from_location(
                ResourceLocationBuf::read("minecraft:the_end").unwrap(),
            ),
            transform,
        });
    server.ticks_all(3);

    let arrived = server
        .player_in(end, anchor)
        .expect("the player is in the End");
    let world = server.world_in(end);
    let registry = world.remove_resource::<PersistedRegistry>().unwrap();
    let saved = registry
        .capture_file(world, arrived, CaptureCause::Autosave)
        .unwrap();
    world.insert_resource(registry);
    assert_eq!(saved.get("Inventory"), Some(&NbtTag::List(items)));
}

const SEED: i64 = 4;

fn overworld_disk(app: &App, save_dir: &std::path::Path) -> RegionDisk {
    let key = ResourceKeyBuf::from_location(ResourceLocationBuf::read(OVERWORLD).unwrap());
    RegionDisk::new(
        region_dir(save_dir, &key).unwrap(),
        app.world().resource::<Blocks>().clone(),
        (**app.world().resource::<CurrentRegistries>()).clone(),
    )
}

fn monument_tag(disk: &mut RegionDisk, column: ColumnPos) -> MonumentBuildingTag {
    let stored = disk.read(column).unwrap().expect("the column is saved");
    let start =
        &stored.chunk.structures.starts[&ResourceLocationBuf::minecraft("monument").unwrap()];
    match &start.pieces[..] {
        [SavedPiece::OceanMonumentBuilding(tag)] => tag.clone(),
        other => panic!("the start holds {other:?}"),
    }
}

#[test]
fn a_saved_monument_without_rooms_is_saved_again_with_its_rebuilt_room_graph() {
    let save_dir = scratch_world_at("monument-rooms", SEED, preset("minecraft:beta"));
    let column = ColumnPos::new(EDITED.x >> 4, EDITED.z >> 4);
    let generated = ocean_monument::rebuild(SEED, column).expect("a monument builds at the column");

    let mut first = WorldServer {
        worlds: Placement::Thread,
        save: Some(save_dir.clone()),
        ..WorldServer::default()
    }
    .start();
    let label = label_of(&first, OVERWORLD);
    viewed(&mut first, label);
    landed(&mut first, label, EDITED);
    save(&mut first);

    let mut disk = overworld_disk(&first, &save_dir);
    drop(first);
    let mut stored = disk.read(column).unwrap().expect("the column is saved");
    stored.chunk.structures.starts.insert(
        ResourceLocationBuf::minecraft("monument").unwrap(),
        StructureStart {
            references: 0,
            pieces: vec![SavedPiece::OceanMonumentBuilding(MonumentBuildingTag {
                header: GridTag {
                    bounds: [
                        generated.bounds.min.x,
                        generated.bounds.min.y,
                        generated.bounds.min.z,
                        generated.bounds.max.x,
                        generated.bounds.max.y,
                        generated.bounds.max.z,
                    ],
                    orientation: generated.orientation.data_2d(),
                    gen_depth: 0,
                },
                rooms: vec![],
                children: vec![],
            })],
        },
    );
    disk.write(
        column,
        VersionedChunk {
            version: Default::default(),
            chunk: stored.chunk,
        },
    );
    disk.flush().unwrap();
    drop(disk);

    let mut second = WorldServer {
        worlds: Placement::Thread,
        save: Some(save_dir.clone()),
        ..WorldServer::default()
    }
    .start();
    let label = label_of(&second, OVERWORLD);
    viewed(&mut second, label);
    landed(&mut second, label, EDITED);
    save(&mut second);

    let mut disk = overworld_disk(&second, &save_dir);
    drop(second);
    let saved = monument_tag(&mut disk, column);
    assert_eq!(saved.rooms.len(), generated.rooms.len());
    assert_eq!(saved.children.len(), generated.children.len());
    assert!(!saved.rooms.is_empty() && !saved.children.is_empty());
    for (saved, generated) in saved.rooms.iter().zip(&generated.rooms) {
        assert_eq!(saved.index, generated.index);
        assert_eq!(saved.openings, generated.has_opening.map(i8::from));
    }
    let _ = std::fs::remove_dir_all(save_dir);
}
