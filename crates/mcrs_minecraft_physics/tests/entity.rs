//! Kernel and property tests of the tick of server-simulated entities.
#![expect(
    clippy::indexing_slicing,
    reason = "the generators index small fixed tables"
)]

use glam::DVec3;
use mcrs_minecraft_physics::BirthKey;
use mcrs_minecraft_physics::BlockAction;
use mcrs_minecraft_physics::BlockFlags;
use mcrs_minecraft_physics::BlockMotion;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::CollisionTableBuilder;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::InsideBoxes;
use mcrs_minecraft_physics::InsideEffect;
use mcrs_minecraft_physics::Living;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::NoBodies;
use mcrs_minecraft_physics::Rest;
use mcrs_minecraft_physics::Scratch;
use mcrs_minecraft_physics::Side;
use mcrs_minecraft_physics::SourceKind;
use mcrs_minecraft_physics::StateSpec;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::TestBlocks;
use mcrs_minecraft_physics::TestFixture;
use mcrs_minecraft_physics::TestRng;
use mcrs_minecraft_physics::TestWorld;
use mcrs_minecraft_physics::View;
use mcrs_minecraft_physics::in_open_air;
use mcrs_minecraft_physics::same_bits;
use mcrs_minecraft_physics::tick_entity;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;

const CASES: u32 = if cfg!(debug_assertions) {
    2_000
} else {
    100_000
};
const SERVER: Side = Side::Server { authority: true };

fn floor(world: &mut TestWorld, state: BlockStateId) {
    world.fill(BlockPos::new(-4, 0, -4), BlockPos::new(4, 0, 4), state);
}

