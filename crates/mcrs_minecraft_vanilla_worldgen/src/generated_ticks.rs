use std::hash::Hash;

use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_region_disk::column_ticks::{ColumnTicks, StoredTick};
use mcrs_minecraft_simulation::{PendingTick, TickPriority};
use mcrs_minecraft_worldgen_feature::placer::GenerationKinds;
use mcrs_voxel::BlockPos;
use rustc_hash::FxHashSet;

pub(crate) fn column_ticks(
    ticks: &[PendingTick<GenerationKinds>],
    definitions: &BlockDefinitions,
) -> ColumnTicks {
    let mut column = ColumnTicks::default();
    let mut seen_blocks = FxHashSet::default();
    let mut seen_fluids = FxHashSet::default();
    for tick in ticks {
        match *tick {
            PendingTick::Block {
                pos,
                block,
                priority,
                ..
            } => {
                if let Some(kind) = definitions.block_index(block) {
                    keep(&mut column.block, &mut seen_blocks, pos, kind, priority);
                }
            }
            PendingTick::Fluid {
                pos,
                fluid,
                priority,
                ..
            } => keep(
                &mut column.fluid,
                &mut seen_fluids,
                pos,
                fluid.id(),
                priority,
            ),
        }
    }
    column
}

fn keep<T: Copy + Eq + Hash>(
    ticks: &mut Vec<StoredTick<T>>,
    seen: &mut FxHashSet<(BlockPos, T)>,
    pos: BlockPos,
    kind: T,
    priority: TickPriority,
) {
    if seen.insert((pos, kind)) {
        ticks.push(StoredTick {
            pos,
            kind,
            delay: 0,
            priority,
        });
    }
}
