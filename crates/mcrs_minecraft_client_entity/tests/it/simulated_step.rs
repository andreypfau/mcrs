use glam::DVec3;
use mcrs_minecraft_client_entity::NetworkEntities;
use mcrs_minecraft_client_world::columns::{Column, ColumnChange, ColumnStore, Extent};
use mcrs_minecraft_core::Look;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_registries::test_corpus;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_visual::Age;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_item::ProtoStack;
use mcrs_minecraft_protocol::{Decode, VarInt};
use mcrs_minecraft_protocol_game::LpVec3;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundEntityPositionSync, ClientboundSetEntityData, PositionPath,
};
use mcrs_minecraft_protocol_game::entity::{MetaDataValue, Metadata, MetadataEntry};
use mcrs_minecraft_registries::{CurrentRegistries, Wire};
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos};

use crate::{Client, add_entity};

const ITEM: i32 = 1;
const FALLING_BLOCK: i32 = 2;
const TYPE_ENTRY: u8 = 8;
const NO_GRAVITY_ENTRY: u8 = 5;
const FLOOR: BlockPos = BlockPos::new(8, 0, 8);
const SPAWN_Y: f64 = 10.0;
const MIN_Y: f64 = -64.0;

fn state(name: &str) -> BlockStateId {
    test_corpus()
        .0
        .default_state(name)
        .expect("the corpus has the block")
}

fn client_over_a_floor() -> Client {
    let mut store = ColumnStore::default();
    store.enter(Extent {
        min_section_y: -4,
        sections: 8,
    });
    store.insert(ColumnPos::new(0, 0), Column::unlit(-4, vec![None; 8]));
    store.set_block(FLOOR, state("minecraft:stone"));
    let mut client = Client::new();
    client.app.insert_resource(store);
    client.app.insert_resource(test_corpus().0.clone());
    client
        .app
        .insert_resource(CurrentRegistries::new(test_registries().clone()));
    client
}

fn stack(count: u8) -> ProtoStack<Wire> {
    ProtoStack::decode(&mut &[count, 1, 0, 0][..]).expect("the stack decodes")
}

fn set_type_entry(client: &mut Client, id: i32, value: MetaDataValue<'static>) {
    set_entry(client, id, TYPE_ENTRY, value);
}

fn set_entry(client: &mut Client, id: i32, index: u8, value: MetaDataValue<'static>) {
    client.receive(ClientboundSetEntityData {
        entity_id: VarInt(id),
        metadata: Metadata(vec![MetadataEntry { index, value }]),
    });
}

fn add(client: &mut Client, id: i32, kind: EntityType, y: f64, data: i32) {
    let mut packet = add_entity(id, kind, DVec3::new(8.5, y, 8.5), 0.0, 0.0);
    packet.data = VarInt(data);
    client.receive(packet);
}

fn add_at(client: &mut Client, id: i32, kind: EntityType, position: DVec3, velocity: DVec3) {
    let mut packet = add_entity(id, kind, position, 0.0, 0.0);
    packet.movement = LpVec3(velocity);
    client.receive(packet);
}

fn motion(client: &Client, id: i32) -> MotionState {
    let entity = client.entity(id).expect("the entity was added");
    *client
        .app
        .world()
        .get::<MotionState>(entity)
        .expect("a simulated entity has a motion")
}

fn ticks(client: &mut Client, count: u32) {
    for _ in 0..count {
        client.tick();
    }
}

fn item_resting_on_the_floor() -> Client {
    let mut client = client_over_a_floor();
    add(&mut client, ITEM, EntityType::Item, SPAWN_Y, 0);
    set_type_entry(&mut client, ITEM, MetaDataValue::Slot(stack(1)));
    ticks(&mut client, 100);
    client
}

