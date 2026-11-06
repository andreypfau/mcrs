//! Vanilla parity of the tick of server-simulated entities: each fixture under
//! `tests/fixtures/physics` holds the states that a vanilla server printed after each tick of one
//! experiment of the physics specification. `tools/physics-fixtures/record.sh` records them.
#![expect(clippy::unwrap_used, clippy::panic, reason = "a malformed fixture fails the test")]

use std::fmt::Write as _;
use std::path::Path;

use glam::DVec3;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_physics::BirthKey;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::BodyView;
use mcrs_minecraft_physics::EntityCollider;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::Living;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::MoveFlags;
use mcrs_minecraft_physics::Scratch;
use mcrs_minecraft_physics::Side;
use mcrs_minecraft_physics::SourceKind;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::TestWorld;
use mcrs_minecraft_physics::Travel;
use mcrs_minecraft_physics::View;
use mcrs_minecraft_physics::WorldBorder;
use mcrs_minecraft_physics::tick_entity;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;
use serde::Deserialize;

mod common;

const TOLERANCE: f64 = 1.0e-6;

#[derive(Deserialize)]
struct Fixture {
    name: String,
    rule: String,
    origin: [i32; 3],
    blocks: Vec<Blocks>,
    entities: Vec<Recorded>,
    border: Option<Border>,
    /// Set for an experiment that the recorder ran two times from the same world: true when the
    /// two runs printed the same states.
    repeat_equal: Option<bool>,
}

#[derive(Deserialize)]
struct Blocks {
    min: [i32; 3],
    max: [i32; 3],
    block: String,
}

#[derive(Deserialize)]
struct Border {
    center: [f64; 2],
    size: f64,
}

#[derive(Deserialize)]
struct Recorded {
    tag: String,
    kind: String,
    nbt: String,
    #[serde(default)]
    attributes: std::collections::BTreeMap<String, f64>,
    #[serde(default)]
    effects: Vec<String>,
    #[serde(default)]
    collider: bool,
    ticks: Vec<Option<Sample>>,
}

#[derive(Deserialize, Clone, Copy)]
struct Sample {
    pos: [f64; 3],
    motion: [f64; 3],
    on_ground: bool,
    fall_distance: f64,
}

/// The entity colliders of a fixture, at their recorded positions.
struct Colliders(Vec<EntityCollider>);

impl BodyView for Colliders {
    fn colliders(&self, query: Aabb, _mover: BirthKey, out: &mut Vec<EntityCollider>) {
        out.extend(
            self.0
                .iter()
                .filter(|found| found.bounds.intersects(&query)),
        );
    }
}

fn state_of(text: &str) -> BlockStateId {
    let definitions = common::definitions();
    let (name, properties) = text.split_once('[').unwrap_or((text, ""));
    let block = definitions
        .block(name)
        .unwrap_or_else(|| panic!("no block {name}"));
    let mut state = block.default_state_id;
    for pair in properties
        .trim_end_matches(']')
        .split(',')
        .filter(|pair| !pair.is_empty())
    {
        let (key, value) = pair.split_once('=').unwrap();
        state = block
            .with_text(state, key, value)
            .unwrap_or_else(|| panic!("{name}: {pair}"));
    }
    state
}

fn world_of(fixture: &Fixture) -> TestWorld {
    let mut world = TestWorld::new();
    let [ox, oy, oz] = fixture.origin;
    for blocks in &fixture.blocks {
        let state = state_of(&blocks.block);
        let at = |offset: [i32; 3]| BlockPos::new(ox + offset[0], oy + offset[1], oz + offset[2]);
        world.fill(at(blocks.min), at(blocks.max), state);
    }
    world.set_border(fixture.border.as_ref().map(|border| {
        let half = border.size / 2.0;
        WorldBorder {
            min_x: border.center[0] - half,
            max_x: border.center[0] + half,
            min_z: border.center[1] - half,
            max_z: border.center[1] + half,
        }
    }));
    world
}

fn row_of(kind: &str) -> Option<MotionRow> {
    Some(match kind {
        "armor_stand" => MotionRow::armor_stand(),
        "item" => MotionRow::item(),
        "falling_block" => MotionRow::falling_block(),
        "tnt" => MotionRow::primed_tnt(),
        "experience_orb" => MotionRow::experience_orb(),
        _ => return None,
    })
}

