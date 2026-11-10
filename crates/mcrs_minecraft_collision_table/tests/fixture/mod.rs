//! The vanilla fixtures under `tests/fixtures/physics`: the states that a vanilla server printed
//! after each tick of one experiment of the physics specification.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "a malformed fixture fails the test"
)]

use std::collections::BTreeMap;
use std::fmt::Write as _;

use glam::DVec3;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_fixtures::FixtureFile;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::Living;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::MoveFlags;
use mcrs_minecraft_physics::Travel;
use mcrs_minecraft_physics::WorldBorder;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;
use serde::Deserialize;

pub const TOLERANCE: f64 = 1.0e-6;

#[derive(Deserialize)]
pub struct Fixture {
    pub name: String,
    pub rule: String,
    pub origin: [i32; 3],
    pub blocks: Vec<Blocks>,
    /// The blocks that the recorder changed between two ticks.
    #[serde(default)]
    pub changes: Vec<Change>,
    pub entities: Vec<Recorded>,
    pub border: Option<Border>,
    /// Set for an experiment that the recorder ran two times from the same world: true when the
    /// two runs printed the same states.
    pub repeat_equal: Option<bool>,
}

#[derive(Deserialize)]
pub struct Blocks {
    pub min: [i32; 3],
    pub max: [i32; 3],
    pub block: String,
}

/// A fill before the step to `tick`.
#[derive(Deserialize)]
pub struct Change {
    pub tick: usize,
    #[serde(flatten)]
    pub blocks: Blocks,
}

#[derive(Deserialize)]
pub struct Border {
    pub center: [f64; 2],
    pub size: f64,
}

#[derive(Deserialize)]
pub struct Recorded {
    pub tag: String,
    pub kind: String,
    pub nbt: String,
    #[serde(default)]
    pub attributes: BTreeMap<String, f64>,
    #[serde(default)]
    pub effects: Vec<String>,
    #[serde(default)]
    pub collider: bool,
    pub ticks: Vec<Option<Sample>>,
}

#[derive(Deserialize, Clone, Copy)]
pub struct Sample {
    pub pos: [f64; 3],
    pub motion: [f64; 3],
    pub on_ground: bool,
    pub fall_distance: f64,
}

/// Parses the fixture files that `mcrs_minecraft_fixtures` lists.
pub fn fixtures(files: &[FixtureFile]) -> Vec<Fixture> {
    files
        .iter()
        .map(|file| serde_json::from_slice(&file.bytes()).unwrap())
        .collect()
}

/// The block boxes of `fixture` in world coordinates, in the order the recorder filled them.
pub fn block_boxes(fixture: &Fixture) -> impl Iterator<Item = (BlockPos, BlockPos, BlockStateId)> {
    boxes(fixture, fixture.blocks.iter())
}

/// The block boxes that the recorder filled before the step to `tick`.
pub fn changes_before(
    fixture: &Fixture,
    tick: usize,
) -> impl Iterator<Item = (BlockPos, BlockPos, BlockStateId)> {
    let changes = fixture
        .changes
        .iter()
        .filter(move |change| change.tick == tick);
    boxes(fixture, changes.map(|change| &change.blocks))
}

fn boxes<'a>(
    fixture: &Fixture,
    blocks: impl Iterator<Item = &'a Blocks>,
) -> impl Iterator<Item = (BlockPos, BlockPos, BlockStateId)> {
    let [ox, oy, oz] = fixture.origin;
    let at = move |offset: [i32; 3]| BlockPos::new(ox + offset[0], oy + offset[1], oz + offset[2]);
    blocks.map(move |blocks| (at(blocks.min), at(blocks.max), state_of(&blocks.block)))
}

pub fn border_of(fixture: &Fixture) -> Option<WorldBorder> {
    fixture.border.as_ref().map(|border| {
        let half = border.size / 2.0;
        WorldBorder {
            min_x: border.center[0] - half,
            max_x: border.center[0] + half,
            min_z: border.center[1] - half,
            max_z: border.center[1] + half,
        }
    })
}

fn state_of(text: &str) -> BlockStateId {
    let definitions = crate::common::definitions();
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

pub fn start_state(
    recorded: &Recorded,
    row: &MotionRow,
    key: EntityKey,
    sample: &Sample,
    phase: u8,
) -> EntityState {
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
    let mut state = EntityState::new(key, body, living);
    state.no_gravity = no_gravity;
    state.phase = phase;
    state
}

/// The difference of `body` from the vanilla sample, or `None` within the tolerance.
pub fn compare(body: &Body, expected: &Sample) -> Option<String> {
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

/// The first tick at which a simulated entity left the vanilla states, with the difference.
pub type Failure = Option<(usize, String)>;

/// The entities of `fixture` that a [`MotionRow`] simulates, with that row, in fixture order.
pub fn simulated(fixture: &Fixture) -> impl Iterator<Item = (&Recorded, MotionRow)> {
    fixture
        .entities
        .iter()
        .filter_map(|recorded| Some((recorded, row_of(&recorded.kind)?)))
}

/// Writes a line to `failures` for each simulated entity that fails in every run; the runs differ
/// in the phase of the rest cycle of an item, which comes from its network id that the console
/// does not print. Gives the number of entities it checked.
pub fn report(fixture: &Fixture, runs: &[Vec<Failure>], failures: &mut String) -> usize {
    if fixture.repeat_equal == Some(false) {
        let _ = writeln!(failures, "{}: two vanilla runs differ", fixture.name);
    }
    for (index, (recorded, _)) in simulated(fixture).enumerate() {
        let results: Option<Vec<&(usize, String)>> = runs
            .iter()
            .map(|run| run.get(index).and_then(Option::as_ref))
            .collect();
        let latest = results.and_then(|results| results.into_iter().max_by_key(|(tick, _)| *tick));
        if let Some((tick, difference)) = latest {
            let _ = writeln!(
                failures,
                "{} ({}) {} {}: tick {tick}: {difference}",
                fixture.name, fixture.rule, recorded.kind, recorded.tag
            );
        }
    }
    simulated(fixture).count()
}
