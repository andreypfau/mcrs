//! A block entity is an entity under its section, and the typed state it
//! holds is the one type the save, the anvil reader and the chunk packet share.

use std::borrow::Cow;

use bevy_app::{App, FixedUpdate};
use bevy_ecs::prelude::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_anvil::{DATA_VERSION, PaletteLookup, Properties, parse_chunk};
use mcrs_minecraft_decoration::block_entity::{BeeOccupant, EndGatewayData, GeneratedBlockEntity};
use mcrs_minecraft_nbt::Nbt;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::chunk::{ChunkData, ChunkDataBlockEntity};
use mcrs_minecraft_protocol::{Decode, Encode};
use mcrs_minecraft_server::world::block_entity::{BlockEntity, packet_entry, spawn_block_entities};
use mcrs_minecraft_server::world::format::anvil::saved_block_entities;
use mcrs_voxel_math::SectionPos;
use mcrs_voxel_world::world::dimension::InDimension;
use mcrs_voxel_world::world::lifecycle::markers::ChunkUnloaded;
use mcrs_voxel_world::world::lifecycle::ticket::{ChunkTicketsCommands, TicketPlugin};
use mcrs_voxel_world::world::storage::block_entity::{
    InSection, SectionBlockEntities, reconcile_block_entities,
};
use mcrs_voxel_world::world::storage::chunk::ChunkIndex;

fn section_pos() -> SectionPos {
    SectionPos::new(2, 4, -1)
}

/// What `BeehiveDecorator` writes: two or three occupants, each with the hive's
/// minimum stay.
fn generated_nest() -> GeneratedBlockEntity {
    GeneratedBlockEntity::Beehive {
        x: 35,
        y: 71,
        z: -14,
        bees: vec![BeeOccupant::bee(128), BeeOccupant::bee(401)],
    }
}

/// Every kind a generator leaves, each in `section_pos`, with the wire type id the
/// built-in registry gives it.
fn every_kind() -> Vec<(GeneratedBlockEntity, i32)> {
    vec![
        (generated_nest(), 33),
        (
            GeneratedBlockEntity::chest(
                mcrs_voxel_math::BlockPos::new(33, 64, -3),
                "minecraft:chests/simple_dungeon".to_owned(),
                -8_123_456_789,
            ),
            1,
        ),
        (
            GeneratedBlockEntity::mob_spawner(
                mcrs_voxel_math::BlockPos::new(46, 79, -16),
                "minecraft:skeleton",
            ),
            9,
        ),
        (
            GeneratedBlockEntity::EndGateway(EndGatewayData {
                x: 40,
                y: 70,
                z: -9,
                age: 0,
                exit_portal: Some([100, 50, -100]),
                exact_teleport: true,
            }),
            22,
        ),
    ]
}

/// A region file the save would hold, carrying these block entity compounds
/// and nothing else, read back through the reader a loading dimension uses.
/// The column carries no sections, so no palette entry is ever asked for.
struct NoPalette;

impl<V> PaletteLookup<V> for NoPalette {
    fn resolve(&self, _name: &str, _properties: Properties<'_>) -> Option<V> {
        None
    }
}

fn saved_column(block_entities: Vec<NbtCompound>) -> mcrs_minecraft_anvil::Chunk {
    let mut root = NbtCompound::new();
    root.put_int("DataVersion", DATA_VERSION);
    root.put_int("xPos", 2);
    root.put_int("zPos", -1);
    root.put_int("yPos", -4);
    root.put_string("Status", "minecraft:full".to_string());
    root.put_list(
        "block_entities",
        block_entities
            .into_iter()
            .map(NbtTag::Compound)
            .collect::<Vec<_>>(),
    );
    let bytes = Nbt::new(String::new(), root).write();
    parse_chunk(&bytes, &NoPalette, &NoPalette).expect("the saved column parses")
}

fn read_back_through_anvil(entries: &[GeneratedBlockEntity]) -> Vec<GeneratedBlockEntity> {
    let compounds = entries
        .iter()
        .map(|entry| mcrs_minecraft_nbt::to_nbt_compound(entry).expect("the entry serialises"))
        .collect();
    saved_block_entities(&saved_column(compounds))
        .expect("every entry names a kind this build reads")
}

/// A jukebox is a kind this build does not read and is dropped; a beehive it
/// does read but cannot parse is an error, so a generated entity never goes
/// missing without a word.
#[test]
fn an_unknown_kind_is_dropped_and_a_broken_known_kind_is_an_error() {
    let mut jukebox = NbtCompound::new();
    jukebox.put_string("id", "minecraft:jukebox".to_string());
    let read = saved_block_entities(&saved_column(vec![jukebox])).expect("a jukebox is skipped");
    assert!(read.is_empty());

    let mut broken = NbtCompound::new();
    broken.put_string("id", "minecraft:beehive".to_string());
    broken.put_string("x", "not a number".to_string());
    assert!(saved_block_entities(&saved_column(vec![broken])).is_err());

    for entry in every_kind().iter().map(|(entry, _)| entry) {
        let compound = mcrs_minecraft_nbt::to_nbt_compound(entry).expect("serialises");
        let id = compound.get_string("id").expect("tagged by its id");
        assert!(
            GeneratedBlockEntity::IDS.contains(&id),
            "{id} is not in IDS"
        );
    }
}

