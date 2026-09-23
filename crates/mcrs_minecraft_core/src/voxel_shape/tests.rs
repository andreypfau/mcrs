use super::*;

fn aabb(min: [f32; 3], max: [f32; 3]) -> Aabb {
    Aabb {
        min: Vec3::from(min),
        max: Vec3::from(max),
    }
}

fn bottom_slab() -> VoxelShape {
    VoxelShape::from_boxes(&[aabb([0.0, 0.0, 0.0], [1.0, 0.5, 1.0])])
}

fn top_slab() -> VoxelShape {
    VoxelShape::from_boxes(&[aabb([0.0, 0.5, 0.0], [1.0, 1.0, 1.0])])
}

fn set_bits(mask: &FaceMask) -> u32 {
    mask.iter().map(|w| w.count_ones()).sum()
}

#[test]
fn empty_returns_pointer_stable_static() {
    let a = VoxelShape::empty();
    let b = VoxelShape::empty();
    assert!(std::ptr::eq(a, b));
    assert!(a.is_empty());
    assert!(!a.occludes_full_block());
}

#[test]
fn block_returns_pointer_stable_static() {
    let a = VoxelShape::block();
    let b = VoxelShape::block();
    assert!(std::ptr::eq(a, b));
    assert!(!a.is_empty());
    assert!(a.occludes_full_block());
}

#[test]
fn singletons_referenced_through_static_globals_are_pointer_equal_to_accessors() {
    assert!(std::ptr::eq(&super::empty::EMPTY, VoxelShape::empty()));
    assert!(std::ptr::eq(&super::block::BLOCK, VoxelShape::block()));
}

#[test]
fn direction_opposite_is_involutive() {
    for dir in Direction::all() {
        assert_eq!(dir.opposite().opposite(), dir);
    }
}

#[test]
fn two_full_cubes_occlude() {
    let b = VoxelShape::block();
    for dir in Direction::all() {
        assert!(b.face_occludes(b, dir));
    }
}

#[test]
fn a_full_cube_occludes_against_an_empty_shape() {
    let b = VoxelShape::block();
    let e = VoxelShape::empty();
    for dir in Direction::all() {
        assert!(b.face_occludes(e, dir), "block against empty on {dir}");
        assert!(e.face_occludes(b, dir), "empty against block on {dir}");
    }
}

#[test]
fn two_empty_shapes_do_not_occlude() {
    let e = VoxelShape::empty();
    for dir in Direction::all() {
        assert!(!e.face_occludes(e, dir));
    }
}

#[test]
fn a_slab_covers_the_seam_it_sits_against_and_leaves_the_other_open() {
    let bottom = bottom_slab();
    assert_eq!(set_bits(bottom.face_mask(Direction::Down)), 1024);
    assert_eq!(set_bits(bottom.face_mask(Direction::Up)), 0);

    let top = top_slab();
    assert_eq!(set_bits(top.face_mask(Direction::Up)), 1024);
    assert_eq!(set_bits(top.face_mask(Direction::Down)), 0);
}

#[test]
fn a_slab_side_face_covers_half_the_seam() {
    let bottom = bottom_slab();
    for dir in [
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ] {
        assert_eq!(set_bits(bottom.face_mask(dir)), 512, "side face {dir}");
    }
}

#[test]
fn side_by_side_slabs_occlude_only_when_their_halves_complement() {
    let bottom = bottom_slab();
    let top = top_slab();
    for dir in [
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ] {
        assert!(
            !bottom.face_occludes(&bottom, dir),
            "two lower halves {dir}"
        );
        assert!(!top.face_occludes(&top, dir), "two upper halves {dir}");
        assert!(
            bottom.face_occludes(&top, dir),
            "lower and upper half {dir}"
        );
    }
}

#[test]
fn a_slab_side_face_against_a_full_cube_occludes() {
    let bottom = bottom_slab();
    assert!(bottom.face_occludes(VoxelShape::block(), Direction::East));
    assert!(VoxelShape::block().face_occludes(&bottom, Direction::East));
}

#[test]
fn stacked_slabs_occlude_only_where_a_solid_face_meets_the_seam() {
    let bottom = bottom_slab();
    let top = top_slab();
    assert!(
        bottom.face_occludes(&bottom, Direction::Up),
        "the upper slab's floor covers the seam"
    );
    assert!(
        !bottom.face_occludes(&top, Direction::Up),
        "a lower slab under a raised one leaves the seam open"
    );
    assert!(top.face_occludes(&bottom, Direction::Up));
}

#[test]
fn from_boxes_canonicalises_the_two_singleton_cases() {
    let empty = VoxelShape::from_boxes(&[]);
    assert!(empty.is_empty());
    assert!(!empty.occludes_full_block());

    let cube = VoxelShape::from_boxes(&[aabb([0.0, 0.0, 0.0], [1.0, 1.0, 1.0])]);
    assert!(matches!(cube.repr, ShapeRepr::Block));
    assert!(cube.occludes_full_block());

    let halves = VoxelShape::from_boxes(&[
        aabb([0.0, 0.0, 0.0], [1.0, 0.5, 1.0]),
        aabb([0.0, 0.5, 0.0], [1.0, 1.0, 1.0]),
    ]);
    assert!(matches!(halves.repr, ShapeRepr::Block));
}

#[test]
fn a_box_reaching_outside_the_unit_cube_is_clipped() {
    let overhanging = VoxelShape::from_boxes(&[aabb([-0.25, -0.25, -0.25], [1.25, 0.5, 1.25])]);
    let slab = bottom_slab();
    for dir in Direction::all() {
        assert_eq!(
            overhanging.face_mask(dir),
            slab.face_mask(dir),
            "clipped face {dir}"
        );
    }
    assert_eq!(overhanging.bounds.min, Vec3::ZERO);
    assert_eq!(overhanging.bounds.max, Vec3::new(1.0, 0.5, 1.0));
    assert!(!overhanging.occludes_full_block());
}

#[test]
fn a_degenerate_box_contributes_nothing() {
    let flat = VoxelShape::from_boxes(&[aabb([0.0, 0.0, 0.0], [1.0, 0.0, 1.0])]);
    assert!(flat.is_empty());
}

#[test]
fn shape_registry_new_reserves_empty_and_block() {
    let reg = ShapeRegistry::new();
    assert_eq!(reg.len(), 2);
    assert!(std::ptr::eq(reg.entries()[0], VoxelShape::empty()));
    assert!(std::ptr::eq(reg.entries()[1], VoxelShape::block()));
}

#[test]
fn interning_dedupes_equal_shapes_and_reuses_the_singletons() {
    let mut reg = ShapeRegistry::new();
    let a = reg.intern(bottom_slab());
    let b = reg.intern(bottom_slab());
    assert!(std::ptr::eq(a, b));
    assert_eq!(reg.len(), 3);
    assert!(std::ptr::eq(
        reg.intern(VoxelShape::from_boxes(&[aabb([0.0; 3], [1.0; 3])])),
        VoxelShape::block()
    ));
}

#[test]
fn an_off_grid_coordinate_rounds_to_the_nearest_cell() {
    let coarse = VoxelShape::from_boxes(&[aabb([0.0, 0.0, 0.0], [1.0, 0.51, 1.0])]);
    let slab = bottom_slab();
    for dir in Direction::all() {
        assert_eq!(coarse.face_mask(dir), slab.face_mask(dir), "face {dir}");
    }
}
