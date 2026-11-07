//! Property tests of the invariants of the move procedure, over random worlds of the test table.
//! A debug build runs a short sweep; a release build runs 10^6 cases for each property.
#![expect(
    clippy::indexing_slicing,
    reason = "the generators index small fixed tables"
)]

use glam::DVec3;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_physics::BirthKey;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::Collider;
use mcrs_minecraft_physics::MoveFlags;
use mcrs_minecraft_physics::Mover;
use mcrs_minecraft_physics::MoverKind;
use mcrs_minecraft_physics::NoBodies;
use mcrs_minecraft_physics::Physics;
use mcrs_minecraft_physics::Probe;
use mcrs_minecraft_physics::QueryEntity;
use mcrs_minecraft_physics::QueryVariant;
use mcrs_minecraft_physics::Resolved;
use mcrs_minecraft_physics::Scratch;
use mcrs_minecraft_physics::Side;
use mcrs_minecraft_physics::SourceKind;
use mcrs_minecraft_physics::SourcedCollider;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::TestBlocks;
use mcrs_minecraft_physics::TestWorld;
use mcrs_minecraft_physics::View;
use mcrs_minecraft_physics::collide;
use mcrs_minecraft_physics::move_body;
use mcrs_minecraft_physics::query_colliders;
use mcrs_minecraft_physics::resolve;
use mcrs_voxel::BlockPos;

const CASES: u32 = if cfg!(debug_assertions) {
    20_000
} else {
    1_000_000
};
const CASES_PER_WORLD: u32 = 64;
const EPSILON: f64 = 1.0e-7;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1_u64 << 53) as f64
    }

    fn range(&mut self, low: f64, high: f64) -> f64 {
        low + (high - low) * self.unit()
    }

    fn below(&mut self, count: usize) -> usize {
        (self.next() % count as u64) as usize
    }

    fn chance(&mut self, numerator: u64, denominator: u64) -> bool {
        self.next() % denominator < numerator
    }

    /// A displacement component: often zero, sometimes tiny, else up to 2 blocks.
    fn component(&mut self) -> f64 {
        match self.below(8) {
            0 | 1 => 0.0,
            2 => self.range(-2.0e-7, 2.0e-7),
            3 => self.range(-0.1, 0.1),
            _ => self.range(-2.0, 2.0),
        }
    }
}

struct Case {
    entity: QueryEntity,
    bounds: Aabb,
    width: f32,
    height: f32,
    displacement: DVec3,
    step_height: f32,
    on_ground: bool,
}

struct Fixture {
    blocks: TestBlocks,
    tables: Tables,
    rng: Rng,
    world: TestWorld,
    scratch: Scratch,
}

impl Fixture {
    fn new(seed: u64) -> Self {
        let blocks = TestBlocks::new();
        let tables = Tables::new(blocks.table.clone());
        Self {
            blocks,
            tables,
            rng: Rng(seed),
            world: TestWorld::new(),
            scratch: Scratch::new(),
        }
    }

    fn reroll_world(&mut self) {
        let mut world = TestWorld::new();
        let solid = self.blocks.solid_states();
        let density = self.rng.below(4) as u64 + 1;
        for z in -3..=3 {
            for y in -3..=3 {
                for x in -3..=3 {
                    if self.rng.chance(density, 10) {
                        let state = solid[self.rng.below(solid.len())];
                        world.set(BlockPos::new(x, y, z), state);
                    }
                }
            }
        }
        self.world = world;
    }

    fn case(&mut self, index: u32) -> Case {
        if index.is_multiple_of(CASES_PER_WORLD) {
            self.reroll_world();
        }
        let rng = &mut self.rng;
        let width = [0.25_f32, 0.6, 0.98, 1.4][rng.below(4)];
        let height = [0.25_f32, 0.6, 1.8, 1.95][rng.below(4)];
        let mut position = DVec3::new(
            rng.range(-2.0, 2.0),
            rng.range(-2.0, 2.0),
            rng.range(-2.0, 2.0),
        );
        if rng.chance(1, 3) {
            position.y = position.y.round();
        }
        let entity = QueryEntity {
            key: BirthKey::new(0, SourceKind::Command, 0, 0),
            position,
            fall_distance: 0.0,
            shift: false,
            powder_snow_solid: false,
            walks_on_powder_snow: false,
        };
        let displacement = DVec3::new(rng.component(), rng.component(), rng.component());
        Case {
            entity,
            bounds: Aabb::of_body(position, width, height),
            width,
            height,
            displacement,
            step_height: [0.0_f32, 0.6, 1.0][rng.below(3)],
            on_ground: rng.chance(1, 2),
        }
    }

