use core::f64::consts::PI;

use glam::DVec3;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_physics::Axis;
use mcrs_minecraft_physics::BirthKey;
use mcrs_minecraft_physics::BlockFlags;
use mcrs_minecraft_physics::BlockMotion;
use mcrs_minecraft_physics::Collider;
use mcrs_minecraft_physics::ColliderSource;
use mcrs_minecraft_physics::CollisionTableBuilder;
use mcrs_minecraft_physics::InsideEffect;
use mcrs_minecraft_physics::NoBodies;
use mcrs_minecraft_physics::Probe;
use mcrs_minecraft_physics::QueryEntity;
use mcrs_minecraft_physics::QueryVariant;
use mcrs_minecraft_physics::Scratch;
use mcrs_minecraft_physics::Shape;
use mcrs_minecraft_physics::SineTable;
use mcrs_minecraft_physics::SourceKind;
use mcrs_minecraft_physics::SourcedCollider;
use mcrs_minecraft_physics::StateSpec;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::TestBlocks;
use mcrs_minecraft_physics::TestWorld;
use mcrs_minecraft_physics::View;
use mcrs_minecraft_physics::WorldBorder;
use mcrs_minecraft_physics::collide;
use mcrs_minecraft_physics::query_colliders;
use mcrs_minecraft_physics::resolve;
use mcrs_minecraft_physics::yaw_radians;
use mcrs_voxel::BlockPos;

fn entity_at(position: DVec3) -> QueryEntity {
    QueryEntity {
        key: BirthKey::new(0, SourceKind::Command, 0, 0),
        position,
        fall_distance: 0.0,
        shift: false,
        powder_snow_solid: false,
        walks_on_powder_snow: false,
    }
}

fn unit_box(min: [f64; 3], max: [f64; 3]) -> Aabb {
    Aabb::new(DVec3::from_array(min), DVec3::from_array(max))
}

#[test]
fn sine_table_has_exact_quarter_turns() {
    let table = SineTable::new();
    assert_eq!(table.entry(0).to_bits(), 0.0_f32.to_bits());
    assert_eq!(table.entry(16384), 1.0);
    assert_eq!(table.entry(32768).to_bits(), 0.0_f32.to_bits());
    assert_eq!(table.entry(49152), -1.0);
    assert_eq!(table.entry(1), (1.0 / 10430.378350470453_f64).sin() as f32);
}

#[test]
fn sine_table_reads_raw_yaw_beyond_one_turn() {
    let table = SineTable::new();
    // 450 degrees is 81920 table steps: one turn and a quarter.
    assert_eq!(table.sin(yaw_radians(450.0)), 1.0);
    assert_eq!(table.cos(yaw_radians(450.0)), 0.0);
    assert_eq!(table.sin(yaw_radians(-450.0)), -1.0);
    assert_eq!(table.cos(yaw_radians(-450.0)), 0.0);
    assert_eq!(table.sin(yaw_radians(720.0)), 0.0);
    assert_eq!(table.cos(yaw_radians(-720.0)), 1.0);
    assert_eq!(
        yaw_radians(180.0),
        f64::from(180.0_f32 * (PI / 180.0) as f32)
    );
}

#[test]
fn bottom_slab_has_three_lines_on_y_and_an_open_top_cell() {
    let slab = Shape::from_boxes(&[unit_box([0.0; 3], [1.0, 0.5, 1.0])]).unwrap();
    assert_eq!(slab.lines(Axis::Y), [0.0, 0.5, 1.0]);
    assert_eq!(slab.lines(Axis::X), [0.0, 1.0]);
    assert!(slab.is_solid(0, 0, 0));
    assert!(!slab.is_solid(0, 1, 0));
}

#[test]
fn carpet_is_one_box_with_its_faces_as_lines() {
    // One pixel is off the grid of 1/8, the finest grid of one box, so the faces are the lines.
    let blocks = TestBlocks::new();
    let class = blocks.table.state(blocks.carpet).class;
    let mcrs_minecraft_physics::ShapeClass::OneBox(id) = class else {
        panic!("a carpet is one box, not {class:?}");
    };
    let carpet = blocks.table.shape(id).unwrap();
    assert_eq!(carpet.lines(Axis::Y), [0.0, 1.0 / 16.0]);
}

