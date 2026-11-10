//! Parity with the vanilla frames of the scene `pig_walk` (`tools/scenes/pig_walk.json`).
//!
//! The record holds the state of the drawn pig at three frames, not the packets that the vanilla
//! client received, so the interpolated position cannot be replayed exactly. The test replays
//! the scene with the packets that its commands imply and checks what the record pins down
//! without them: the ticks of a frozen world, the body and head of a pig that walks straight,
//! and the walk accumulator against the distance that the pig covered.

use glam::DVec3;
use mcrs_minecraft_client_entity::FrameValues;
use mcrs_minecraft_client_entity::walk::{self, Walk};
use mcrs_minecraft_core::Look;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_fixtures::fixture;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::PositionFlags;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundTeleportEntity, ClientboundTickingState, ClientboundTickingStep,
};
use mcrs_minecraft_protocol_game::entity::Pose;
use serde::Deserialize;

use crate::{Client, add_entity};

const PIG: i32 = 1;
const START: DVec3 = DVec3::new(0.5, -60.0, -2.0);
const STEP: f64 = 0.2;
const PIG_WALK_TARGET_FACTOR: f32 = 4.0;
const ANGLE_TOLERANCE: f32 = 1e-4;
const WALK_TOLERANCE: f32 = 1e-5;

#[derive(Deserialize)]
struct Record {
    frames: Vec<RecordedFrame>,
}

#[derive(Deserialize)]
struct RecordedFrame {
    entities: Vec<RecordedPig>,
}

#[derive(Deserialize)]
struct RecordedPig {
    partial_tick: f32,
    position: [f64; 3],
    age: u32,
    body_yaw: f32,
    head_yaw: f32,
    head_pitch: f32,
    walk_position: f32,
    walk_speed: f32,
    hurt_time: u32,
    death_time: f32,
    baby: bool,
    saddled: bool,
    pose: String,
    invisible: bool,
}

impl RecordedPig {
    fn position(&self) -> DVec3 {
        DVec3::from_array(self.position)
    }
}

fn recorded_pigs() -> Vec<RecordedPig> {
    let text = fixture("client_frames")
        .file("pig_walk.records.json")
        .text();
    let record: Record = serde_json::from_str(&text).expect("the record has the frame layout");
    record
        .frames
        .into_iter()
        .flat_map(|frame| frame.entities)
        .collect()
}

/// `0.6 · speed + 0.4 · position` of a walk that starts at rest grows by `0.4 · target` in each
/// tick, so while no target reaches 1 it is `1.6` times the distance covered, whatever the
/// distance of each tick.
fn walk_invariant(position: f32, speed: f32) -> f32 {
    0.6 * speed + 0.4 * position
}

#[test]
fn the_recorded_walk_matches_the_accumulator_for_the_distance_covered() {
    let pigs = recorded_pigs();
    assert_eq!(pigs.len(), 3, "the scene draws the pig in three frames");
    for pig in &pigs {
        let distance = pig.position().distance(START) as f32;
        let mut walk = Walk::default();
        for _ in 0..pig.age {
            let target = walk::target_speed(distance / pig.age as f32, PIG_WALK_TARGET_FACTOR);
            walk.advance(target, 1.0);
        }
        let ours = walk_invariant(walk.position, walk.speed);
        let recorded = walk_invariant(pig.walk_position, pig.walk_speed);
        assert!(
            (ours - recorded).abs() < WALK_TOLERANCE,
            "age {}: {ours} != {recorded}",
            pig.age
        );
    }
}

fn assert_angle(actual: f32, expected: f32, what: &str, age: u32) {
    assert!(
        (actual - expected).abs() < ANGLE_TOLERANCE,
        "{what} at age {age}: {actual} != {expected}"
    );
}

#[test]
fn a_replay_of_the_scene_ticks_and_turns_the_pig_as_the_record() {
    let pigs = recorded_pigs();
    let mut client = Client::new();
    client.receive(ClientboundTickingState {
        tick_rate: 20.0,
        frozen: true,
    });
    client.receive(add_entity(PIG, EntityType::Pig, START, 0.0, 0.0));
    let mut frames: Vec<FrameValues> = Vec::new();
    for step in 1..=8 {
        client.receive(ClientboundTeleportEntity {
            entity_id: VarInt(PIG),
            position: START + DVec3::new(0.0, 0.0, STEP * f64::from(step)),
            velocity: DVec3::ZERO,
            look: Look::default(),
            relatives: PositionFlags::default(),
            on_ground: true,
        });
        // The scene waits 5 frozen ticks; none of them ticks the pig.
        for _ in 0..5 {
            client.tick();
        }
        client.receive(ClientboundTickingStep { steps: VarInt(1) });
        client.tick();
        let partial_tick = client.ticking().partial_tick(0.0);
        frames.push(client.inputs(PIG, |inputs| inputs.frame(partial_tick)));
    }

    for pig in &pigs {
        let frame = frames
            .iter()
            .find(|frame| frame.clock - frame.partial_tick == pig.age as f32)
            .unwrap_or_else(|| panic!("the replay reaches age {}", pig.age));
        assert_eq!(
            frame.partial_tick, pig.partial_tick,
            "a frozen pig draws its latest tick"
        );
        assert_angle(frame.body_yaw, pig.body_yaw, "body yaw", pig.age);
        assert_angle(frame.head_yaw, pig.head_yaw, "head yaw", pig.age);
        assert_angle(frame.pitch, pig.head_pitch, "head pitch", pig.age);
        assert_eq!(frame.hurt_time, pig.hurt_time);
        assert_eq!(frame.death_ticks, pig.death_time);
        assert_eq!(frame.baby, pig.baby);
        assert_eq!(frame.saddled, pig.saddled);
        assert_eq!(frame.invisible, pig.invisible);
        assert_eq!(frame.pose, Pose::Standing);
        assert_eq!(pig.pose, "standing");
    }
}
