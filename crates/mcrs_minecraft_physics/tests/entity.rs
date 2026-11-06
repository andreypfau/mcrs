//! Kernel and property tests of the tick of server-simulated entities.
#![expect(
    clippy::indexing_slicing,
    reason = "the generators index small fixed tables"
)]

use glam::DVec3;
use mcrs_minecraft_physics::BirthKey;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::LANES;
use mcrs_minecraft_physics::Living;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::NoBodies;
use mcrs_minecraft_physics::Rest;
use mcrs_minecraft_physics::Scratch;
use mcrs_minecraft_physics::Side;
use mcrs_minecraft_physics::SourceKind;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::TestBlocks;
use mcrs_minecraft_physics::TestWorld;
use mcrs_minecraft_physics::View;
use mcrs_minecraft_physics::in_open_air;
use mcrs_minecraft_physics::same_bits;
use mcrs_minecraft_physics::tick_entity;
use mcrs_minecraft_physics::tick_in_air;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;

const CASES: u32 = if cfg!(debug_assertions) {
    2_000
} else {
    100_000
};
const SERVER: Side = Side::Server { authority: true };

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn range(&mut self, low: f64, high: f64) -> f64 {
        low + (high - low) * ((self.next() >> 11) as f64 / (1_u64 << 53) as f64)
    }

    fn below(&mut self, count: usize) -> usize {
        (self.next() % count as u64) as usize
    }
}

struct Fixture {
    blocks: TestBlocks,
    tables: Tables,
    world: TestWorld,
    scratch: Scratch,
    actions: Vec<mcrs_minecraft_physics::BlockAction>,
}

impl Fixture {
    fn new() -> Self {
        let blocks = TestBlocks::new();
        let tables = Tables::new(blocks.table.clone());
        Self {
            blocks,
            tables,
            world: TestWorld::new(),
            scratch: Scratch::new(),
            actions: Vec::new(),
        }
    }

    fn floor(&mut self, state: BlockStateId) {
        self.world
            .fill(BlockPos::new(-4, 0, -4), BlockPos::new(4, 0, 4), state);
    }

    fn tick(
        &mut self,
        row: &MotionRow,
        entity: &mut EntityState,
        wake: bool,
    ) -> mcrs_minecraft_physics::TickOutcome {
        let view = View {
            world: &self.world,
            bodies: &NoBodies,
            tables: &self.tables,
        };
        self.actions.clear();
        let outcome = tick_entity(
            view,
            SERVER,
            row,
            entity,
            wake,
            &mut self.actions,
            &mut self.scratch,
        );
        entity.age = entity.age.wrapping_add(1);
        outcome
    }
}

fn entity(position: DVec3, velocity: DVec3, living: bool) -> EntityState {
    let body = Body {
        position,
        velocity,
        ..Body::default()
    };
    let living = living.then(Living::default);
    EntityState::new(BirthKey::new(0, SourceKind::Command, 0, 0), body, living)
}

fn velocity_after(fixture: &mut Fixture, row: &MotionRow, living: bool, ticks: u32) -> Vec<DVec3> {
    let mut state = entity(DVec3::new(0.5, 3.0, 0.5), DVec3::ZERO, living);
    let mut seen = Vec::new();
    for _ in 0..ticks {
        fixture.tick(row, &mut state, true);
        seen.push(state.body.velocity);
    }
    seen
}

fn bits(vector: DVec3) -> [u64; 3] {
    vector.to_array().map(f64::to_bits)
}