    fn view(&self) -> View<'_, TestWorld, NoBodies> {
        View {
            world: &self.world,
            bodies: &NoBodies,
            tables: &self.tables,
        }
    }

    fn resolve(&mut self, case: &Case) -> Resolved {
        let probe = Probe {
            entity: case.entity,
            bounds: case.bounds,
            displacement: case.displacement,
            step_height: case.step_height,
            on_ground: case.on_ground,
        };
        let view = View {
            world: &self.world,
            bodies: &NoBodies,
            tables: &self.tables,
        };
        resolve(view, &probe, &mut self.scratch)
    }

    fn colliders(&self, case: &Case, query: Aabb) -> Vec<Collider> {
        let mut found: Vec<SourcedCollider> = Vec::new();
        query_colliders(
            self.view(),
            &case.entity,
            query,
            None,
            QueryVariant::Collision,
            &mut found,
        );
        found.into_iter().map(|found| found.collider).collect()
    }

    /// The colliders near the box before and after any move of the case.
    fn neighbourhood(&self, case: &Case) -> Vec<Collider> {
        let reach = case.bounds.expand_towards(case.displacement);
        let up = f64::from(case.step_height);
        self.colliders(case, reach.inflate(DVec3::new(1.0, 1.0 + up, 1.0)))
    }

    fn overlaps(&self, colliders: &[Collider], bounds: &Aabb) -> bool {
        let shrunk = bounds.deflate(EPSILON);
        colliders
            .iter()
            .any(|collider| collider.intersects(&self.tables.collision, &shrunk))
    }

    fn mover(&self, case: &Case) -> Mover {
        Mover {
            key: case.entity.key,
            width: case.width,
            height: case.height,
            step_height: case.step_height,
            physics: Physics::Collide,
            side: Side::Server { authority: true },
            shift: false,
            keeps_to_edges: false,
            restitution: 0.0,
            block_bounce_factor: 1.0,
            gravity: 0.08,
            vertical_drag: 0.98,
            movement_efficiency: None,
            powder_snow_solid: false,
            walks_on_powder_snow: false,
            intent: None,
        }
    }

    fn body(case: &Case) -> Body {
        Body {
            position: case.entity.position,
            velocity: case.displacement,
            flags: MoveFlags {
                on_ground: case.on_ground,
                ..MoveFlags::default()
            },
            ..Body::default()
        }
    }
}

fn run(seed: u64, mut check: impl FnMut(&mut Fixture, &Case)) {
    let mut fixture = Fixture::new(seed);
    for index in 0..CASES {
        let case = fixture.case(index);
        check(&mut fixture, &case);
    }
}

fn axes(vector: DVec3) -> [f64; 3] {
    vector.to_array()
}

#[test]
fn inv_001_002_no_axis_grows_or_turns() {
    run(1, |fixture, case| {
        let resolved = fixture.resolve(case);
        let r = resolved.displacement;
        for (index, (moved, asked)) in axes(r).into_iter().zip(axes(case.displacement)).enumerate()
        {
            let lifted = index == 1 && resolved.step.is_some();
            if asked == 0.0 && !lifted {
                assert_eq!(moved, 0.0, "a zero request moves zero on axis {index}");
            }
            if resolved.step.is_none() {
                assert!(
                    moved.abs() <= asked.abs(),
                    "axis {index}: {moved} beyond {asked}"
                );
                assert!(
                    moved.abs() < EPSILON || moved.signum() == asked.signum(),
                    "axis {index}: {moved} turned against {asked}"
                );
            }
        }
    });
}

#[test]
fn inv_003_a_free_box_stays_free() {
    run(3, |fixture, case| {
        let colliders = fixture.neighbourhood(case);
        if fixture.overlaps(&colliders, &case.bounds) {
            return;
        }
        let resolved = fixture.resolve(case);
        let after = case.bounds.translate(resolved.displacement);
        assert!(
            !fixture.overlaps(&colliders, &after),
            "{:?} moved by {:?} into a collider",
            case.bounds,
            resolved.displacement
        );
    });
}

#[test]
fn inv_004_005_step_up_lifts_within_the_height_and_gains_distance() {
    run(4, |fixture, case| {
        let resolved = fixture.resolve(case);
        let Some(step) = resolved.step else {
            return;
        };
        let r = resolved.displacement;
        let lift = r.y - step.base_lift;
        let height = f64::from(case.step_height);
        assert!(
            -EPSILON <= lift && lift <= height + EPSILON,
            "lift {lift} for {height}"
        );
        let gained = r.x * r.x + r.z * r.z;
        let plain = step.plain.x * step.plain.x + step.plain.z * step.plain.z;
        assert!(gained > plain, "the step-up gains {gained} over {plain}");
    });
}

#[test]
fn inv_006_007_on_ground_after_a_landing() {
    run(6, |fixture, case| {
        let mover = fixture.mover(case);
        let mut body = Fixture::body(case);
        let colliders = fixture.neighbourhood(case);
        let outcome = move_body(
            fixture.view(),
            &mover,
            &mut body,
            MoverKind::Own,
            case.displacement,
            &mut Scratch::new(),
        );
        let (d, r) = (outcome.requested, outcome.resolved);
        assert_eq!(body.flags.on_ground, r.y != d.y && d.y < 0.0);
        if !body.flags.on_ground || outcome.step.is_some() {
            return;
        }
        if d.y.abs() < EPSILON && !colliders.is_empty() {
            return;
        }
        let bottom = case.bounds.min.y + r.y;
        let mut lines = Vec::new();
        let near = colliders.iter().any(|collider| {
            collider.write_y_lines(&fixture.tables.collision, &mut lines);
            lines.iter().any(|line| (line - bottom).abs() <= EPSILON)
        });
        assert!(near, "on ground at {bottom} with no grid line under it");
    });
}

