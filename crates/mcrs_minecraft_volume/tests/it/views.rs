use std::ops::ControlFlow;

use mcrs_minecraft_core::{BlockPos, BoundingBox, SectionPos};
use mcrs_minecraft_volume::{
    Access, AccessPolicy, BlockStateId, BlockVolume, BlockVolumeMut, BoxVolume, Cursor, Missing,
    Restricted, SectionBox, SectionVolume, SparseVolume, Unavailable, VisitOptions, VisitOrder,
    Volume, VoxelStore, VoxelStoreMut,
};

use crate::access::WEAKEST_FIRST;

const AIR: BlockStateId = BlockStateId(0);
const STONE: BlockStateId = BlockStateId(1);

fn world(held: &[(i32, i32, i32)]) -> SparseVolume {
    let mut volume = SparseVolume::unbounded();
    for &(x, y, z) in held {
        volume.insert(SectionPos::new(x, y, z), AIR);
    }
    volume
}

#[test]
fn a_cursor_reads_what_the_volume_reads_across_sections() {
    let mut volume = world(&[(0, 0, 0), (1, 0, 0)]);
    volume
        .set_block(BlockPos::new(15, 4, 4), STONE)
        .expect("held");
    volume
        .set_block(BlockPos::new(16, 4, 4), STONE)
        .expect("held");
    let mut cursor = Cursor::new(&volume);
    for x in [14, 15, 16, 17, 32, 15, -1] {
        let at = BlockPos::new(x, 4, 4);
        assert_eq!(cursor.block(at), volume.block(at), "x = {x}");
    }
    assert_eq!(
        cursor.block(BlockPos::new(32, 0, 0)),
        Err(Unavailable::Unloaded)
    );
}

#[test]
fn a_cursor_holding_a_cut_section_still_refuses_cells_past_the_bounds() {
    let bounds = BoundingBox {
        min: BlockPos::new(-2, 0, 5),
        max: BlockPos::new(1, 3, 6),
    };
    let volume = BoxVolume::filled(bounds, STONE).expect("small");
    let mut cursor = Cursor::new(&volume);
    assert_eq!(cursor.block(BlockPos::new(-1, 0, 5)), Ok(STONE));
    assert_eq!(
        cursor.block(BlockPos::new(-3, 0, 5)),
        Err(Unavailable::Outside)
    );
}

struct Fixed(Access);

impl Volume for Fixed {
    fn bounds(&self) -> BoundingBox {
        BoundingBox {
            min: BlockPos::new(i32::MIN, i32::MIN, i32::MIN),
            max: BlockPos::new(i32::MAX, i32::MAX, i32::MAX),
        }
    }

    fn access(&self, _: SectionPos) -> Access {
        self.0
    }
}

impl VoxelStore for Fixed {
    fn voxel(&self, _: BlockPos) -> BlockStateId {
        STONE
    }
}

impl VoxelStoreMut for Fixed {
    fn set_voxel(&mut self, _: BlockPos, _: BlockStateId) -> BlockStateId {
        STONE
    }
}

struct Policy(Access);

impl AccessPolicy for Policy {
    fn access(&self, _: SectionPos) -> Access {
        self.0
    }
}

#[test]
fn a_restriction_answers_the_weaker_of_store_and_policy() {
    let at = BlockPos::new(3, -70, 9);
    for store in WEAKEST_FIRST {
        for policy in WEAKEST_FIRST {
            let met = store.meet(policy);
            let mut inner = Fixed(store);
            let mut view = Restricted::new(&mut inner, Policy(policy));
            let case = format!("store {store:?}, policy {policy:?}");
            assert_eq!(view.access(SectionPos::from(at)), met, "{case}");
            assert_eq!(view.block(at), met.check_read().map(|()| STONE), "{case}");
            assert_eq!(
                view.set_block(at, AIR),
                met.check_write().map(|()| STONE),
                "{case}"
            );
        }
    }
}

type Seen = Vec<((i32, i32, i32), Result<(), Unavailable>)>;

