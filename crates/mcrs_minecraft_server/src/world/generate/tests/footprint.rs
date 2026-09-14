//! A write outside the 3×3 region is dropped, and is a data error.
//!
//! Every generator is trusted to stay inside its footprint, and nothing in the
//! release build stops one that does not. What stops it is the debug assertion
//! in the region writer, so this file is where that assertion is shown to fire.

use std::sync::Arc;

use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_protocol::ColumnPos;
use mcrs_voxel_storage::{Blocks, BlocksMut, VoxelId};

use crate::world::generate::ColumnBlocks;
use crate::world::generate::stages::ColumnRegion;
use crate::world::generate::staging::RegionSnapshots;
use crate::world::generate::tests::{
    bare_fill_context, build_beta_router, flat_snapshot, region_of,
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

/// The far corner of the ring is inside the footprint, so the same assertion
/// must not fire one block short of the edge.
#[test]
fn the_corners_of_the_ring_are_inside_the_footprint() {
    let snapshots = empty_region();
    let blocks = ColumnBlocks::new(&Y_SECTIONS);
    let ctx = bare_fill_context(build_beta_router());
    let mut region = ColumnRegion::new(&snapshots, &blocks, &ctx);
    for (x, z) in [(-16, -16), (31, 31), (-1, 31), (31, -1)] {
        region.set(BlockPos::new(x, 8, z), VoxelId::default());
        let _ = region.get(BlockPos::new(x, 8, z));
    }
}