/// What the section's forward index says one held entity is, read back out of
/// its components rather than out of anything the spawn kept.
fn wire_form(world: &World, entity: Entity) -> GeneratedBlockEntity {
    world.get::<BlockEntity>(entity).expect("a kind").0.clone()
}

fn app_with_section(section_pos: SectionPos) -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins(TicketPlugin);
    app.add_systems(FixedUpdate, reconcile_block_entities);
    let dim = app
        .world_mut()
        .spawn((ChunkIndex::default(), ChunkTicketsCommands::default()))
        .id();
    let section = app
        .world_mut()
        .spawn((
            section_pos,
            InDimension(dim),
            SectionBlockEntities::default(),
        ))
        .id();
    app.world_mut()
        .get_mut::<ChunkIndex>(dim)
        .unwrap()
        .insert(section_pos, section);
    (app, dim, section)
}

#[test]
fn every_generated_kind_survives_a_save_load_round_trip_and_reaches_a_client() {
    let kinds = every_kind();
    let generated: Vec<GeneratedBlockEntity> =
        kinds.iter().map(|(entry, _)| entry.clone()).collect();

    let loaded = read_back_through_anvil(&generated);
    assert_eq!(
        loaded, generated,
        "the save form the generator wrote is what the anvil reader hands back"
    );

    let (mut app, dim, section) = app_with_section(section_pos());
    let mut commands = app.world_mut().commands();
    spawn_block_entities(&mut commands, InDimension(dim), loaded);
    app.world_mut().flush();
    app.world_mut().run_schedule(FixedUpdate);

    let held = app
        .world()
        .get::<SectionBlockEntities>(section)
        .expect("the section keeps its index")
        .to_vec();
    assert_eq!(
        held.len(),
        kinds.len(),
        "one entity per kind, under the section they sit in"
    );

    let mut entries: Vec<ChunkDataBlockEntity<'static>> = Vec::new();
    for entity in &held {
        assert_eq!(
            app.world().get::<InSection>(*entity).map(|link| link.0),
            Some(section),
            "and it links back to that section"
        );
        entries
            .push(packet_entry(&wire_form(app.world(), *entity)).expect("the wire entry encodes"));
    }

    let mut bytes = Vec::new();
    ChunkData {
        heightmaps: Vec::new(),
        data: &[],
        block_entities: Cow::Owned(entries),
    }
    .encode(&mut bytes)
    .expect("the chunk packet encodes");
    let decoded = ChunkData::decode(&mut bytes.as_slice()).expect("and decodes");

    assert_eq!(decoded.block_entities.len(), kinds.len());
    for entry in decoded.block_entities.iter() {
        let ChunkDataBlockEntity {
            packed_xz,
            y,
            kind,
            data,
        } = entry;
        let read =
            GeneratedBlockEntity::from_compound(data).expect("the client reads the same type back");
        let (expected, expected_kind) = kinds
            .iter()
            .find(|(candidate, _)| candidate.position() == read.position())
            .expect("an entry the client did not receive");
        assert_eq!(&read, expected, "the state reaches the client");
        assert_eq!(kind.0, *expected_kind, "named by its registry index");
        let pos = expected.position();
        assert_eq!(*packed_xz, (((pos.x & 15) << 4) | (pos.z & 15)) as i8);
        assert_eq!(*y as i32, pos.y);
    }
}

#[test]
fn despawning_a_section_leaves_no_orphan_block_entity() {
    let (mut app, dim, section) = app_with_section(section_pos());
    let entries: Vec<GeneratedBlockEntity> =
        every_kind().into_iter().map(|(entry, _)| entry).collect();
    let count = entries.len();
    let mut commands = app.world_mut().commands();
    spawn_block_entities(&mut commands, InDimension(dim), entries);
    app.world_mut().flush();
    app.world_mut().run_schedule(FixedUpdate);

    let held = app
        .world()
        .get::<SectionBlockEntities>(section)
        .unwrap()
        .to_vec();
    assert_eq!(held.len(), count);

    app.world_mut().entity_mut(section).insert(ChunkUnloaded);
    app.world_mut().run_schedule(FixedUpdate);

    assert!(
        app.world().get_entity(section).is_err(),
        "the section is gone"
    );
    for entity in held {
        assert!(
            app.world().get_entity(entity).is_err(),
            "and a custom link cascades nothing, so the despawn had to take it through the index"
        );
    }
}

/// `BeehiveBlockEntity.Occupant.CODEC` requires `entity_data`, and a `bees`
/// element without it fails the list codec outright, so a vanilla reader loads
/// the hive with no bees at all.
#[test]
fn a_saved_occupant_names_the_entity_it_holds() {
    let compound = mcrs_minecraft_nbt::to_nbt_compound(&generated_nest()).expect("the hive writes");
    let NbtTag::List(bees) = compound.get("bees").expect("the hive carries its bees") else {
        panic!("bees is a list");
    };
    assert_eq!(bees.len(), 2);
    for bee in bees {
        let NbtTag::Compound(bee) = bee else {
            panic!("an occupant is a compound")
        };
        let NbtTag::Compound(entity_data) = bee.get("entity_data").expect("entity_data is written")
        else {
            panic!("entity_data is a compound")
        };
        assert_eq!(
            entity_data.get("id"),
            Some(&NbtTag::String("minecraft:bee".to_owned()))
        );
    }
}