#[test]
fn box_on_a_quarter_grid_stops_at_an_inner_line() {
    // A box that penetrates a shape stops at the next grid line inside the shape.
    let shape = Shape::from_boxes(&[unit_box([0.25; 3], [0.75; 3])]).unwrap();
    assert_eq!(shape.lines(Axis::Y), [0.0, 0.25, 0.5, 0.75, 1.0]);
    let blocks = TestBlocks::new();
    let mover = unit_box([0.3, 0.2, 0.3], [0.7, 0.3, 0.7]);
    let collider = Collider::Entity(unit_box([0.25; 3], [0.75; 3]));
    assert_eq!(
        collider.clip(&blocks.table, Axis::Y, &mover, 1.0),
        0.5 - 0.3
    );
}

#[test]
fn clip_keeps_a_result_just_past_zero() {
    let blocks = TestBlocks::new();
    let wall = Collider::Entity(unit_box([1.0, 0.0, 0.0], [2.0, 1.0, 1.0]));
    let mover = unit_box([0.0, 0.0, 0.0], [1.000_000_05, 1.0, 1.0]);
    let resolved = collide(&blocks.table, &[wall], mover, DVec3::new(0.5, 0.0, 0.0));
    assert_eq!(resolved.x, 1.0 - 1.000_000_05);
    assert!(resolved.x < 0.0);
}

#[test]
fn clip_stops_at_exactly_zero_before_the_next_collider() {
    let blocks = TestBlocks::new();
    let wall = Collider::Entity(unit_box([1.0, 0.0, 0.0], [2.0, 1.0, 1.0]));
    let far = Collider::Entity(unit_box([5.0, 0.0, 0.0], [6.0, 1.0, 1.0]));
    let mover = unit_box([0.0, 0.0, 0.0], [1.000_000_05, 1.0, 1.0]);
    let resolved = collide(
        &blocks.table,
        &[wall, far],
        mover,
        DVec3::new(0.5, 0.0, 0.0),
    );
    assert_eq!(resolved.x.to_bits(), 0.0_f64.to_bits());
}

#[test]
fn step_up_takes_the_lowest_height_that_gains_distance() {
    let blocks = TestBlocks::new();
    let tables = Tables::new(blocks.table.clone());
    let mut world = TestWorld::new();
    world.fill(
        BlockPos::new(-2, 0, -2),
        BlockPos::new(4, 0, 2),
        blocks.stone,
    );
    world.set(BlockPos::new(1, 1, 0), blocks.carpet);
    world.set(BlockPos::new(2, 1, 0), blocks.snow[1]);
    let position = DVec3::new(0.5, 1.0, 0.5);
    let bounds = Aabb::of_body(position, 0.6, 1.95);
    let probe = Probe {
        entity: entity_at(position),
        bounds,
        displacement: DVec3::new(1.5, -0.08, 0.0),
        step_height: 0.6,
        on_ground: true,
    };
    let view = View {
        world: &world,
        bodies: &NoBodies,
        tables: &tables,
    };
    let resolved = resolve(view, &probe, &mut Scratch::new());
    assert_eq!(
        resolved.displacement,
        DVec3::new(2.0 - bounds.max.x, 1.0 / 16.0, 0.0)
    );
    let step = resolved.step.unwrap();
    assert_eq!(step.plain.x, 1.0 - bounds.max.x);
}

#[test]
fn scan_visits_x_fastest_then_y_then_z() {
    let blocks = TestBlocks::new();
    let tables = Tables::new(blocks.table.clone());
    let mut world = TestWorld::new();
    world.fill(BlockPos::new(0, 0, 0), BlockPos::new(1, 1, 1), blocks.stone);
    let query = unit_box([0.1; 3], [1.9; 3]);
    let found = colliders_of(&world, &tables, query, QueryVariant::Collision);
    let order: Vec<BlockPos> = found.iter().filter_map(block_pos).collect();
    let expected: Vec<BlockPos> = [
        (0, 0, 0),
        (1, 0, 0),
        (0, 1, 0),
        (1, 1, 0),
        (0, 0, 1),
        (1, 0, 1),
        (0, 1, 1),
        (1, 1, 1),
    ]
    .map(BlockPos::from)
    .to_vec();
    assert_eq!(order, expected);
}

