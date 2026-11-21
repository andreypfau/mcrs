use core::cell::Cell;

use crate::SimulationVolume;
use crate::behaviour::{BlockBehaviour, BlockFacts, MemoryPartition};
use crate::cx::{Cx, TileCx};
use crate::footprints::Footprints;
use crate::output::StepOutput;
use crate::overlay::{OverlaidVolume, Overlay};
use crate::policy::OrderingPolicy;
use crate::random_tick::block_tile;
use crate::region::{Region, cell_key, colour};
use crate::scalar::GameTick;
use crate::shard::{BlockShard, Env};
use crate::tick_queue::Kinds;
use mcrs_voxel::{BlockPos, ColumnPos, SectionPos};
use rustc_hash::FxHashMap;

pub trait Spread {
    fn spread<T: Send, R: Fn(&mut T) + Sync>(&self, items: &mut [T], run: &R);
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Serial;

impl Spread for Serial {
    fn spread<T: Send, R: Fn(&mut T) + Sync>(&self, items: &mut [T], run: &R) {
        for item in items {
            run(item);
        }
    }
}

pub(crate) struct TileTask<K: Kinds, M> {
    pub(crate) tile: ColumnPos,
    pub(crate) items: Vec<usize>,
    pub(crate) memory: Option<M>,
    shard: BlockShard<K>,
    out: StepOutput<K>,
    overlay: Overlay,
    reruns: Vec<usize>,
}

impl<K: Kinds, M: MemoryPartition + Default> TileTask<K, M> {
    fn new(tile: ColumnPos) -> Self {
        Self {
            tile,
            items: Vec::new(),
            memory: None,
            shard: BlockShard::new(),
            out: StepOutput::default(),
            overlay: Overlay::default(),
            reruns: Vec::new(),
        }
    }

