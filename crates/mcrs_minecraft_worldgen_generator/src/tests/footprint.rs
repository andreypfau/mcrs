//! A write outside the 3×3 region is dropped, and is a data error.
//!
//! Every generator is trusted to stay inside its footprint, and nothing in the
//! release build stops one that does not. What stops it is the debug assertion
//! in the region writer, so this file is where that assertion is shown to fire.

use std::sync::Arc;

use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_core::BlockStateId;
use mcrs_minecraft_core::SectionPos;
use mcrs_minecraft_protocol::ColumnPos;
use mcrs_minecraft_volume::{Access, BlockVolume, BlockVolumeMut, Denied, Unavailable, Volume};
use mcrs_minecraft_worldgen_feature::placer::WorldGenVolume;

use crate::ColumnBlocks;
use crate::stages::ColumnRegion;
use crate::staging::RegionSnapshots;
use crate::tests::{
    bare_fill_context, build_beta_router, corpus, corpus_state_of, flat_snapshot, region_of,
};

const CENTRE: ColumnPos = ColumnPos { x: 0, z: 0 };
const Y_SECTIONS: [i32; 1] = [0];

fn empty_region() -> RegionSnapshots {
    let y_sections: Arc<[i32]> = Y_SECTIONS.as_slice().into();
    region_of(CENTRE, |col| {
        flat_snapshot(col, &y_sections, |_| None, None)
    })
}

/// A write one column past the ring: the position the reference would refuse
/// and log, and the one a generator that oversteps its footprint reaches.
#[test]
#[cfg(debug_assertions)]
#[should_panic = "left the region"]
fn a_write_past_the_ring_is_a_debug_assertion() {
    let snapshots = empty_region();
    let blocks = ColumnBlocks::new(&Y_SECTIONS);
    let ctx = bare_fill_context(build_beta_router());
    let mut region = ColumnRegion::new(&snapshots, &blocks, &ctx);
    region.set(BlockPos::new(32, 8, 0), VoxelId::default());
}

#[test]
#[cfg(debug_assertions)]
#[should_panic = "left the region"]
fn a_read_past_the_ring_is_a_debug_assertion() {
    let snapshots = empty_region();
    let blocks = ColumnBlocks::new(&Y_SECTIONS);
    let ctx = bare_fill_context(build_beta_router());
    let region = ColumnRegion::new(&snapshots, &blocks, &ctx);
    let _ = region.get(BlockPos::new(0, 8, -17));
}

/// Every corner of the ring is inside the footprint, so neither the volume nor the
/// assertion refuses it, and one block further out is refused.
#[test]
fn the_region_grants_its_corners_and_refuses_one_block_past_them() {
    let snapshots = empty_region();
    let blocks = ColumnBlocks::new(&Y_SECTIONS);
    let ctx = bare_fill_context(build_beta_router());
    let mut region = ColumnRegion::new(&snapshots, &blocks, &ctx);
    let state = corpus().default_state("minecraft:stone");
    let (bottom, top) = (0, 15);
    let corners: [((i32, i32), &[(i32, i32)]); 4] = [
        ((-16, -16), &[(-1, 0), (0, -1)]),
        ((31, 31), &[(1, 0), (0, 1)]),
        ((-1, 31), &[(0, 1)]),
        ((31, -1), &[(1, 0)]),
    ];
    for ((x, z), outward) in corners {
        for y in [bottom, top] {
            let at = BlockPos::new(x, y, z);
            assert_eq!(region.access(SectionPos::from(at)), Access::Write, "{at}");
            assert_eq!(region.set_block(at, state), Ok(BlockStateId::AIR), "{at}");
            assert_eq!(region.block(at), Ok(state), "{at}");
            region.set(at, VoxelId(state.number()));
            assert_eq!(region.get(at), VoxelId(state.number()), "{at}");
        }
        for &(dx, dz) in outward {
            let past = BlockPos::new(x + dx, bottom, z + dz);
            assert_eq!(
                region.access(SectionPos::from(past)),
                Access::Outside,
                "{past}"
            );
            assert_eq!(region.block(past), Err(Unavailable::Outside), "{past}");
        }
        for past in [
            BlockPos::new(x, bottom - 1, z),
            BlockPos::new(x, top + 1, z),
        ] {
            assert_eq!(
                region.access(SectionPos::from(past)),
                Access::Outside,
                "{past}"
            );
            assert_eq!(region.block(past), Err(Unavailable::Outside), "{past}");
            assert_eq!(
                region.set_block(past, state),
                Err(Denied::Outside),
                "{past}"
            );
        }
    }

    let mut written: Vec<_> = region
        .finish()
        .iter()
        .map(|(col, delta)| ((col.x, col.z), delta.writes.len()))
        .collect();
    written.sort();
    assert_eq!(
        written,
        vec![((-1, -1), 2), ((-1, 1), 2), ((1, -1), 2), ((1, 1), 2)]
    );
}

/// A write answers what it replaced: the filled block first, then the unit's own
/// earlier write, in a ring column and in the centre alike.
#[test]
fn a_rewrite_answers_the_earlier_write() {
    let stone = VoxelId::from(corpus().default_state("minecraft:stone").number());
    let y_sections: Arc<[i32]> = Y_SECTIONS.as_slice().into();
    let snapshots = region_of(CENTRE, |col| {
        flat_snapshot(col, &y_sections, |_| Some(stone), None)
    });
    let blocks = ColumnBlocks::new(&Y_SECTIONS);
    blocks.unpack(&snapshots[4].sections);
    let ctx = bare_fill_context(build_beta_router());
    let mut region = ColumnRegion::new(&snapshots, &blocks, &ctx);
    let (first, second) = (
        corpus().default_state("minecraft:dirt"),
        corpus().default_state("minecraft:gravel"),
    );
    for at in [BlockPos::new(20, 4, -3), BlockPos::new(5, 4, 9)] {
        assert_eq!(
            region.set_block(at, first),
            Ok(corpus_state_of(stone)),
            "{at}"
        );
        assert_eq!(region.set_block(at, second), Ok(first), "{at}");
        assert_eq!(region.block(at), Ok(second), "{at}");
    }
}

/// A tick travels in the delta of the column that holds it; one past the ring is refused.
#[test]
fn a_tick_goes_to_the_column_it_falls_in() {
    use mcrs_minecraft_block::keys::Fluid;
    use mcrs_minecraft_simulation::{PendingTick, ScheduleTicks, TickPriority};

    let snapshots = empty_region();
    let blocks = ColumnBlocks::new(&Y_SECTIONS);
    let ctx = bare_fill_context(build_beta_router());
    let mut region = ColumnRegion::new(&snapshots, &blocks, &ctx);
    let tick = |pos| PendingTick::Fluid {
        pos,
        fluid: Fluid::Water,
        delay: 0,
        priority: TickPriority::Normal,
    };
    let east = BlockPos::new(20, 8, 3);
    assert_eq!(region.schedule_tick(tick(east)), Ok(()));
    assert_eq!(
        region.schedule_tick(tick(BlockPos::new(32, 8, 0))),
        Err(Denied::Outside)
    );

    let deltas = region.finish();
    assert_eq!(deltas.len(), 1);
    let (col, delta) = &deltas[0];
    assert_eq!(*col, ColumnPos::new(1, 0));
    assert_eq!(delta.ticks, [tick(east)]);
}
