use glam::DVec3;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_physics::Axis;
use mcrs_minecraft_physics::BlockFlags;
use mcrs_minecraft_physics::BlockMotion;
use mcrs_minecraft_physics::Collider;
use mcrs_minecraft_physics::ColliderSource;
use mcrs_minecraft_physics::CollisionTableBuilder;
use mcrs_minecraft_physics::InsideBoxes;
use mcrs_minecraft_physics::InsideEffect;
use mcrs_minecraft_physics::NoBodies;
use mcrs_minecraft_physics::Probe;
use mcrs_minecraft_physics::QueryVariant;
use mcrs_minecraft_physics::Scratch;
use mcrs_minecraft_physics::Shape;
use mcrs_minecraft_physics::SourcedCollider;
use mcrs_minecraft_physics::StateSpec;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::TestBlocks;
use mcrs_minecraft_physics::TestFixture;
use mcrs_minecraft_physics::TestWorld;
use mcrs_minecraft_physics::View;
use mcrs_minecraft_physics::WorldBorder;
use mcrs_minecraft_physics::collide;
use mcrs_minecraft_physics::query_colliders;
use mcrs_minecraft_physics::resolve;
use mcrs_voxel::BlockPos;

fn unit_box(min: [f64; 3], max: [f64; 3]) -> Aabb {
    Aabb::new(DVec3::from_array(min), DVec3::from_array(max))
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
fn shp_005_one_axis_off_the_eighth_grid_keeps_only_faces_on_every_axis() {
    let shape = Shape::from_boxes(&[unit_box([0.25, 0.0, 0.0], [0.75, 1.0 / 16.0, 1.0])]).unwrap();
    assert_eq!(shape.lines(Axis::X), [0.25, 0.75]);
    assert_eq!(shape.lines(Axis::Y), [0.0, 1.0 / 16.0]);
}

#[test]
fn shp_005_union_joins_a_line_exactly_one_tolerance_above_the_kept_line() {
    let shape = Shape::from_boxes(&[
        unit_box([0.1, 0.0, 0.0], [0.3, 1.0, 1.0]),
        unit_box([0.3 + 1.0e-7, 0.0, 0.0], [0.6, 1.0, 1.0]),
    ])
    .unwrap();
    assert_eq!(shape.lines(Axis::X), [0.1, 0.3, 0.6]);
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
fn col_002_negative_zero_survives_only_without_colliders() {
    let blocks = TestBlocks::new();
    let mover = unit_box([0.0; 3], [1.0; 3]);
    let d = DVec3::new(-0.0, -0.5, -0.0);
    let free = collide(&blocks.table, &[], mover, d);
    assert_eq!(free.x.to_bits(), (-0.0_f64).to_bits());
    let far = Collider::Entity(unit_box([5.0, 0.0, 0.0], [6.0, 1.0, 1.0]));
    let blocked = collide(&blocks.table, &[far], mover, d);
    assert_eq!(blocked.x.to_bits(), 0.0_f64.to_bits());
    assert_eq!(blocked.z.to_bits(), 0.0_f64.to_bits());
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
        entity: TestFixture::query_entity(position),
        bounds,
        displacement: DVec3::new(1.5, -0.08, 0.0),
        step_height: 0.6,
        on_ground: true,
    };
    let view = View::new(&world, &NoBodies, &tables);
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
        inside_shape: InsideBoxes::FullCube,
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
    let view = View::new(&world, &NoBodies, &tables);
    let inside = TestFixture::query_entity(DVec3::new(0.5, 0.5, 0.5));
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
fn shp_010_grown_border_includes_its_minimum_lines_only() {
    let border = WorldBorder {
        min_x: -4.5,
        max_x: 4.5,
        min_z: -4.5,
        max_z: 4.5,
    };
    assert!(border.contains_with_margin(DVec3::new(-5.5, 0.0, -5.5), 1.0));
    assert!(!border.contains_with_margin(DVec3::new(5.5, 0.0, 0.0), 1.0));
    assert!(!border.contains_with_margin(DVec3::new(0.0, 0.0, 5.5), 1.0));
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
        entity: TestFixture::query_entity(position),
        bounds,
        displacement: DVec3::new(2.0, 0.0, 0.0),
        step_height: 0.0,
        on_ground: false,
    };
    let view = View::new(&world, &NoBodies, &tables);
    let resolved = resolve(view, &probe, &mut Scratch::new());
    assert_eq!(resolved.displacement.x, 5.0 - bounds.max.x);
}

fn colliders_of(
    world: &TestWorld,
    tables: &Tables,
    query: Aabb,
    variant: QueryVariant,
) -> Vec<SourcedCollider> {
    let view = View::new(world, &NoBodies, tables);
    let mut found = Vec::new();
    let entity = TestFixture::query_entity(query.min);
    query_colliders(view, &entity, query, None, variant, &mut found);
    found
}

fn block_pos(found: &SourcedCollider) -> Option<BlockPos> {
    match found.source {
        ColliderSource::Block { pos, .. } => Some(pos),
        _ => None,
    }
}