    pub(crate) fn plan(tasks: &mut Vec<Self>, positions: impl Iterator<Item = (usize, BlockPos)>) {
        let mut groups: Vec<(ColumnPos, Vec<usize>)> = Vec::new();
        for (index, at) in positions {
            let tile = block_tile(ColumnPos::from(SectionPos::from(at)));
            match groups.iter_mut().find(|(known, _)| *known == tile) {
                Some((_, items)) => items.push(index),
                None => groups.push((tile, vec![index])),
            }
        }
        groups.sort_unstable_by_key(|(tile, _)| (colour(*tile), *tile));
        tasks.truncate(groups.len());
        for (used, (tile, items)) in groups.into_iter().enumerate() {
            match tasks.get_mut(used) {
                Some(task) => {
                    task.tile = tile;
                    task.items = items;
                }
                None => {
                    let mut task = Self::new(tile);
                    task.items = items;
                    tasks.push(task);
                }
            }
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "each parameter is a distinct part of the world that the tile reads"
    )]
    pub(crate) fn run<V, F, B, P, R>(
        &mut self,
        world: &V,
        base: &BlockShard<K>,
        env: &Env<'_, F, B, P>,
        shared: &B::Shared,
        footprints: &Footprints,
        now: GameTick,
        run: &R,
    ) where
        V: SimulationVolume,
        F: BlockFacts<K>,
        B: BlockBehaviour<K, Memory = M>,
        P: OrderingPolicy,
        R: Fn(&mut Cx<'_, K, OverlaidVolume<'_, V>, F, B>, usize),
    {
        self.shard.chain.reset();
        let region = Region::of(self.tile);
        let mut volume = OverlaidVolume {
            world,
            region,
            overlay: &mut self.overlay,
            last: Cell::default(),
        };
        let mut cx = Cx::new(
            &mut self.shard,
            &mut volume,
            &mut self.out,
            env,
            self.memory.as_mut(),
            shared,
            now,
        );
        cx.task = Some(TileCx {
            region,
            base,
            footprints,
            overflowed: false,
        });
        for &index in &self.items {
            cx.tile_item(index, &mut self.reruns, run);
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "each parameter is a distinct part of the world that the commit writes"
    )]
    pub(crate) fn commit<V, F, B, P>(
        &mut self,
        shard: &mut BlockShard<K>,
        volume: &mut V,
        env: &Env<'_, F, B, P>,
        memory: &mut B::Memory,
        shared: &B::Shared,
        out: &mut StepOutput<K>,
        now: GameTick,
        reruns: &mut Vec<usize>,
    ) where
        V: SimulationVolume,
        F: BlockFacts<K>,
        B: BlockBehaviour<K, Memory = M>,
        P: OrderingPolicy,
    {
        if let Some(part) = self.memory.take() {
            memory.merge(part);
        }
        for write in &self.out.writes {
            let _ = volume.set_block(write.pos, write.new);
        }
        self.overlay.clear();
        out.writes.append(&mut self.out.writes);
        out.destroyed.append(&mut self.out.destroyed);
        out.delivered.append(&mut self.out.delivered);
        out.report.absorb(self.out.report);
        let mut cx = Cx::new(shard, volume, out, env, Some(memory), shared, now);
        for mut message in self.out.outbound.drain(..) {
            message.stamp = cx.shard.clock.stamp(now);
            cx.apply(message);
        }
        self.out.clear();
        reruns.append(&mut self.reruns);
    }
}

pub(crate) struct Split {
    pub(crate) hot: Vec<usize>,
    pub(crate) reruns: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HotCells {
    Use,
    Ignore,
}

const HOT_SWEEP: usize = 4096;

pub(crate) fn is_hot<K: Kinds>(
    shard: &BlockShard<K>,
    hot_ticks: i64,
    now: GameTick,
    at: BlockPos,
) -> bool {
    shard
        .hot
        .get(&cell_key(at))
        .is_some_and(|&last| now.0.saturating_sub(last) <= hot_ticks)
}

fn mark_hot<K: Kinds>(shard: &mut BlockShard<K>, hot_ticks: i64, now: GameTick, at: BlockPos) {
    if shard.hot.len() >= HOT_SWEEP {
        shard
            .hot
            .retain(|_, last| now.0.saturating_sub(*last) <= hot_ticks);
    }
    shard.hot.insert(cell_key(at), now.0);
}

#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a distinct part of the world that the tiles read or write"
)]
pub(crate) fn run_tiled<K, V, F, B, P, S, R>(
    shard: &mut BlockShard<K>,
    volume: &mut V,
    env: &Env<'_, F, B, P>,
    memory: &mut B::Memory,
    shared: &B::Shared,
    footprints: &Footprints,
    out: &mut StepOutput<K>,
    now: GameTick,
    spread: &S,
    positions: &[BlockPos],
    hot_cells: HotCells,
    run: &R,
) -> Split
where
    K: Kinds,
    V: SimulationVolume + Sync,
    F: BlockFacts<K> + Sync,
    B: BlockBehaviour<K>,
    P: OrderingPolicy,
    S: Spread,
    R: Fn(&mut Cx<'_, K, OverlaidVolume<'_, V>, F, B>, usize) + Sync,
{
    let hot_ticks = env.config.hot_ticks;
    let hot: Vec<usize> = positions
        .iter()
        .enumerate()
        .filter(|(_, at)| hot_cells == HotCells::Use && is_hot(shard, hot_ticks, now, **at))
        .map(|(index, _)| index)
        .collect();
    let mut tasks: Vec<TileTask<K, B::Memory>> = Vec::new();
    let mut reruns = core::mem::take(&mut shard.reruns);
    TileTask::plan(
        &mut tasks,
        positions
            .iter()
            .enumerate()
            .filter(|(index, _)| hot.binary_search(index).is_err())
            .map(|(index, at)| (index, *at)),
    );
    for phase in tasks.chunk_by_mut(|a, b| colour(a.tile) == colour(b.tile)) {
        lend_memory(memory, phase);
        let (world, base) = (&*volume, &*shard);
        spread.spread(phase, &|task: &mut TileTask<K, B::Memory>| {
            task.run(world, base, env, shared, footprints, now, run);
        });
        for task in phase {
            task.commit(
                shard,
                volume,
                env,
                &mut *memory,
                shared,
                out,
                now,
                &mut reruns,
            );
        }
    }
    reruns.sort_unstable();
    out.report.tile_reruns = out
        .report
        .tile_reruns
        .saturating_add(u32::try_from(reruns.len()).unwrap_or(u32::MAX));
    if hot_cells == HotCells::Use {
        for at in reruns.iter().filter_map(|&index| positions.get(index)) {
            mark_hot(shard, hot_ticks, now, *at);
        }
    }
    let taken = core::mem::take(&mut reruns);
    shard.reruns = reruns;
    Split { hot, reruns: taken }
}

fn lend_memory<K: Kinds, M: MemoryPartition + Default>(
    memory: &mut M,
    phase: &mut [TileTask<K, M>],
) {
    let mut parts: FxHashMap<u64, M> = memory.split(cell_key).into_iter().collect();
    for task in phase.iter_mut() {
        let mut part = M::default();
        for cell in Region::of(task.tile).cells() {
            if let Some(found) = parts.remove(&cell) {
                part.merge(found);
            }
        }
        task.memory = Some(part);
    }
    for (_, part) in parts {
        memory.merge(part);
    }
}