#[test]
fn an_item_and_a_falling_block_fall_between_updates_and_rest_on_the_floor() {
    const GRAVITY: f64 = 0.04;
    let drag = f64::from(0.98_f32);
    let sand = i32::from(state("minecraft:sand").number());
    let mut client = client_over_a_floor();
    add(&mut client, ITEM, EntityType::Item, SPAWN_Y, 0);
    set_type_entry(&mut client, ITEM, MetaDataValue::Slot(stack(1)));
    add(
        &mut client,
        FALLING_BLOCK,
        EntityType::FallingBlock,
        SPAWN_Y,
        sand,
    );

    let v1 = -GRAVITY;
    let v2 = v1 * drag - GRAVITY;
    let v3 = v2 * drag - GRAVITY;
    let expected = [SPAWN_Y + v1, SPAWN_Y + v1 + v2, SPAWN_Y + v1 + v2 + v3];
    for (tick, y) in expected.into_iter().enumerate() {
        client.tick();
        for id in [ITEM, FALLING_BLOCK] {
            let body = motion(&client, id).0.body;
            assert!(
                (body.position.y - y).abs() < 1e-12,
                "{id}: tick {}: y {} is not {y}",
                tick + 1,
                body.position.y
            );
            assert!(
                (body.velocity.y - [v1, v2, v3][tick] * drag).abs() < 1e-12,
                "{id}: tick {}: the velocity after the drag",
                tick + 1
            );
        }
    }

    ticks(&mut client, 100);
    for id in [ITEM, FALLING_BLOCK] {
        let body = motion(&client, id).0.body;
        assert_eq!(
            body.position,
            DVec3::new(8.5, 1.0, 8.5),
            "{id} rests on the floor"
        );
        assert!(body.flags.on_ground, "{id} is on the ground");
    }
}

struct Removal {
    name: &'static str,
    kind: EntityType,
    y: f64,
    data: i32,
    entry: Option<MetaDataValue<'static>>,
    kept_for: u32,
}

#[test]
fn the_client_removes_a_spent_entity_in_the_tick_that_its_type_ends_it() {
    let below_world = MIN_Y - 64.0 + 0.1;
    let cases = [
        Removal {
            name: "a later fuse replaces the count, then the TNT burns out",
            kind: EntityType::Tnt,
            y: SPAWN_Y,
            data: 0,
            entry: Some(MetaDataValue::VarInt(VarInt(2))),
            kept_for: 1,
        },
        Removal {
            name: "an item with an empty stack",
            kind: EntityType::Item,
            y: SPAWN_Y,
            data: 0,
            entry: Some(MetaDataValue::Slot(stack(0))),
            kept_for: 0,
        },
        Removal {
            name: "an item that starts its third step below the world",
            kind: EntityType::Item,
            y: below_world,
            data: 0,
            entry: Some(MetaDataValue::Slot(stack(1))),
            kept_for: 2,
        },
        Removal {
            name: "a falling block of air",
            kind: EntityType::FallingBlock,
            y: SPAWN_Y,
            data: 0,
            entry: None,
            kept_for: 0,
        },
        Removal {
            name: "a falling block whose data names no block state",
            kind: EntityType::FallingBlock,
            y: SPAWN_Y,
            data: i32::MAX,
            entry: None,
            kept_for: 0,
        },
        Removal {
            name: "an orb skips its first step, then starts its fourth step below the world",
            kind: EntityType::ExperienceOrb,
            y: below_world,
            data: 0,
            entry: None,
            kept_for: 4,
        },
    ];
    for case in cases {
        let name = case.name;
        let mut client = client_over_a_floor();
        add(&mut client, ITEM, case.kind, case.y, case.data);
        if let Some(entry) = case.entry {
            set_type_entry(&mut client, ITEM, entry);
        }
        ticks(&mut client, case.kept_for);
        let entity = client.entity(ITEM);
        assert!(
            entity.is_some_and(|entity| client.app.world().get_entity(entity).is_ok()),
            "{name}: kept for {} ticks",
            case.kept_for
        );
        client.tick();
        assert_eq!(client.entity(ITEM), None, "{name}: no network id");
        assert!(
            entity.is_some_and(|entity| client.app.world().get_entity(entity).is_err()),
            "{name}: removed"
        );
        assert!(client.app.world().resource::<NetworkEntities>().is_empty());
    }
}

#[test]
fn an_item_without_gravity_stays_in_the_air_until_the_server_gives_it_back() {
    let mut client = client_over_a_floor();
    add(&mut client, ITEM, EntityType::Item, SPAWN_Y, 0);
    set_type_entry(&mut client, ITEM, MetaDataValue::Slot(stack(1)));
    set_entry(
        &mut client,
        ITEM,
        NO_GRAVITY_ENTRY,
        MetaDataValue::Boolean(true),
    );
    let start = motion(&client, ITEM).0.body.position;
    ticks(&mut client, 10);
    assert_eq!(motion(&client, ITEM).0.body.position, start);

    set_entry(
        &mut client,
        ITEM,
        NO_GRAVITY_ENTRY,
        MetaDataValue::Boolean(false),
    );
    ticks(&mut client, 10);
    let fallen = motion(&client, ITEM).0.body.position;
    assert!(fallen.y < start.y - 0.1, "the item falls: {fallen}");
}