fn visited(volume: &SparseVolume, area: SectionBox, order: VisitOrder, missing: Missing) -> Seen {
    let mut seen = Vec::new();
    let flow =
        volume.visit_sections::<(), _>(area, VisitOptions { order, missing }, |at, section| {
            seen.push(((at.x, at.y, at.z), section.map(|_| ())));
            ControlFlow::Continue(())
        });
    assert_eq!(flow, ControlFlow::Continue(()));
    seen
}

#[test]
fn a_sparse_visit_walks_only_what_it_holds_in_the_order_asked() {
    let volume = world(&[(1, 0, 0), (0, 1, 0), (0, 0, 1), (5, 0, 0), (0, -9, 0)]);
    let area = SectionBox {
        min: SectionPos::new(0, -1, 0),
        max: SectionPos::new(1, 1, 1),
    };
    assert_eq!(
        visited(&volume, area, VisitOrder::SectionMajor, Missing::Skip),
        vec![
            ((1, 0, 0), Ok(())),
            ((0, 0, 1), Ok(())),
            ((0, 1, 0), Ok(()))
        ]
    );
    assert_eq!(
        visited(&volume, area, VisitOrder::ColumnMajor, Missing::Skip),
        vec![
            ((0, 1, 0), Ok(())),
            ((0, 0, 1), Ok(())),
            ((1, 0, 0), Ok(()))
        ]
    );

    let hole = Err(Unavailable::Unloaded);
    assert_eq!(
        visited(&volume, area, VisitOrder::SectionMajor, Missing::Report),
        vec![
            ((0, -1, 0), hole),
            ((1, -1, 0), hole),
            ((0, -1, 1), hole),
            ((1, -1, 1), hole),
            ((0, 0, 0), hole),
            ((1, 0, 0), Ok(())),
            ((0, 0, 1), Ok(())),
            ((1, 0, 1), hole),
            ((0, 1, 0), Ok(())),
            ((1, 1, 0), hole),
            ((0, 1, 1), hole),
            ((1, 1, 1), hole),
        ]
    );

    let mut calls = 0;
    let options = VisitOptions {
        order: VisitOrder::Any,
        missing: Missing::Skip,
    };
    let stopped = volume.visit_sections(area, options, |at, _| {
        calls += 1;
        assert!(area.contains(at));
        ControlFlow::Break(())
    });
    assert_eq!((stopped, calls), (ControlFlow::Break(()), 1));
}

#[test]
fn a_sparse_visit_stays_inside_the_bounds_and_survives_an_empty_area() {
    let mut volume = SparseVolume::new(BoundingBox {
        min: BlockPos::new(0, 0, 0),
        max: BlockPos::new(31, 15, 15),
    });
    assert!(volume.insert(SectionPos::new(1, 0, 0), STONE));
    assert!(!volume.insert(SectionPos::new(2, 0, 0), STONE));
    let area = SectionBox {
        min: SectionPos::new(0, 0, 0),
        max: SectionPos::new(3, 0, 0),
    };
    assert_eq!(
        visited(&volume, area, VisitOrder::Any, Missing::Skip),
        vec![((1, 0, 0), Ok(()))]
    );
    assert_eq!(
        visited(&volume, area, VisitOrder::SectionMajor, Missing::Report),
        vec![
            ((0, 0, 0), Err(Unavailable::Unloaded)),
            ((1, 0, 0), Ok(())),
            ((2, 0, 0), Err(Unavailable::Outside)),
            ((3, 0, 0), Err(Unavailable::Outside)),
        ]
    );

    let inverted = SectionBox {
        min: SectionPos::new(1, 0, 0),
        max: SectionPos::new(0, 0, 0),
    };
    for missing in [Missing::Skip, Missing::Report] {
        for order in [
            VisitOrder::Any,
            VisitOrder::SectionMajor,
            VisitOrder::ColumnMajor,
        ] {
            assert!(visited(&volume, inverted, order, missing).is_empty());
        }
    }
}
