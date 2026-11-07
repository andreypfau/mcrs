//! Vanilla parity of the tick of server-simulated entities: each fixture under
//! `tests/fixtures/physics` holds the states that a vanilla server printed after each tick of one
//! experiment of the physics specification. `tools/physics-fixtures/record.sh` records them.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "a malformed fixture fails the test"
)]

use std::fmt::Write as _;
use std::path::Path;

use glam::DVec3;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_physics::BirthKey;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::EntityCollider;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::GridFlags;
use mcrs_minecraft_physics::GridRow;
use mcrs_minecraft_physics::KeyedStream;
use mcrs_minecraft_physics::ListedBodies;
use mcrs_minecraft_physics::Living;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::MoveFlags;
use mcrs_minecraft_physics::PushRules;
use mcrs_minecraft_physics::PushTargets;
use mcrs_minecraft_physics::Scratch;
use mcrs_minecraft_physics::Side;
use mcrs_minecraft_physics::SourceKind;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::TeamSlot;
use mcrs_minecraft_physics::TestWorld;
use mcrs_minecraft_physics::Travel;
use mcrs_minecraft_physics::View;
use mcrs_minecraft_physics::WorldBorder;
use mcrs_minecraft_physics::push_entity;
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
        "zombie" => MotionRow::zombie(),
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
        let no_ai = recorded.nbt.contains("NoAI:1b");
        if no_gravity || no_ai {
            living.travel = Travel::Closed;
        }
        living
    });
    let mut state = EntityState::new(BirthKey::new(0, SourceKind::Command, 0, 0), body, living);
    state.no_gravity = no_gravity;
    state.phase = phase;
    state
}

/// A recorded entity that the kernel steps.
struct Simulated<'a> {
    recorded: &'a Recorded,
    row: MotionRow,
    state: EntityState,
    failure: Option<(usize, String)>,
}

/// Steps every simulated entity of a fixture together, with the push between the living ones
/// from the state at the start of each tick; records the first tick that differs for each.
fn replay(view: View<'_, TestWorld, ListedBodies>, entities: &mut [Simulated<'_>]) {
    let mut scratch = Scratch::new();
    let mut actions = Vec::new();
    let rules = PushRules::default();
    let ticks = entities
        .iter()
        .map(|entity| entity.recorded.ticks.len())
        .max()
        .unwrap_or(0);
    for tick in 1..ticks {
        let rows: Vec<GridRow> = entities
            .iter()
            .enumerate()
            .filter_map(|(index, entity)| grid_row(entity, index))
            .collect();
        let bodies = ListedBodies {
            colliders: view.bodies.colliders.clone(),
            rows,
        };
        let view = View::new(view.world, &bodies, view.tables);
        for (index, entity) in entities.iter_mut().enumerate() {
            if entity.failure.is_some() {
                continue;
            }
            let Some(Some(expected)) = entity.recorded.ticks.get(tick).copied() else {
                continue;
            };
            let own = grid_row(entity, index);
            actions.clear();
            tick_entity(
                view,
                SERVER,
                &entity.row,
                &mut entity.state,
                true,
                &mut actions,
                &mut scratch,
            );
            if let Some(own) = own {
                let stream = KeyedStream::new(&own.key, tick as u64);
                push_entity(
                    &bodies,
                    &own,
                    &rules,
                    &stream,
                    &mut entity.state.body.velocity,
                    &mut scratch,
                );
            }
            entity.state.age = entity.state.age.wrapping_add(1);
            entity.failure = compare(&entity.state.body, &expected).map(|text| (tick, text));
        }
    }
}

fn grid_row(entity: &Simulated<'_>, index: usize) -> Option<GridRow> {
    entity.row.category.is_living().then(|| {
        let id = EntityId(index as u64 + 1);
        let width = entity.row.dimensions.width;
        GridRow {
            position: entity.state.body.position,
            half_width: width / 2.0,
            height: entity.row.dimensions.height,
            key: BirthKey::new(0, SourceKind::Command, 0, index as u32),
            id,
            root_vehicle: id,
            team: TeamSlot::default(),
            flags: if entity.recorded.kind == "armor_stand" {
                GridFlags::empty()
            } else {
                GridFlags::PUSHABLE
            },
            push_targets: if entity.recorded.kind == "armor_stand" {
                PushTargets::None
            } else {
                PushTargets::Each
            },
        }
    })
}

fn compare(body: &Body, expected: &Sample) -> Option<String> {
    let position = (body.position - DVec3::from_array(expected.pos))
        .abs()
        .max_element();
    let velocity = (body.velocity - DVec3::from_array(expected.motion))
        .abs()
        .max_element();
    let fall = (body.fall_distance - expected.fall_distance).abs();
    let same = position <= TOLERANCE
        && velocity <= TOLERANCE
        && fall <= TOLERANCE
        && body.flags.on_ground == expected.on_ground;
    (!same).then(|| {
        format!(
            "position {:?} against {:?}, velocity {:?} against {:?}, on ground {} against {}, fall {} against {}",
            body.position.to_array(),
            expected.pos,
            body.velocity.to_array(),
            expected.motion,
            body.flags.on_ground,
            expected.on_ground,
            body.fall_distance,
            expected.fall_distance
        )
    })
}

const SERVER: Side = Side::Server { authority: true };

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
        let bodies = ListedBodies {
            colliders: fixture
                .entities
                .iter()
                .filter(|recorded| recorded.collider)
                .filter_map(|recorded| {
                    let sample = recorded.ticks.first().copied().flatten()?;
                    Some(EntityCollider {
                        bounds: Aabb::of_body(DVec3::from_array(sample.pos), 1.0, 1.0),
                        id: EntityId(1000),
                    })
                })
                .collect(),
            rows: Vec::new(),
        };
        let view = View::new(&world, &bodies, &tables);
        // An item rests in a cycle whose phase comes from its network id, which the console does
        // not print: each phase runs, and an item passes when one phase matches.
        let runs: Vec<Vec<Option<(usize, String)>>> = (0..4)
            .map(|phase| {
                let mut entities: Vec<Simulated<'_>> = fixture
                    .entities
                    .iter()
                    .filter_map(|recorded| {
                        let row = row_of(&recorded.kind)?;
                        let first = recorded.ticks.first().copied().flatten()?;
                        Some(Simulated {
                            recorded,
                            row,
                            state: start_state(recorded, &row, &first, phase),
                            failure: None,
                        })
                    })
                    .collect();
                replay(view, &mut entities);
                entities.into_iter().map(|entity| entity.failure).collect()
            })
            .collect();
        let simulated: Vec<&Recorded> = fixture
            .entities
            .iter()
            .filter(|recorded| row_of(&recorded.kind).is_some())
            .collect();
        for (index, recorded) in simulated.iter().enumerate() {
            checked += 1;
            let results: Vec<&Option<(usize, String)>> =
                runs.iter().filter_map(|run| run.get(index)).collect();
            if results.iter().any(|result| result.is_none()) {
                continue;
            }
            let latest = results
                .iter()
                .filter_map(|result| result.as_ref())
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