#[test]
fn scan_reads_large_shapes_on_faces_only() {
    let mut builder = CollisionTableBuilder::new();
    let spec = |boxes| StateSpec {
        boxes,
        context: None,
        flags: BlockFlags::empty(),
        inside: InsideEffect::None,
        motion: BlockMotion::DEFAULT,
    };
    let _air = builder.push(spec(&[]));
    let big = builder
        .push(spec(&[unit_box([-0.5; 3], [1.5; 3])]))
        .unwrap();
    let tables = Tables::new(builder.build());
    // The region of this query is (-1, -1, -1) to (1, 1, 1).
    let query = unit_box([0.1; 3], [0.9; 3]);
    let found_at = |at: BlockPos| {
        let mut world = TestWorld::new();
        world.set(at, big);
        !colliders_of(&world, &tables, query, QueryVariant::Collision).is_empty()
    };
    assert!(
        found_at(BlockPos::new(-1, 0, 0)),
        "a large shape on a face counts"
    );
    assert!(
        found_at(BlockPos::new(0, 1, 0)),
        "a large shape on a face counts"
    );
    assert!(
        !found_at(BlockPos::new(-1, 1, 0)),
        "only a moving piston counts on an edge"
    );
    assert!(
        !found_at(BlockPos::new(-1, -1, -1)),
        "a corner never counts"
    );
}

#[test]
fn feet_of_the_old_position_select_the_scaffolding_shape() {
    let blocks = TestBlocks::new();
    let tables = Tables::new(blocks.table.clone());
    let mut world = TestWorld::new();
    world.set(BlockPos::new(0, 0, 0), blocks.scaffolding);
    world.set_border(Some(WorldBorder {
        min_x: -1.5,
        max_x: 1.5,
        min_z: -1.5,
        max_z: 1.5,
    }));
    let query = unit_box([0.2, 0.5, 0.2], [0.8, 1.5, 0.8]);
    let view = View {
        world: &world,
        bodies: &NoBodies,
        tables: &tables,
    };
    let inside = entity_at(DVec3::new(0.5, 0.5, 0.5));
    let mut found = Vec::new();
    query_colliders(
        view,
        &inside,
        query,
        None,
        QueryVariant::Collision,
        &mut found,
    );
    assert!(
        found
            .iter()
            .all(|found| found.source == ColliderSource::Border)
    );
    found.clear();
    query_colliders(
        view,
        &inside,
        query,
        Some(1.0),
        QueryVariant::Collision,
        &mut found,
    );
    let top = tables.collision.scaffolding_top();
    assert!(found.iter().any(|found| matches!(
        found.collider,
        Collider::Block { shape, .. } if shape == top
    )));
    found.clear();
    query_colliders(
        view,
        &inside,
        query,
        Some(1.0),
        QueryVariant::Placement,
        &mut found,
    );
    assert!(found.is_empty());
}

#[test]
fn border_stops_a_box_at_the_rounded_edge() {
    let blocks = TestBlocks::new();
    let tables = Tables::new(blocks.table.clone());
    let mut world = TestWorld::new();
    world.set_border(Some(WorldBorder {
        min_x: -4.5,
        max_x: 4.5,
        min_z: -4.5,
        max_z: 4.5,
    }));
    let position = DVec3::new(4.0, 0.0, 0.0);
    let bounds = Aabb::of_body(position, 0.6, 1.8);
    let probe = Probe {
        entity: entity_at(position),
        bounds,
        displacement: DVec3::new(2.0, 0.0, 0.0),
        step_height: 0.0,
        on_ground: false,
    };
    let view = View {
        world: &world,
        bodies: &NoBodies,
        tables: &tables,
    };
    let resolved = resolve(view, &probe, &mut Scratch::new());
    assert_eq!(resolved.displacement.x, 5.0 - bounds.max.x);
}

fn colliders_of(
    world: &TestWorld,
    tables: &Tables,
    query: Aabb,
    variant: QueryVariant,
) -> Vec<SourcedCollider> {
    let view = View {
        world,
        bodies: &NoBodies,
        tables,
    };
    let mut found = Vec::new();
    let entity = entity_at(query.min);
    query_colliders(view, &entity, query, None, variant, &mut found);
    found
}

fn block_pos(found: &SourcedCollider) -> Option<BlockPos> {
    match found.source {
        ColliderSource::Block { pos, .. } => Some(pos),
        _ => None,
    }
}