fn start_state(recorded: &Recorded, row: &MotionRow, sample: &Sample, phase: u8) -> EntityState {
    let body = Body {
        position: DVec3::from_array(sample.pos),
        velocity: DVec3::from_array(sample.motion),
        flags: MoveFlags {
            on_ground: sample.on_ground,
            ..MoveFlags::default()
        },
        fall_distance: sample.fall_distance,
        ..Body::default()
    };
    let no_gravity = recorded.nbt.contains("NoGravity:1b");
    let living = row.category.is_living().then(|| {
        let mut living = Living::of(row);
        for (name, value) in &recorded.attributes {
            let attributes = &mut living.attributes;
            match name.as_str() {
                "step_height" => attributes.step_height = *value,
                "bounciness" => attributes.bounciness = *value,
                "friction_modifier" => attributes.friction_modifier = *value,
                "air_drag_modifier" => attributes.air_drag_modifier = *value,
                "movement_efficiency" => attributes.movement_efficiency = *value,
                other => panic!("no attribute {other}"),
            }
        }
        living.effects.slow_falling = recorded
            .effects
            .iter()
            .any(|effect| effect == "slow_falling");
        if no_gravity {
            living.travel = Travel::Closed;
        }
        living
    });
    let mut state = EntityState::new(BirthKey::new(0, SourceKind::Command, 0, 0), body, living);
    state.no_gravity = no_gravity;
    state.phase = phase;
    state
}

/// Replays one entity; returns the first tick that differs, with a description.
fn replay(
    view: View<'_, TestWorld, Colliders>,
    recorded: &Recorded,
    row: &MotionRow,
    phase: u8,
) -> Option<(usize, String)> {
    let first = recorded.ticks.first().copied().flatten()?;
    let mut state = start_state(recorded, row, &first, phase);
    let mut scratch = Scratch::new();
    let mut actions = Vec::new();
    for (tick, expected) in recorded.ticks.iter().enumerate().skip(1) {
        let Some(expected) = expected else {
            return None;
        };
        actions.clear();
        tick_entity(
            view,
            Side::Server { authority: true },
            row,
            &mut state,
            true,
            &mut actions,
            &mut scratch,
        );
        state.age = state.age.wrapping_add(1);
        let body = &state.body;
        let position = (body.position - DVec3::from_array(expected.pos))
            .abs()
            .max_element();
        let velocity = (body.velocity - DVec3::from_array(expected.motion))
            .abs()
            .max_element();
        let fall = (body.fall_distance - expected.fall_distance).abs();
        if position > TOLERANCE
            || velocity > TOLERANCE
            || fall > TOLERANCE
            || body.flags.on_ground != expected.on_ground
        {
            return Some((
                tick,
                format!(
                    "position {:?} against {:?}, velocity {:?} against {:?}, on ground {} against {}, \
                     fall {} against {}",
                    body.position.to_array(),
                    expected.pos,
                    body.velocity.to_array(),
                    expected.motion,
                    body.flags.on_ground,
                    expected.on_ground,
                    body.fall_distance,
                    expected.fall_distance
                ),
            ));
        }
    }
    None
}

#[test]
fn server_entities_follow_the_vanilla_fixtures() {
    let tables = Tables::new(common::table().clone());
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/physics");
    let mut paths: Vec<_> = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    paths.sort();
    let mut failures = String::new();
    let mut checked = 0;
    for path in paths {
        let fixture: Fixture = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        if fixture.repeat_equal == Some(false) {
            let _ = writeln!(failures, "{}: two vanilla runs differ", fixture.name);
        }
        let world = world_of(&fixture);
        let colliders = Colliders(
            fixture
                .entities
                .iter()
                .filter(|recorded| recorded.collider)
                .filter_map(|recorded| {
                    let sample = recorded.ticks.first().copied().flatten()?;
                    let position = DVec3::from_array(sample.pos);
                    Some(EntityCollider {
                        bounds: Aabb::of_body(position, 1.0, 1.0),
                        id: EntityId(1),
                    })
                })
                .collect(),
        );
        let view = View {
            world: &world,
            bodies: &colliders,
            tables: &tables,
        };
        for recorded in &fixture.entities {
            let Some(row) = row_of(&recorded.kind) else {
                continue;
            };
            checked += 1;
            let phases: &[u8] = if recorded.kind == "item" {
                &[0, 1, 2, 3]
            } else {
                &[0]
            };
            let results: Vec<_> = phases
                .iter()
                .map(|&phase| replay(view, recorded, &row, phase))
                .collect();
            if results.iter().any(Option::is_none) {
                continue;
            }
            let latest = results
                .iter()
                .flatten()
                .max_by_key(|(tick, _)| *tick)
                .unwrap();
            let _ = writeln!(
                failures,
                "{} ({}) {} {}: tick {}: {}",
                fixture.name, fixture.rule, recorded.kind, recorded.tag, latest.0, latest.1
            );
        }
    }
    println!("{checked} recorded entities");
    assert!(failures.is_empty(), "{}", failures);
}