fn tick(fixture: &mut TestFixture, row: &MotionRow, entity: &mut EntityState, wake: bool) {
    fixture.actions.clear();
    tick_entity(
        View::new(&fixture.world, &NoBodies, &fixture.tables),
        SERVER,
        row,
        entity,
        wake,
        &mut fixture.actions,
        &mut fixture.scratch,
    );
    entity.age = entity.age.wrapping_add(1);
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

fn velocity_after(
    fixture: &mut TestFixture,
    row: &MotionRow,
    living: bool,
    ticks: u32,
) -> Vec<DVec3> {
    let mut state = entity(DVec3::new(0.5, 3.0, 0.5), DVec3::ZERO, living);
    let mut seen = Vec::new();
    for _ in 0..ticks {
        tick(fixture, row, &mut state, true);
        seen.push(state.body.velocity);
    }
    seen
}

fn bits(vector: DVec3) -> [u64; 3] {
    vector.to_array().map(f64::to_bits)
}

#[test]
fn skip_003_rest_states_on_a_flat_block_top() {
    let mut fixture = TestFixture::new();
    floor(&mut fixture.world, fixture.blocks.stone);
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

/// The fall distance after one tick in which the entity, with a fall distance of 5, falls into
/// `block`, and the drop of that tick.
fn fall_into(row: &MotionRow, living: bool, block: BlockStateId) -> (f64, f64) {
    let mut fixture = TestFixture::new();
    floor(&mut fixture.world, fixture.blocks.stone);
    fixture.world.set(BlockPos::new(0, 1, 0), block);
    let start = DVec3::new(0.5, 2.2, 0.5);
    let mut state = entity(start, DVec3::new(0.0, -0.5, 0.0), living);
    state.body.fall_distance = 5.0;
    tick(&mut fixture, row, &mut state, true);
    (state.body.fall_distance, start.y - state.body.position.y)
}

/// E53: an item with `Motion` `[0.1, 0, 0.07]` on stone has both horizontal components in
/// `[-1.0E-6, 1.0E-6]` first at tick 67; from there they are 0 and the item rests.
#[test]
fn dev_014_an_item_on_the_ground_stops_and_rests() {
    let mut fixture = TestFixture::new();
    floor(&mut fixture.world, fixture.blocks.stone);
    let first_stops: Vec<u32> = (0..4)
        .map(|phase| {
            let mut item = entity(DVec3::new(0.5, 1.0, 0.5), DVec3::new(0.1, 0.0, 0.07), false);
            item.phase = phase;
            let mut stop = None;
            for tick_number in 1..=80 {
                tick(&mut fixture, &MotionRow::item(), &mut item, false);
                let [x, _, z] = item.body.velocity.to_array();
                if x.abs() <= 1.0e-6 && z.abs() <= 1.0e-6 && stop.is_none() {
                    assert_eq!([x, z], [0.0, 0.0], "the step cuts the small speed to 0");
                    stop = Some(tick_number);
                }
            }
            assert!(
                matches!(item.rest, Rest::Resting(_)),
                "the stopped item rests"
            );
            stop.unwrap_or(u32::MAX)
        })
        .collect();
    assert!(first_stops.contains(&67), "{first_stops:?}");
}

#[test]
fn dev_014_only_an_item_on_the_ground_stops() {
    let mut fixture = TestFixture::new();
    floor(&mut fixture.world, fixture.blocks.stone);
    let small = DVec3::new(5.0e-7, 0.0, -5.0e-7);
    for (row, height, on_ground) in [
        (MotionRow::item(), 1.5, false),
        (MotionRow::experience_orb(), 1.0, true),
    ] {
        let mut state = entity(DVec3::new(0.5, height, 0.5), small, false);
        state.body.flags.on_ground = on_ground;
        tick(&mut fixture, &row, &mut state, true);
        assert_eq!(
            state.body.flags.on_ground, on_ground,
            "the {:?} stays where it was",
            row.category
        );
        assert!(
            state.body.velocity.x != 0.0 && state.body.velocity.z != 0.0,
            "the {:?} keeps its small speed: {}",
            row.category,
            state.body.velocity
        );
    }
}

#[test]
fn fall_004_a_resetting_block_keeps_the_fall_of_the_tick() {
    let blocks = TestBlocks::new();
    for (row, living, block) in [
        (MotionRow::item(), false, blocks.sweet_berry_bush),
        (MotionRow::armor_stand(), true, blocks.cobweb),
    ] {
        let (fall, drop) = fall_into(&row, living, block);
        assert!(drop > 0.0, "the {:?} falls", row.category);
        assert_eq!(fall, drop);
    }
}

#[test]
fn fall_004_the_feet_reset_the_fall_when_the_centre_stays_above_the_block() {
    let mut fixture = TestFixture::new();
    floor(&mut fixture.world, fixture.blocks.stone);
    fixture
        .world
        .set(BlockPos::new(0, 1, 0), fixture.blocks.sweet_berry_bush);
    let start = DVec3::new(0.5, 2.05, 0.5);
    let mut item = entity(start, DVec3::new(0.0, -0.06, 0.0), false);
    item.body.fall_distance = 5.0;
    tick(&mut fixture, &MotionRow::item(), &mut item, true);
    let drop = start.y - item.body.position.y;
    assert!(
        item.body.position.y + 0.125 > 2.0,
        "the centre stays above the block"
    );
    assert!(
        (item.body.fall_distance - drop).abs() < 1.0e-12,
        "only the fall of the tick"
    );
}

/// A table can give a state an inside effect or a fall reset with no inside action; the section
/// masks must still send the box through the inside step.
#[test]
fn fall_004_a_block_acts_inside_a_masked_section_without_an_inside_action() {
    let mut builder = CollisionTableBuilder::new();
    let mut push = |flags: BlockFlags, inside: InsideEffect| {
        builder
            .push(StateSpec {
                boxes: &[],
                context: None,
                flags,
                inside,
                inside_shape: InsideBoxes::FullCube,
                motion: BlockMotion::DEFAULT,
            })
            .unwrap()
    };
    let _air = push(BlockFlags::empty(), InsideEffect::None);
    let fall_reset = push(BlockFlags::FALL_DAMAGE_RESETTING, InsideEffect::None);
    let cobweb = push(BlockFlags::empty(), InsideEffect::Cobweb);
    let tables = Tables::new(builder.build());
    for block in [fall_reset, cobweb] {
        let mut world = TestWorld::new();
        world.set(BlockPos::new(0, 1, 0), block);
        world.build_masks(&tables.collision);
        let mut item = entity(DVec3::new(0.5, 1.5, 0.5), DVec3::ZERO, false);
        item.body.fall_distance = 5.0;
        tick_entity(
            View::new(&world, &NoBodies, &tables),
            SERVER,
            &MotionRow::item(),
            &mut item,
            true,
            &mut Vec::new(),
            &mut Scratch::new(),
        );
        if block == fall_reset {
            assert!(item.body.fall_distance < 1.0, "the block resets the fall");
        } else {
            assert_ne!(item.body.stuck, DVec3::ZERO, "the cobweb holds the item");
        }
    }
}

#[test]
fn rch_008_a_thin_inside_shape_acts_only_on_a_centre_that_reaches_it() {
    let tripwire = BlockPos::new(0, 1, 0);
    for (height, acts) in [(1.5, false), (1.05, true)] {
        let mut fixture = TestFixture::new();
        fixture.world.set(tripwire, fixture.blocks.tripwire);
        let mut item = entity(DVec3::new(0.5, height, 0.5), DVec3::ZERO, false);
        item.no_gravity = true;
        tick(&mut fixture, &MotionRow::item(), &mut item, true);
        assert_eq!(
            fixture.actions.contains(&BlockAction::Inside(tripwire)),
            acts,
            "an item at {height}"
        );
    }
}

#[test]
fn ord_005_a_resting_item_applies_the_blocks_of_its_last_path() {
    let mut fixture = TestFixture::new();
    floor(&mut fixture.world, fixture.blocks.stone);
    let cobweb = BlockPos::new(0, 2, 0);
    fixture.world.set(cobweb, fixture.blocks.cobweb);
    let mut item = entity(DVec3::new(0.5, 2.1, 0.5), DVec3::new(0.0, -1.2, 0.0), false);
    tick(&mut fixture, &MotionRow::item(), &mut item, true);
    assert!(item.body.flags.on_ground, "the item lands");
    let landed = item.body.position;
    tick(&mut fixture, &MotionRow::item(), &mut item, true);
    assert_eq!(item.body.position, landed, "the item rests");
    assert!(fixture.actions.contains(&BlockAction::Inside(cobweb)));
}

#[test]
fn grv_002_slow_falling_reads_the_vertical_velocity_after_the_move() {
    let mut fixture = TestFixture::new();
    floor(&mut fixture.world, fixture.blocks.stone);
    fixture
        .world
        .set(BlockPos::new(0, 3, 0), fixture.blocks.stone);
    let mut zombie = entity(DVec3::new(0.5, 1.0, 0.5), DVec3::new(0.0, 0.3, 0.0), true);
    if let Some(living) = zombie.living.as_mut() {
        living.effects.slow_falling = true;
    }
    tick(&mut fixture, &MotionRow::zombie(), &mut zombie, true);
    assert!(
        zombie.body.flags.vertical_collision,
        "the head hits the block"
    );
    assert_eq!(zombie.body.velocity.y, -0.01 * f64::from(0.98_f32));
}

#[test]
fn inv_011_horizontal_speed_decays_by_the_ground_factor() {
    assert_eq!(f64::from(0.6_f32 * 0.91_f32), 0.546000063419342);
    let mut fixture = TestFixture::new();
    fixture.world.fill(
        BlockPos::new(-64, 0, -64),
        BlockPos::new(64, 0, 64),
        fixture.blocks.stone,
    );
    let row = MotionRow::armor_stand();
    let mut rng = TestRng(11);
    for _ in 0..CASES / 20 {
        let mut state = entity(
            DVec3::new(0.5, 1.0, 0.5),
            DVec3::new(0.0, -0.0784, 0.0),
            true,
        );
        state.body.flags.on_ground = true;
        tick(&mut fixture, &row, &mut state, true);
        state.body.velocity.x = rng.range(-0.5, 0.5);
        state.body.velocity.z = rng.range(-0.5, 0.5);
        for _ in 0..40 {
            let before = state.body.velocity;
            tick(&mut fixture, &row, &mut state, true);
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
fn scene(fixture: &mut TestFixture, rng: &mut TestRng) -> (MotionRow, EntityState) {
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
    let mut fixture = TestFixture::new();
    let mut rng = TestRng(5);
    for _ in 0..CASES / 10 {
        let (row, start) = scene(&mut fixture, &mut rng);
        let mut full = start;
        let mut skipping = start;
        for index in 0..60 {
            tick(&mut fixture, &row, &mut full, true);
            let wake = rng.below(16) == 0;
            tick(&mut fixture, &row, &mut skipping, wake);
            assert!(
                same_bits(&full.body, &skipping.body),
                "tick {index} of {:?}: {:?} against {:?}",
                row.category,
                full.body,
                skipping.body
            );
        }
    }
}

#[test]
fn the_open_air_path_equals_the_general_path() {
    let mut fixture = TestFixture::new();
    fixture.world.fill(
        BlockPos::new(0, -64, 0),
        BlockPos::new(1, -64, 1),
        fixture.blocks.stone,
    );
    let plain = fixture.world.clone();
    let mut masked = fixture.world.clone();
    masked.build_masks(&fixture.tables.collision);
    let fast_view = View::new(&masked, &NoBodies, &fixture.tables);
    let general_view = View::new(&plain, &NoBodies, &fixture.tables);
    let mut rng = TestRng(7);
    let rows = [
        MotionRow::item(),
        MotionRow::primed_tnt(),
        MotionRow::falling_block(),
        MotionRow::experience_orb(),
    ];
    let mut entities = Vec::new();
    for _ in 0..CASES {
        let row = rows[rng.below(rows.len())];
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
        let mut general = entity(position, velocity, false);
        general.body.fall_distance = rng.range(0.0, 10.0);
        general.no_gravity = rng.below(8) == 0;
        general.phase = rng.below(4) as u8;
        general.age = rng.below(1000) as u32;
        if rng.below(4) == 0 {
            general.body.velocity = DVec3::ZERO;
            general.rest = Rest::Candidate(general.body);
        }
        let mut fast = general;
        assert!(in_open_air(fast_view, &row, &fast, &mut entities));
        assert!(!in_open_air(general_view, &row, &general, &mut entities));
        for (view, state) in [(general_view, &mut general), (fast_view, &mut fast)] {
            tick_entity(
                view,
                SERVER,
                &row,
                state,
                false,
                &mut fixture.actions,
                &mut fixture.scratch,
            );
        }
        assert!(
            same_bits(&fast.body, &general.body),
            "{:?} against {:?}",
            fast.body,
            general.body
        );
        assert_eq!(fast.rest, general.rest);
    }
}