#[test]
fn a_resting_item_falls_when_the_block_under_it_goes_away() {
    let mut client = item_resting_on_the_floor();
    assert_eq!(motion(&client, ITEM).0.body.position.y, 1.0);

    let world = client.app.world_mut();
    world
        .resource_mut::<ColumnStore>()
        .set_block(FLOOR, BlockStateId::AIR);
    world.write_message(ColumnChange::Reblocked(FLOOR, BlockStateId::AIR));
    ticks(&mut client, 4);

    let body = motion(&client, ITEM).0.body;
    assert!(
        body.position.y < 1.0,
        "the item falls: y {}",
        body.position.y
    );
    assert!(!body.flags.on_ground);
}

#[test]
fn an_orb_target_moves_with_the_step_of_the_orb() {
    const ORB: i32 = 3;
    const CONTROL: i32 = 4;
    let mut client = client_over_a_floor();
    add_at(
        &mut client,
        ORB,
        EntityType::ExperienceOrb,
        DVec3::new(12.5, SPAWN_Y, 12.5),
        DVec3::ZERO,
    );
    add_at(
        &mut client,
        CONTROL,
        EntityType::ExperienceOrb,
        DVec3::new(4.5, SPAWN_Y, 4.5),
        DVec3::ZERO,
    );
    ticks(&mut client, 4);
    let control_at_update = motion(&client, CONTROL).0.body.position;
    let target = motion(&client, ORB).0.body.position + DVec3::new(0.0, 0.5, 1.0);
    client.receive(ClientboundEntityPositionSync {
        entity_id: VarInt(ORB),
        position: PositionPath::Linear(target),
        look: Look::default(),
        on_ground: false,
    });
    ticks(&mut client, 3);
    let fallen = motion(&client, CONTROL).0.body.position - control_at_update;
    assert!(fallen.y < -0.1, "the orbs fall: {fallen}");
    let orb = motion(&client, ORB).0.body.position;
    assert!(
        orb.distance(target + fallen) < 1e-9,
        "the orb reaches the target moved by its own fall: {orb} != {}",
        target + fallen
    );
}

#[test]
fn an_item_in_a_column_that_is_not_loaded_does_not_tick() {
    let velocity = DVec3::new(0.0, 0.2, 0.1);
    let mut client = client_over_a_floor();
    add_at(
        &mut client,
        ITEM,
        EntityType::Item,
        DVec3::new(24.5, SPAWN_Y, 8.5),
        velocity,
    );
    set_type_entry(&mut client, ITEM, MetaDataValue::Slot(stack(1)));
    let before = motion(&client, ITEM).0.body;
    ticks(&mut client, 5);
    let after = motion(&client, ITEM).0.body;
    assert_eq!(
        (after.position, after.velocity),
        (before.position, before.velocity)
    );
    let entity = client.entity(ITEM).expect("the item was added");
    assert_eq!(client.app.world().get::<Age>(entity), Some(&Age::default()));
}

#[test]
fn an_item_moves_into_a_column_that_is_not_loaded_as_into_air() {
    const CONTROL: i32 = 3;
    let velocity = DVec3::new(0.25, 0.1, 0.0);
    let mut client = client_over_a_floor();
    for (id, x) in [(ITEM, 15.9), (CONTROL, 4.9)] {
        add_at(
            &mut client,
            id,
            EntityType::Item,
            DVec3::new(x, SPAWN_Y, 8.5),
            velocity,
        );
        set_type_entry(&mut client, id, MetaDataValue::Slot(stack(1)));
    }
    let start = [ITEM, CONTROL].map(|id| motion(&client, id).0.body.position);
    client.tick();
    let [edge, control] = [ITEM, CONTROL].map(|id| motion(&client, id).0.body.position);
    assert!(
        edge.x > 16.0,
        "the item enters the column that is not loaded: {edge}"
    );
    let [edge_start, control_start] = start;
    assert!(
        ((edge - edge_start) - (control - control_start)).length() < 1e-9,
        "the item moves as the item in the air: {edge} {control}"
    );
}
