use std::collections::BTreeMap;
use std::ops::RangeInclusive;
use std::sync::Arc;

use mcrs_minecraft_worldgen_density::aquifer_cells::AquiferCells;
use mcrs_voxel::ColumnPos;

use crate::ColumnBlocks;
use crate::stages::{FillContext, fill_column, merge_column, run_region};
use crate::staging::{FilledSnapshot, StagingStore};
use crate::task::CancellationToken;

#[must_use]
pub fn generate_area(
    ctx: &FillContext,
    min: ColumnPos,
    max: ColumnPos,
) -> BTreeMap<ColumnPos, FilledSnapshot> {
    if min.x > max.x || min.z > max.z {
        return BTreeMap::new();
    }
    let cancel = CancellationToken::new();
    let mut store = StagingStore::default();
    let rungs = ctx.rungs();
    let mut buffer = ColumnBlocks::new(&ctx.y_sections);
    let mut cells = AquiferCells::default();
    for col in Bounds::around(min, max, halo(rungs)).columns() {
        if let Some(snapshot) = fill_column(ctx, col, &mut buffer, None, &mut cells, &cancel) {
            store.insert_filled(snapshot);
        }
    }
    let mut kept = halo(rungs);
    for rung in 0..rungs {
        let reach = halo(rungs.saturating_sub(rung).saturating_sub(1));
        for col in Bounds::around(min, max, reach.saturating_add(1)).columns() {
            let Some(region) = store.region(col, rung) else {
                continue;
            };
            for (target, delta) in run_region(ctx, &region, rung) {
                store.push_delta(col, rung, target, delta);
            }
        }
        for col in Bounds::around(min, max, reach).columns() {
            let deltas = store.deltas(col, rung);
            let Some(base) = store.base(col, rung) else {
                continue;
            };
            let merged = merge_column(
                base,
                &deltas,
                &ctx.predicates,
                ctx.features()
                    .map(|program| &program.world.has_block_entity),
            );
            store.insert_staged(col, rung, Arc::new(merged));
        }
        let merged = Bounds::around(min, max, reach);
        for col in Bounds::around(min, max, kept).columns() {
            if merged.contains(col) {
                store.release_merged_below(col, rung);
            } else {
                store.forget(col);
            }
        }
        kept = reach;
    }
    Bounds::around(min, max, 0)
        .columns()
        .filter_map(|col| Some((col, Arc::unwrap_or_clone(store.take_merged(col)?))))
        .collect()
}

fn halo(rungs: usize) -> i32 {
    i32::try_from(rungs).unwrap_or(i32::MAX).saturating_mul(2)
}

struct Bounds {
    x: RangeInclusive<i32>,
    z: RangeInclusive<i32>,
}

impl Bounds {
    fn around(min: ColumnPos, max: ColumnPos, margin: i32) -> Self {
        Bounds {
            x: min.x.saturating_sub(margin)..=max.x.saturating_add(margin),
            z: min.z.saturating_sub(margin)..=max.z.saturating_add(margin),
        }
    }

    fn contains(&self, col: ColumnPos) -> bool {
        self.x.contains(&col.x) && self.z.contains(&col.z)
    }

    fn columns(&self) -> impl Iterator<Item = ColumnPos> + use<> {
        let zs = self.z.clone();
        self.x
            .clone()
            .flat_map(move |x| zs.clone().map(move |z| ColumnPos::new(x, z)))
    }
}
