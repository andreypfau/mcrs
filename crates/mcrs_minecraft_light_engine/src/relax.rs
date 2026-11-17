use rayon::prelude::*;

use mcrs_minecraft_core::Direction;

use crate::block::LightRegistry;
use crate::field::{BlockSnapshot, CellIndex, LightField};

const SEQUENTIAL_ROUND_LIMIT: usize = 4096;

#[inline]
fn spread(
    from: CellIndex,
    field: &LightField,
    blocks: &BlockSnapshot,
    registry: &LightRegistry,
) -> ([CellIndex; 6], usize) {
    let mut raised = [0; 6];
    let mut count = 0;

    let (Some(level), Some(from_block)) = (field.get(from), blocks.get(from)) else {
        return (raised, count);
    };
    for dir in Direction::all() {
        let Some(to) = field.layout().step(from, dir) else {
            continue;
        };

        let Some(current) = field.get(to) else {
            continue;
        };
        if current.get() + 1 >= level.get() {
            continue;
        }

        let Some(to_block) = blocks.get(to) else {
            continue;
        };
        let Some(cost) = registry.attenuation(from_block, to_block, dir) else {
            continue;
        };
        let want = level.saturating_sub(cost);
        if !want.is_zero()
            && field.raise(to, want)
            && let Some(slot) = raised.get_mut(count)
        {
            *slot = to;
            count += 1;
        }
    }

    (raised, count)
}

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
        next.clear();
        if frontier.len() < SEQUENTIAL_ROUND_LIMIT {
            for &from in &frontier {
                let (raised, count) = spread(from, field, blocks, registry);
                next.extend_from_slice(raised.get(..count).unwrap_or_default());
            }
        } else {
            next.par_extend(frontier.par_iter().flat_map_iter(|&from| {
                let (raised, count) = spread(from, field, blocks, registry);
                raised.into_iter().take(count)
            }));
        }
        std::mem::swap(&mut frontier, &mut next);
    }

    rounds
}