#[test]
fn inv_008_the_move_is_deterministic() {
    run(8, |fixture, case| {
        let first = fixture.resolve(case);
        let second = fixture.resolve(case);
        assert_eq!(
            axes(first.displacement).map(f64::to_bits),
            axes(second.displacement).map(f64::to_bits)
        );
    });
}

#[test]
fn inv_009_collider_order_changes_little() {
    run(9, |fixture, case| {
        let query = case.bounds.expand_towards(case.displacement);
        let mut colliders = fixture.colliders(case, query);
        if colliders.len() < 2 {
            return;
        }
        let table = &fixture.tables.collision;
        let ordered = collide(table, &colliders, case.bounds, case.displacement);
        colliders.reverse();
        let rotate = fixture.rng.below(colliders.len());
        colliders.rotate_left(rotate);
        let shuffled = collide(table, &colliders, case.bounds, case.displacement);
        let difference = (ordered - shuffled).abs().max_element();
        assert!(
            difference <= EPSILON,
            "order changed the move by {difference}"
        );
    });
}

#[test]
fn inv_010_no_collider_moves_the_whole_request() {
    let mut fixture = Fixture::new(10);
    let empty = TestWorld::new();
    for index in 0..CASES {
        let case = fixture.case(index);
        let probe = Probe {
            entity: case.entity,
            bounds: case.bounds,
            displacement: case.displacement,
            step_height: case.step_height,
            on_ground: case.on_ground,
        };
        let view = View {
            world: &empty,
            bodies: &NoBodies,
            tables: &fixture.tables,
        };
        let resolved = resolve(view, &probe, &mut fixture.scratch);
        assert_eq!(resolved.displacement, case.displacement);
    }
}

#[test]
fn inv_012_edge_back_off_only_shortens() {
    run(12, |fixture, case| {
        let mover = Mover {
            shift: true,
            keeps_to_edges: true,
            ..fixture.mover(case)
        };
        let mut body = Fixture::body(case);
        let d = DVec3::new(
            case.displacement.x,
            -case.displacement.y.abs(),
            case.displacement.z,
        );
        let outcome = move_body(
            fixture.view(),
            &mover,
            &mut body,
            MoverKind::Own,
            d,
            &mut Scratch::new(),
        );
        for (backed, asked) in [(outcome.requested.x, d.x), (outcome.requested.z, d.z)] {
            assert!(backed.abs() <= asked.abs(), "{backed} beyond {asked}");
            assert!(
                backed == 0.0 || backed.signum() == asked.signum(),
                "{backed} turned against {asked}"
            );
        }
    });
}

#[test]
fn inv_013_a_clipped_axis_loses_its_speed() {
    run(13, |fixture, case| {
        let mover = fixture.mover(case);
        let mut body = Fixture::body(case);
        let outcome = move_body(
            fixture.view(),
            &mover,
            &mut body,
            MoverKind::Own,
            case.displacement,
            &mut Scratch::new(),
        );
        if outcome.clipped.x {
            assert_eq!(body.velocity.x, 0.0);
        }
        if outcome.clipped.z {
            assert_eq!(body.velocity.z, 0.0);
        }
    });
}

#[test]
fn section_masks_give_the_result_of_reading_every_block() {
    let mut fixture = Fixture::new(14);
    let mut masked = TestWorld::new();
    for index in 0..CASES {
        let case = fixture.case(index);
        if index.is_multiple_of(CASES_PER_WORLD) {
            masked = fixture.world.clone();
            masked.build_masks(&fixture.tables.collision);
        }
        let plain = fixture.resolve(&case);
        let original = core::mem::replace(&mut fixture.world, masked);
        let with_masks = fixture.resolve(&case);
        masked = core::mem::replace(&mut fixture.world, original);
        assert_eq!(plain, with_masks);
    }
}

#[test]
fn flg_004_a_move_off_the_ground_clears_the_missed_search() {
    let fixture = Fixture::new(0);
    let position = DVec3::new(0.5, 10.0, 0.5);
    let case = Case {
        entity: QueryEntity {
            key: BirthKey::new(0, SourceKind::Command, 0, 0),
            position,
            fall_distance: 0.0,
            shift: false,
            powder_snow_solid: false,
            walks_on_powder_snow: false,
        },
        bounds: Aabb::of_body(position, 0.6, 1.8),
        width: 0.6,
        height: 1.8,
        displacement: DVec3::new(0.0, -0.1, 0.0),
        step_height: 0.6,
        on_ground: true,
    };
    let mut body = Fixture::body(&case);
    body.support_missed = true;
    move_body(
        fixture.view(),
        &fixture.mover(&case),
        &mut body,
        MoverKind::Own,
        case.displacement,
        &mut Scratch::new(),
    );
    assert!(!body.flags.on_ground);
    assert!(!body.support_missed);
}
