use mcrs_minecraft_core::{BlockPos, BoundingBox, SectionPos};
use mcrs_minecraft_volume::{
    Access, BlockStateId, BlockVolume, BlockVolumeMut, BoxVolume, Denied, SparseVolume,
    Unavailable, Volume,
};

const AIR: BlockStateId = BlockStateId(0);
const STONE: BlockStateId = BlockStateId(1);

#[test]
fn a_sparse_volume_answers_absent_sections_as_unloaded_even_at_the_extremes() {
    let mut volume = SparseVolume::unbounded();
    let far = BlockPos::new(i32::MAX, i32::MIN, i32::MAX);
    let section = SectionPos::from(far);
    assert_eq!(volume.access(section), Access::Unloaded);
    assert_eq!(volume.block(far), Err(Unavailable::Unloaded));
    assert_eq!(volume.set_block(far, STONE), Err(Denied::Unloaded));

    volume.insert(section, AIR);
    assert_eq!(volume.access(section), Access::Write);
    assert_eq!(volume.set_block(far, STONE), Ok(AIR));
    assert_eq!(volume.set_block(far, AIR), Ok(STONE));
    assert!(volume.remove(section));
    assert_eq!(volume.block(far), Err(Unavailable::Unloaded));
}

#[test]
fn a_sparse_volume_holds_only_sections_wholly_inside_its_bounds() {
    let mut volume = SparseVolume::new(BoundingBox {
        min: BlockPos::new(0, 0, 0),
        max: BlockPos::new(19, 15, 15),
    });
    assert!(volume.insert(SectionPos::new(0, 0, 0), STONE));
    assert!(!volume.insert(SectionPos::new(1, 0, 0), STONE));
    assert!(!volume.insert(SectionPos::new(2, 0, 0), STONE));
    assert_eq!(volume.len(), 1);

    assert_eq!(volume.block(BlockPos::new(15, 0, 0)), Ok(STONE));
    assert_eq!(volume.access(SectionPos::new(1, 0, 0)), Access::Unloaded);
    assert_eq!(
        volume.block(BlockPos::new(19, 0, 0)),
        Err(Unavailable::Unloaded)
    );
    assert_eq!(
        volume.block(BlockPos::new(20, 0, 0)),
        Err(Unavailable::Outside)
    );
    assert_eq!(volume.access(SectionPos::new(2, 0, 0)), Access::Outside);
    assert_eq!(
        volume.section(SectionPos::new(2, 0, 0)).err(),
        Some(Unavailable::Outside)
    );
    assert_eq!(
        volume.set_block(BlockPos::new(19, 0, 0), AIR),
        Err(Denied::Unloaded)
    );
    assert_eq!(
        volume.set_block(BlockPos::new(20, 0, 0), AIR),
        Err(Denied::Outside)
    );
}

#[test]
fn a_box_grants_the_sections_it_touches_but_only_its_own_cells() {
    let bounds = BoundingBox {
        min: BlockPos::new(-2, 0, 5),
        max: BlockPos::new(1, 3, 6),
    };
    let mut volume = BoxVolume::filled(bounds, STONE).expect("small");
    assert_eq!(volume.access(SectionPos::new(-1, 0, 0)), Access::Write);
    assert_eq!(volume.access(SectionPos::new(1, 0, 0)), Access::Outside);

    assert_eq!(volume.set_block(BlockPos::new(-2, 3, 6), AIR), Ok(STONE));
    assert_eq!(volume.block(BlockPos::new(-2, 3, 6)), Ok(AIR));
    assert_eq!(
        volume.block(BlockPos::new(2, 0, 5)),
        Err(Unavailable::Outside)
    );
    assert_eq!(
        volume.set_block(BlockPos::new(-3, 0, 5), AIR),
        Err(Denied::Outside)
    );

    let cells: Vec<_> = volume.iter().collect();
    assert_eq!(cells.len(), 4 * 4 * 2);
    assert_eq!(cells[0], (BlockPos::new(-2, 0, 5), STONE));
    assert_eq!(cells[1].0, BlockPos::new(-1, 0, 5));
    assert_eq!(cells.iter().filter(|(_, state)| *state == AIR).count(), 1);

    volume.fill_layer(3, AIR);
    assert_eq!(volume.iter().filter(|(_, state)| *state == AIR).count(), 8);
}

#[test]
fn a_box_too_large_to_address_is_refused() {
    let bounds = BoundingBox {
        min: BlockPos::new(i32::MIN, i32::MIN, i32::MIN),
        max: BlockPos::new(i32::MAX, i32::MAX, i32::MAX),
    };
    assert!(BoxVolume::filled(bounds, AIR).is_none());
}

#[test]
fn an_empty_box_grants_no_section() {
    let bounds = BoundingBox {
        min: BlockPos::new(1, 0, 0),
        max: BlockPos::new(0, 0, 0),
    };
    let volume = BoxVolume::filled(bounds, STONE).expect("an empty box is addressable");
    assert_eq!(volume.access(SectionPos::new(0, 0, 0)), Access::Outside);
}
