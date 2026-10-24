use mcrs_minecraft_core::{BlockPos, BlockStateId, BoundingBox, Mirror, Rotation, SectionPos};
use mcrs_minecraft_volume::{
    BlockVolume, BlockVolumeMut, CopyError, Denied, SparseVolume, Transform, copy_blocks,
};

/// An L of three blocks at the corner of section (0, 0, 0): stone at the corner, dirt
/// one step east and glass one step south.
fn source() -> (SparseVolume, [BlockStateId; 4]) {
    let [air, stone, dirt, glass] = crate::states(4)[..] else {
        unreachable!("four states")
    };
    let mut from = SparseVolume::unbounded();
    from.insert(SectionPos::new(0, 0, 0), air);
    for (at, state) in [((0, 0, 0), stone), ((1, 0, 0), dirt), ((0, 0, 1), glass)] {
        let _ = from.set_block(BlockPos::new(at.0, at.1, at.2), state);
    }
    (from, [air, stone, dirt, glass])
}

fn area() -> BoundingBox {
    BoundingBox {
        min: BlockPos::new(0, 0, 0),
        max: BlockPos::new(1, 0, 1),
    }
}

#[test]
fn a_clockwise_copy_turns_east_into_south_and_south_into_west() {
    let (from, [air, stone, dirt, glass]) = source();
    let mut to = SparseVolume::unbounded();
    to.insert(SectionPos::new(0, 0, 0), air);
    to.insert(SectionPos::new(-1, 0, 0), air);
    let transform = Transform {
        rotation: Rotation::Clockwise90,
        offset: BlockPos::new(4, 2, 4),
        ..Transform::IDENTITY
    };

    assert_eq!(copy_blocks(&from, area(), &mut to, transform, |s| s), Ok(4));
    assert_eq!(to.block(BlockPos::new(4, 2, 4)), Ok(stone));
    assert_eq!(to.block(BlockPos::new(4, 2, 5)), Ok(dirt));
    assert_eq!(to.block(BlockPos::new(3, 2, 4)), Ok(glass));
    assert_eq!(to.block(BlockPos::new(3, 2, 5)), Ok(air));
}

#[test]
fn a_mirrored_copy_flips_south_to_north() {
    let (from, [air, stone, _, glass]) = source();
    let mut to = SparseVolume::unbounded();
    to.insert(SectionPos::new(0, 0, 0), air);
    to.insert(SectionPos::new(0, 0, -1), air);
    let transform = Transform {
        mirror: Mirror::LeftRight,
        offset: BlockPos::new(0, 0, 8),
        ..Transform::IDENTITY
    };

    assert_eq!(copy_blocks(&from, area(), &mut to, transform, |s| s), Ok(4));
    assert_eq!(to.block(BlockPos::new(0, 0, 8)), Ok(stone));
    assert_eq!(to.block(BlockPos::new(0, 0, 7)), Ok(glass));
}

#[test]
fn a_copy_that_reaches_an_unloaded_target_moves_nothing() {
    let (from, [air, ..]) = source();
    let mut to = SparseVolume::unbounded();
    to.insert(SectionPos::new(0, 0, 0), air);
    let transform = Transform {
        offset: BlockPos::new(15, 0, 0),
        ..Transform::IDENTITY
    };

    assert_eq!(
        copy_blocks(&from, area(), &mut to, transform, |s| s),
        Err(CopyError::Target(Denied::Unloaded))
    );
    assert_eq!(to.block(BlockPos::new(15, 0, 0)), Ok(air));
}
