//! Replays the entity packets that the vanilla client applied in a capture scene, and compares
//! the state of each entity after each tick with the rows that the vanilla client logged.
//!
//! A packet applies before the first row whose update is larger than the updates before the
//! packet: only packets that arrive while no entity ticks can shift between two runs, and those
//! packets cannot change a row.

use std::path::PathBuf;

use glam::DVec3;
use mcrs_minecraft_core::Look;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundAddEntity, ClientboundDamageEvent, ClientboundEntityEvent,
    ClientboundEntityPositionSync, ClientboundMoveEntityPos, ClientboundMoveEntityPosRot,
    ClientboundMoveEntityRot, ClientboundRotateHead, ClientboundSetEntityData,
    ClientboundTickingState, ClientboundTickingStep, DeltaStep, VecDelta, PositionPath,
    PositionStep,
};
use mcrs_minecraft_protocol_game::entity::{MetaDataValue, Metadata, MetadataEntry};
use mcrs_minecraft_protocol_game::{ByteAngle, LpVec3};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::Client;

const POSITION_TOLERANCE: f64 = 1e-6;
const ANGLE_TOLERANCE: f32 = 1e-4;
const WALK_TOLERANCE: f32 = 1e-5;

#[derive(Deserialize)]
struct PacketLog {
    packets: Vec<PacketRecord>,
}

#[derive(Deserialize)]
struct PacketRecord {
    updates_before: u32,
    #[serde(flatten)]
    packet: RecordedPacket,
}

/// A packet that is not in this list fails the parse of the record.
#[derive(Deserialize)]
#[serde(tag = "packet")]
enum RecordedPacket {
    #[serde(rename = "minecraft:ticking_state")]
    TickingState(TickingState),
    #[serde(rename = "minecraft:ticking_step")]
    TickingStep(TickingStep),
    #[serde(rename = "minecraft:add_entity")]
    AddEntity(AddEntity),
    #[serde(rename = "minecraft:set_entity_data")]
    SetEntityData(SetEntityData),
    #[serde(
        rename = "minecraft:move_entity_pos",
        alias = "minecraft:move_entity_pos_rot",
        alias = "minecraft:move_entity_rot"
    )]
    Move(Move),
    #[serde(rename = "minecraft:entity_position_sync")]
    PositionSync(PositionSync),
    #[serde(rename = "minecraft:rotate_head")]
    RotateHead(RotateHead),
    #[serde(rename = "minecraft:damage_event")]
    DamageEvent(EntityOnly),
    #[serde(rename = "minecraft:entity_event")]
    EntityEvent(EntityEvent),
    /// The client does not move an entity of the server with its velocity.
    #[serde(rename = "minecraft:set_entity_motion")]
    SetEntityMotion {},
}

#[derive(Deserialize)]
struct TickLog {
    updates: Vec<UpdateRow>,
}

#[derive(Deserialize)]
struct UpdateRow {
    update: u32,
    entities: Vec<EntityRow>,
}

#[derive(Deserialize)]
struct EntityRow {
    entity: i32,
    age: u32,
    position: [f64; 3],
    previous_position: [f64; 3],
    on_ground: bool,
    path_before_update: bool,
    path_after_update: bool,
    pitch: f32,
    body_yaw: f32,
    previous_body_yaw: f32,
    head_yaw: f32,
    previous_head_yaw: f32,
    walk_position: f32,
    walk_speed: f32,
    hurt_time: u32,
    death_time: u32,
}

#[derive(Deserialize)]
struct AddEntity {
    entity: i32,
    uuid: String,
    #[serde(rename = "type")]
    kind: String,
    position: [f64; 3],
    yaw: f32,
    pitch: f32,
    head_yaw: f32,
    data: i32,
}

#[derive(Deserialize)]
struct EntityOnly {
    entity: i32,
}

#[derive(Deserialize)]
struct EntityEvent {
    entity: i32,
    event: i8,
}

#[derive(Deserialize)]
struct SetEntityData {
    entity: i32,
    values: Vec<DataValue>,
}

/// An entry without `value` is one that the capture does not decode.
#[derive(Deserialize)]
struct DataValue {
    index: u8,
    value: Option<RecordedValue>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(untagged)]
enum RecordedValue {
    Boolean(bool),
    Byte(i8),
    Float(f32),
}

#[derive(Deserialize)]
struct Move {
    entity: i32,
    delta: Option<RecordedDelta>,
    yaw: Option<f32>,
    pitch: Option<f32>,
    on_ground: bool,
}

#[derive(Deserialize)]
#[serde(tag = "form", rename_all = "lowercase")]
enum RecordedDelta {
    Single {
        units: [i16; 3],
    },
    Stepped {
        steps: Vec<RecordedDeltaStep>,
        #[serde(default)]
        start_immediately: bool,
    },
}

#[derive(Deserialize)]
struct RecordedDeltaStep {
    units: [i16; 3],
    ticks: i32,
}