#[test]
fn skip_003_rest_states_on_a_flat_block_top() {
    let mut fixture = Fixture::new();
    fixture.floor(fixture.blocks.stone);
    let walker = velocity_after(&mut fixture, &MotionRow::armor_stand(), true, 40);
    assert_eq!(walker[39].y, -0.0784000015258789);
    assert_eq!(walker[39].y, walker[38].y);
    let tnt = velocity_after(&mut fixture, &MotionRow::primed_tnt(), false, 40);
    assert_eq!(bits(tnt[39])[1], (-0.0_f64).to_bits());
    let orb = velocity_after(&mut fixture, &MotionRow::experience_orb(), false, 40);
    assert_eq!(bits(orb[39])[1], 0.0_f64.to_bits());
    let item = velocity_after(&mut fixture, &MotionRow::item(), false, 41);
    let cycle: Vec<f64> = item[37..41].iter().map(|velocity| velocity.y).collect();
    assert_eq!(cycle, [-0.04, -0.04 - 0.04, -0.04 - 0.04 - 0.04, 0.0]);
}

#[test]
fn grv_007_ground_drag_is_0_546000063419342() {
    assert_eq!(f64::from(0.6_f32 * 0.91_f32), 0.546000063419342);
    let mut fixture = Fixture::new();
    fixture.floor(fixture.blocks.stone);
    let row = MotionRow::armor_stand();
    let mut state = entity(
        DVec3::new(0.5, 1.0, 0.5),
        DVec3::new(0.0, -0.0784, 0.0),
        true,
    );
    state.body.flags.on_ground = true;
    fixture.tick(&row, &mut state, true);
    state.body.velocity.x = 0.5;
    fixture.tick(&row, &mut state, true);
    assert_eq!(state.body.velocity.x, 0.5 * 0.546000063419342);
}

#[test]
fn inv_011_horizontal_speed_decays_by_the_ground_factor() {
    let mut fixture = Fixture::new();
    fixture.world.fill(
        BlockPos::new(-64, 0, -64),
        BlockPos::new(64, 0, 64),
        fixture.blocks.stone,
    );
    let row = MotionRow::armor_stand();
    let mut rng = Rng(11);
    for _ in 0..CASES / 20 {
        let mut state = entity(
            DVec3::new(0.5, 1.0, 0.5),
            DVec3::new(0.0, -0.0784, 0.0),
            true,
        );
        state.body.flags.on_ground = true;
        fixture.tick(&row, &mut state, true);
        state.body.velocity.x = rng.range(-0.5, 0.5);
        state.body.velocity.z = rng.range(-0.5, 0.5);
        for _ in 0..40 {
            let before = state.body.velocity;
            fixture.tick(&row, &mut state, true);
            for (after, start) in [
                (state.body.velocity.x, before.x),
                (state.body.velocity.z, before.z),
            ] {
                if start.abs() < 0.003 {
                    assert_eq!(after, 0.0);
                } else {
                    assert_eq!(after, start * 0.546000063419342);
                }
            }
        }
    }
}

/// A random scene: a floor of a random state with random blocks on it, and an entity of a random
/// category with a random start state.
fn scene(fixture: &mut Fixture, rng: &mut Rng) -> (MotionRow, EntityState) {
    let mut world = TestWorld::new();
    let floors = [
        fixture.blocks.stone,
        fixture.blocks.ice,
        fixture.blocks.slime,
        fixture.blocks.soul_sand,
    ];
    world.fill(
        BlockPos::new(-3, 0, -3),
        BlockPos::new(3, 0, 3),
        floors[rng.below(floors.len())],
    );
    let solid = fixture.blocks.solid_states();
    for _ in 0..rng.below(6) {
        let at = BlockPos::new(rng.below(7) as i32 - 3, 1, rng.below(7) as i32 - 3);
        world.set(at, solid[rng.below(solid.len())]);
    }
    fixture.world = world;
    let rows = [
        MotionRow::armor_stand(),
        MotionRow::zombie(),
        MotionRow::item(),
        MotionRow::primed_tnt(),
        MotionRow::falling_block(),
        MotionRow::experience_orb(),
    ];
    let row = rows[rng.below(rows.len())];
    let position = DVec3::new(
        rng.range(-1.0, 1.0),
        rng.range(1.0, 4.0),
        rng.range(-1.0, 1.0),
    );
    let velocity = DVec3::new(
        rng.range(-0.3, 0.3),
        rng.range(-0.3, 0.3),
        rng.range(-0.3, 0.3),
    );
    let mut state = entity(position, velocity, row.category.is_living());
    state.phase = rng.below(4) as u8;
    (row, state)
}

