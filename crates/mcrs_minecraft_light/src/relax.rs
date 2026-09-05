//! Light is the least fixed point of a monotone map, so a purely rising
//! relaxation reaches the same answer whatever order the updates happen in.
//! That is why there is no domain decomposition here, no halo, no ownership and
//! no barrier: threads write wherever they like through `fetch_max`, and the
//! unit of parallelism is a cell of the frontier rather than a section.

use rayon::prelude::*;

use mcrs_voxel_math::Direction;

use crate::block::LightRegistry;
use crate::field::{BlockSnapshot, CellIndex, LightField};

/// Below this many cells a round is run on one thread.
///
/// Handing work to a thread pool costs tens of microseconds, which is more than
/// a small frontier costs to walk. A single torch spends most of its fifteen
/// rounds well under this, so the whole edit stays on one thread.
const SEQUENTIAL_ROUND_LIMIT: usize = 4096;

/// Raises the neighbours of one cell, reporting those that rose.
///
/// Returns at most six, so the caller needs no allocation.
#[inline]
fn spread(
    from: CellIndex,
    field: &LightField,
    blocks: &BlockSnapshot,
    registry: &LightRegistry,
) -> ([CellIndex; 6], usize) {
    let mut raised = [0; 6];
    let mut count = 0;

    let level = field.get(from);
    let from_block = blocks.get(from);
    for dir in Direction::all() {
        let Some(to) = field.layout().step(from, dir) else {
            continue;
        };

        // Bail out before reading any block. Every step costs at least one
        // level, so a neighbour already this bright can never be improved — and
        // this is what keeps block reads down to roughly one per cell written.
        if field.get(to).get() + 1 >= level.get() {
            continue;
        }

        let Some(cost) = registry.attenuation(from_block, blocks.get(to), dir) else {
            continue;
        };
        let want = level.saturating_sub(cost);
        if !want.is_zero() && field.raise(to, want) {
            raised[count] = to;
            count += 1;
        }
    }

    (raised, count)
}

/// Raises light until nothing changes. Returns the number of rounds taken.
pub fn relax(
    field: &LightField,
    blocks: &BlockSnapshot,
    registry: &LightRegistry,
    seeds: Vec<CellIndex>,
) -> usize {
    let mut frontier = seeds;
    let mut next = Vec::new();
    let mut rounds = 0;

    while !frontier.is_empty() {
        rounds += 1;
        if frontier.len() < SEQUENTIAL_ROUND_LIMIT {
            next.clear();
            for &from in &frontier {
                let (raised, count) = spread(from, field, blocks, registry);
                next.extend_from_slice(&raised[..count]);
            }
            std::mem::swap(&mut frontier, &mut next);
        } else {
            frontier = frontier
                .par_iter()
                .flat_map_iter(|&from| {
                    let (raised, count) = spread(from, field, blocks, registry);
                    (0..count).map(move |i| raised[i])
                })
                .collect();
        }
    }

    rounds
}