#[derive(Deserialize)]
struct PositionSync {
    entity: i32,
    path: RecordedPath,
    yaw: f32,
    pitch: f32,
    on_ground: bool,
}

#[derive(Deserialize)]
#[serde(tag = "form", rename_all = "lowercase")]
enum RecordedPath {
    Single {
        position: [f64; 3],
    },
    Stepped {
        steps: Vec<RecordedPathStep>,
        #[serde(default)]
        start_immediately: bool,
    },
}

#[derive(Deserialize)]
struct RecordedPathStep {
    position: [f64; 3],
    ticks: i32,
}

#[derive(Deserialize)]
struct RotateHead {
    entity: i32,
    head_yaw: f32,
}

#[derive(Deserialize)]
struct TickingState {
    tick_rate: f32,
    frozen: bool,
}

#[derive(Deserialize)]
struct TickingStep {
    steps: i32,
}

fn read_artefact<T: DeserializeOwned>(file: &str) -> T {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../mcrs_minecraft_client/tests/fixtures/vanilla/frames")
        .join(file);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("reading {}: {error}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn motion_delta(delta: &RecordedDelta) -> VecDelta {
    match delta {
        RecordedDelta::Single { units } => VecDelta::Linear(*units),
        RecordedDelta::Stepped {
            steps,
            start_immediately,
        } => VecDelta::Stepped {
            steps: steps
                .iter()
                .map(|step| DeltaStep {
                    ticks: VarInt(step.ticks),
                    delta: step.units,
                })
                .collect(),
            start_immediately: *start_immediately,
        },
    }
}

fn position_path(path: &RecordedPath) -> PositionPath {
    match path {
        RecordedPath::Single { position } => PositionPath::Linear(DVec3::from_array(*position)),
        RecordedPath::Stepped {
            steps,
            start_immediately,
        } => PositionPath::Stepped {
            steps: steps
                .iter()
                .map(|step| PositionStep {
                    position: DVec3::from_array(step.position),
                    tick_offset: VarInt(step.ticks),
                })
                .collect(),
            start_immediately: *start_immediately,
        },
    }
}

fn data_value(value: RecordedValue) -> MetaDataValue<'static> {
    match value {
        RecordedValue::Boolean(flag) => MetaDataValue::Boolean(flag),
        RecordedValue::Byte(byte) => MetaDataValue::Byte(byte),
        RecordedValue::Float(float) => MetaDataValue::Float(float),
    }
}

fn apply(client: &mut Client, packet: &RecordedPacket) {
    match packet {
        RecordedPacket::TickingState(packet) => client.receive(ClientboundTickingState {
            tick_rate: packet.tick_rate,
            frozen: packet.frozen,
        }),
        RecordedPacket::TickingStep(packet) => client.receive(ClientboundTickingStep {
            steps: VarInt(packet.steps),
        }),
        RecordedPacket::AddEntity(packet) => client.receive(ClientboundAddEntity {
            id: VarInt(packet.entity),
            uuid: Uuid::parse_str(&packet.uuid).expect("the record holds a UUID"),
            kind: EntityType::read(&packet.kind).expect("the record holds an entity type"),
            pos: DVec3::from_array(packet.position),
            movement: LpVec3::default(),
            pitch: ByteAngle::from_degrees(packet.pitch),
            yaw: ByteAngle::from_degrees(packet.yaw),
            head_yaw: ByteAngle::from_degrees(packet.head_yaw),
            data: VarInt(packet.data),
        }),
        RecordedPacket::SetEntityData(packet) => {
            let mut entries = Vec::with_capacity(packet.values.len());
            for entry in &packet.values {
                if let Some(value) = entry.value {
                    entries.push(MetadataEntry {
                        index: entry.index,
                        value: data_value(value),
                    });
                }
            }
            client.receive(ClientboundSetEntityData {
                entity_id: VarInt(packet.entity),
                metadata: Metadata(entries),
            });
        }
        RecordedPacket::Move(packet) => apply_move(client, packet),
        RecordedPacket::PositionSync(packet) => client.receive(ClientboundEntityPositionSync {
            entity_id: VarInt(packet.entity),
            position: position_path(&packet.path),
            look: Look {
                yaw: packet.yaw,
                pitch: packet.pitch,
            },
            on_ground: packet.on_ground,
        }),
        RecordedPacket::RotateHead(packet) => client.receive(ClientboundRotateHead {
            entity_id: VarInt(packet.entity),
            y_head_rot: ByteAngle::from_degrees(packet.head_yaw),
        }),
        RecordedPacket::DamageEvent(packet) => client.receive(ClientboundDamageEvent {
            entity_id: VarInt(packet.entity),
            damage_type: VarInt(0),
            cause_entity_id: VarInt(0),
            direct_entity_id: VarInt(0),
            source_position: None,
        }),
        RecordedPacket::EntityEvent(packet) => client.receive(ClientboundEntityEvent {
            entity_id: packet.entity,
            entity_status: packet.event,
        }),
        RecordedPacket::SetEntityMotion {} => {}
    }
}