#[test]
fn skip_rest_equals_no_rest() {
    let mut fixture = Fixture::new();
    let mut rng = Rng(5);
    for _ in 0..CASES / 10 {
        let (row, start) = scene(&mut fixture, &mut rng);
        let mut full = start;
        let mut skipping = start;
        for tick in 0..60 {
            fixture.tick(&row, &mut full, true);
            let wake = rng.below(16) == 0;
            fixture.tick(&row, &mut skipping, wake);
            assert!(
                same_bits(&full.body, &skipping.body),
                "tick {tick} of {:?}: {:?} against {:?}",
                row.category,
                full.body,
                skipping.body
            );
        }
    }
}

#[test]
fn batch_equals_scalar_in_open_air() {
    let mut fixture = Fixture::new();
    fixture.world.fill(
        BlockPos::new(0, -64, 0),
        BlockPos::new(1, -64, 1),
        fixture.blocks.stone,
    );
    let plain = fixture.world.clone();
    let mut masked = fixture.world.clone();
    masked.build_masks(&fixture.tables.collision);
    let mut rng = Rng(7);
    let rows = [
        MotionRow::item(),
        MotionRow::primed_tnt(),
        MotionRow::falling_block(),
        MotionRow::experience_orb(),
    ];
    let mut entities = Vec::new();
    for _ in 0..CASES {
        let row = rows[rng.below(rows.len())];
        let mut lanes: [EntityState; LANES] = core::array::from_fn(|_| {
            let position = DVec3::new(
                rng.range(100.0, 200.0),
                rng.range(100.0, 200.0),
                rng.range(100.0, 200.0),
            );
            let velocity = DVec3::new(
                rng.range(-2.0, 2.0),
                rng.range(-2.0, 2.0),
                rng.range(-2.0, 2.0),
            );
            let mut state = entity(position, velocity, false);
            state.body.fall_distance = rng.range(0.0, 10.0);
            state.no_gravity = rng.below(8) == 0;
            state.phase = rng.below(4) as u8;
            state.age = rng.below(1000) as u32;
            if rng.below(4) == 0 {
                state.body.velocity = DVec3::ZERO;
                state.rest = Rest::Candidate(state.body);
            }
            state
        });
        let mut general = lanes;
        let mut fast = lanes;
        let fast_view = View {
            world: &masked,
            bodies: &NoBodies,
            tables: &fixture.tables,
        };
        let general_view = View {
            world: &plain,
            bodies: &NoBodies,
            tables: &fixture.tables,
        };
        for lane in &lanes {
            assert!(in_open_air(fast_view, &row, lane, &mut entities));
            assert!(!in_open_air(general_view, &row, lane, &mut entities));
        }
        tick_in_air(&row, &mut lanes);
        let mut actions = Vec::new();
        for (single, quick) in general.iter_mut().zip(&mut fast) {
            tick_entity(
                general_view,
                SERVER,
                &row,
                single,
                false,
                &mut actions,
                &mut fixture.scratch,
            );
            tick_entity(
                fast_view,
                SERVER,
                &row,
                quick,
                false,
                &mut actions,
                &mut fixture.scratch,
            );
        }
        for ((batched, single), quick) in lanes.iter().zip(&general).zip(&fast) {
            assert!(
                same_bits(&batched.body, &single.body),
                "{:?} against {:?}",
                batched.body,
                single.body
            );
            assert!(
                same_bits(&quick.body, &single.body),
                "{:?} against {:?}",
                quick.body,
                single.body
            );
            assert_eq!(batched.rest, single.rest);
            assert_eq!(quick.rest, single.rest);
        }
    }
}