fn apply_move(client: &mut Client, packet: &Move) {
    let entity_id = VarInt(packet.entity);
    let rotation = packet.yaw.zip(packet.pitch);
    match (packet.delta.as_ref().map(motion_delta), rotation) {
        (Some(delta), None) => client.receive(ClientboundMoveEntityPos {
            entity_id,
            delta,
            on_ground: packet.on_ground,
        }),
        (Some(delta), Some((yaw, pitch))) => client.receive(ClientboundMoveEntityPosRot {
            entity_id,
            delta,
            y_rot: ByteAngle::from_degrees(yaw),
            x_rot: ByteAngle::from_degrees(pitch),
            on_ground: packet.on_ground,
        }),
        (None, Some((yaw, pitch))) => client.receive(ClientboundMoveEntityRot {
            entity_id,
            y_rot: ByteAngle::from_degrees(yaw),
            x_rot: ByteAngle::from_degrees(pitch),
            on_ground: packet.on_ground,
        }),
        (None, None) => panic!(
            "a move of entity {} with no delta and no rotation",
            packet.entity
        ),
    }
}

fn assert_position(actual: DVec3, expected: [f64; 3], what: &str) {
    let expected = DVec3::from_array(expected);
    assert!(
        (actual - expected).abs().max_element() < POSITION_TOLERANCE,
        "{what}: {actual} != {expected}"
    );
}

fn assert_angle(actual: f32, expected: f32, what: &str) {
    assert!(
        (actual - expected).abs() < ANGLE_TOLERANCE,
        "{what}: {actual} != {expected}"
    );
}

fn assert_walk(actual: f32, expected: f32, what: &str) {
    assert!(
        (actual - expected).abs() < WALK_TOLERANCE,
        "{what}: {actual} != {expected}"
    );
}

fn has_path(client: &mut Client, network_id: i32) -> bool {
    client.inputs(network_id, |inputs| inputs.motion.interpolation.has_path())
}

fn check(client: &mut Client, update: u32, row: &EntityRow, path_before: bool) {
    let context = |what: &str| format!("update {update}, entity {}: {what}", row.entity);
    assert_eq!(
        path_before,
        row.path_before_update,
        "{}",
        context("path before")
    );
    client.inputs(row.entity, |inputs| {
        let motion = inputs.motion;
        let living = inputs.living.expect("a pig is a living entity");
        assert_eq!(inputs.age.0, row.age, "{}", context("age"));
        assert_position(motion.current.position, row.position, &context("position"));
        assert_position(
            motion.previous.position,
            row.previous_position,
            &context("previous position"),
        );
        assert_eq!(motion.on_ground, row.on_ground, "{}", context("on ground"));
        assert_eq!(
            motion.interpolation.has_path(),
            row.path_after_update,
            "{}",
            context("path after")
        );
        assert_angle(motion.current.pitch, row.pitch, &context("pitch"));
        assert_angle(living.body_yaw, row.body_yaw, &context("body yaw"));
        assert_angle(
            living.previous_body_yaw,
            row.previous_body_yaw,
            &context("previous body yaw"),
        );
        assert_angle(living.head_yaw, row.head_yaw, &context("head yaw"));
        assert_angle(
            living.previous_head_yaw,
            row.previous_head_yaw,
            &context("previous head yaw"),
        );
        assert_walk(
            living.walk.position,
            row.walk_position,
            &context("walk position"),
        );
        assert_walk(living.walk.speed, row.walk_speed, &context("walk speed"));
        assert_eq!(living.hurt_time, row.hurt_time, "{}", context("hurt time"));
        assert_eq!(
            living.death_time,
            row.death_time,
            "{}",
            context("death time")
        );
    });
}

fn replay(scene: &str) {
    let packets: PacketLog = read_artefact(&format!("{scene}.packets.json"));
    let ticks: TickLog = read_artefact(&format!("{scene}.entity_ticks.json"));
    let mut client = Client::new();
    let mut pending = packets.packets.iter().peekable();
    for row in &ticks.updates {
        while let Some(record) = pending.next_if(|record| record.updates_before < row.update) {
            apply(&mut client, &record.packet);
        }
        let paths_before: Vec<bool> = row
            .entities
            .iter()
            .map(|entity| has_path(&mut client, entity.entity))
            .collect();
        client.tick();
        for (entity, path_before) in row.entities.iter().zip(paths_before) {
            check(&mut client, row.update, entity, path_before);
        }
    }
    for record in pending {
        apply(&mut client, &record.packet);
    }
}

#[test]
fn pig_walk() {
    replay("pig_walk");
}

#[test]
fn pig_hurt_death() {
    replay("pig_hurt_death");
}

#[test]
fn pig_idle() {
    replay("pig_idle");
}
