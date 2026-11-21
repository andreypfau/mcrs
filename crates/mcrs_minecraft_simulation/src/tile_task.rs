use core::cell::Cell;
use core::ops::Range;

use crate::SimulationVolume;
use crate::behaviour::{BlockBehaviour, BlockFacts, MemoryPartition};
use crate::cx::{Cx, TileCx};
use crate::footprints::Footprints;
use crate::output::StepOutput;
use crate::overlay::{OverlaidVolume, Overlay};
use crate::policy::OrderingPolicy;
use crate::random_tick::block_tile;
use crate::region::{Region, Touch, cell_key, colour};
use crate::scalar::GameTick;
use crate::shard::{BlockShard, Env};
use crate::tick_queue::Kinds;
use mcrs_voxel::{BlockPos, ColumnPos, SectionPos};
use rustc_hash::{FxHashMap, FxHashSet};

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

pub(crate) struct Produced {
    pub(crate) index: usize,
    pub(crate) reads: Touch,
    pub(crate) writes: Touch,
    pub(crate) blocks: Range<usize>,
    pub(crate) outbound: Range<usize>,
    pub(crate) delivered: Range<usize>,
    pub(crate) destroyed: Range<usize>,
    pub(crate) chains: Range<usize>,
}

pub(crate) struct TileTask<K: Kinds, M> {
    tile: Option<ColumnPos>,
    items: Vec<usize>,
    memory: Option<M>,
    before: Option<M>,
    shard: BlockShard<K>,
    out: StepOutput<K>,
    overlay: Overlay,
    reruns: Vec<usize>,
    produced: Vec<Produced>,
    consumed: Vec<(BlockPos, K::Block)>,
}

impl<K: Kinds, M: MemoryPartition + Default + Clone> TileTask<K, M> {
    fn new(tile: Option<ColumnPos>, items: Vec<usize>) -> Self {
        Self {
            tile,
            items,
            memory: None,
            before: None,
            shard: BlockShard::new(),
            out: StepOutput::default(),
            overlay: Overlay::default(),
            reruns: Vec::new(),
            produced: Vec::new(),
            consumed: Vec::new(),
        }
    }

    fn plan(tasks: &mut Vec<Self>, positions: impl Iterator<Item = (usize, BlockPos)>) {
        let mut groups: Vec<(ColumnPos, Vec<usize>)> = Vec::new();
        let mut slots: FxHashMap<ColumnPos, usize> = FxHashMap::default();
        for (index, at) in positions {
            let tile = block_tile(ColumnPos::from(SectionPos::from(at)));
            let slot = *slots.entry(tile).or_insert_with(|| {
                groups.push((tile, Vec::new()));
                groups.len().saturating_sub(1)
            });
            if let Some((_, items)) = groups.get_mut(slot) {
                items.push(index);
            }
        }
        groups.sort_unstable_by_key(|(tile, _)| (colour(*tile), *tile));
        tasks.extend(
            groups
                .into_iter()
                .map(|(tile, items)| Self::new(Some(tile), items)),
        );
    }

    fn region(&self) -> Region {
        self.tile.map_or(Region::WHOLE, Region::of)
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "each parameter is a distinct part of the world that the tile reads"
    )]
    fn run<V, F, B, P, R>(
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
        let region = self.region();
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
            consumed: core::mem::take(&mut self.consumed),
            reads: Touch::EMPTY,
            writes: Touch::EMPTY,
        });
        for &index in &self.items {
            cx.tile_item(index, &mut self.reruns, &mut self.produced, run);
        }
        if let Some(task) = cx.task.take() {
            self.consumed = task.consumed;
        }
        self.before = cx.task_memory.take();
    }

    fn publish<V: SimulationVolume>(&mut self, volume: &mut V, memory: &mut M) {
        if let Some(part) = self.memory.take() {
            memory.merge(part);
        }
        for write in &self.out.writes {
            let _ = volume.set_block(write.pos, write.new);
        }
        self.overlay.clear();
    }

    fn unpublish<V: SimulationVolume>(&self, volume: &mut V) {
        for write in self.out.writes.iter().rev() {
            let _ = volume.set_block(write.pos, write.old);
        }
    }
}

pub(crate) enum Tiled {
    Done { reruns: Vec<usize>, left: u32 },
    Redo { left: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Sequence,
    Colour,
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

fn mark_hot<K: Kinds>(shard: &mut BlockShard<K>, hot_ticks: i64, now: GameTick, at: &[BlockPos]) {
    if at.is_empty() {
        return;
    }
    if shard.hot.len() >= HOT_SWEEP {
        shard
            .hot
            .retain(|_, last| now.0.saturating_sub(*last) <= hot_ticks);
    }
    for at in at {
        shard.hot.insert(cell_key(*at), now.0);
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a distinct part of the world that the tiles read or write"
)]
pub(crate) fn run_tiled<K, V, F, B, P, S, R, Before>(
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
    mode: Mode,
    run: &R,
    before: &Before,
) -> Tiled
where
    K: Kinds,
    V: SimulationVolume + Sync,
    F: BlockFacts<K> + Sync,
    B: BlockBehaviour<K>,
    P: OrderingPolicy,
    S: Spread,
    R: Fn(&mut Cx<'_, K, OverlaidVolume<'_, V>, F, B>, usize) + Sync,
    Before: Fn(&mut BlockShard<K>, usize),
{
    let hot_ticks = env.config.hot_ticks;
    let hot: Vec<usize> = positions
        .iter()
        .enumerate()
        .filter(|(_, at)| mode == Mode::Sequence && is_hot(shard, hot_ticks, now, **at))
        .map(|(index, _)| index)
        .collect();
    let mut tasks: Vec<TileTask<K, B::Memory>> = Vec::new();
    if !hot.is_empty() {
        tasks.push(TileTask::new(None, hot.clone()));
    }
    TileTask::plan(
        &mut tasks,
        positions
            .iter()
            .enumerate()
            .filter(|(index, _)| hot.binary_search(index).is_err())
            .map(|(index, at)| (index, *at)),
    );
    for phase in tasks.chunk_by_mut(|a, b| a.tile.map(colour) == b.tile.map(colour)) {
        lend_memory(memory, phase);
        let (world, base) = (&*volume, &*shard);
        spread.spread(phase, &|task: &mut TileTask<K, B::Memory>| {
            task.run(world, base, env, shared, footprints, now, run);
        });
        for task in phase.iter_mut() {
            task.publish(volume, memory);
        }
    }
    let mut overflowed: Vec<usize> = tasks
        .iter()
        .flat_map(|task| task.reruns.iter().copied())
        .collect();
    overflowed.sort_unstable();
    let mut reruns = Vec::new();
    if !overflowed.is_empty() {
        let mut task = TileTask::new(None, overflowed.clone());
        lend_memory(memory, core::slice::from_mut(&mut task));
        task.run(&*volume, &*shard, env, shared, footprints, now, run);
        task.publish(volume, memory);
        reruns = core::mem::take(&mut task.reruns);
        tasks.push(task);
    }
    let left = u32::try_from(overflowed.len()).unwrap_or(u32::MAX);
    if mode == Mode::Colour {
        finish(
            &mut tasks, shard, volume, env, memory, shared, out, now, before, mode,
        );
        return Tiled::Done { reruns, left };
    }
    let conflicts = conflicts(&tasks, positions);
    if !conflicts.is_empty() {
        for task in tasks.iter().rev() {
            task.unpublish(volume);
        }
        restore_memory(memory, &mut tasks);
        shard.chain.reset();
        mark_hot(shard, hot_ticks, now, &conflicts);
        out.report.tile_redos = out.report.tile_redos.saturating_add(1);
        return Tiled::Redo { left };
    }
    finish(
        &mut tasks, shard, volume, env, memory, shared, out, now, before, mode,
    );
    let overflowed: Vec<BlockPos> = overflowed
        .iter()
        .filter_map(|&index| positions.get(index))
        .copied()
        .collect();
    mark_hot(shard, hot_ticks, now, &overflowed);
    Tiled::Done { reruns, left }
}

#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a distinct part of the world that the commit writes"
)]
fn finish<K, V, F, B, P, Before>(
    tasks: &mut [TileTask<K, B::Memory>],
    shard: &mut BlockShard<K>,
    volume: &mut V,
    env: &Env<'_, F, B, P>,
    memory: &mut B::Memory,
    shared: &B::Shared,
    out: &mut StepOutput<K>,
    now: GameTick,
    before: &Before,
    mode: Mode,
) where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    P: OrderingPolicy,
    Before: Fn(&mut BlockShard<K>, usize),
{
    let mut order: Vec<(usize, usize, usize)> = tasks
        .iter()
        .enumerate()
        .flat_map(|(slot, task)| {
            task.produced
                .iter()
                .enumerate()
                .map(move |(entry, produced)| (produced.index, slot, entry))
        })
        .collect();
    if mode == Mode::Sequence {
        order.sort_unstable();
    }
    for (item, slot, entry) in order {
        let Some(task) = tasks.get(slot) else {
            continue;
        };
        let Some(produced) = task.produced.get(entry) else {
            continue;
        };
        if let Some(writes) = task.out.writes.get(produced.blocks.clone()) {
            out.writes.extend_from_slice(writes);
        }
        if let Some(delivered) = task.out.delivered.get(produced.delivered.clone()) {
            out.delivered.extend_from_slice(delivered);
        }
        if let Some(destroyed) = task.out.destroyed.get(produced.destroyed.clone()) {
            out.destroyed.extend_from_slice(destroyed);
        }
        if let Some(chains) = task.out.chains.get(produced.chains.clone()) {
            out.chains.extend_from_slice(chains);
        }
        before(shard, item);
        let mut cx = Cx::new(shard, volume, out, env, Some(&mut *memory), shared, now);
        if let Some(messages) = task.out.outbound.get(produced.outbound.clone()) {
            for message in messages {
                let mut message = *message;
                message.stamp = cx.shard.clock.stamp(now);
                cx.apply(message);
            }
        }
    }
    for task in tasks.iter_mut() {
        for consumed in task.consumed.drain(..) {
            shard.ticking.remove(&consumed);
        }
        out.report.absorb(task.out.report);
    }
}

struct Seen<'a> {
    task: usize,
    produced: &'a Produced,
}

fn out_of_order(a: &Seen<'_>, b: &Seen<'_>) -> bool {
    if a.task == b.task {
        return false;
    }
    let (early, late) = if a.produced.index < b.produced.index {
        (a, b)
    } else {
        (b, a)
    };
    late.task < early.task
        && (early.produced.reads.intersects(&late.produced.writes)
            || early.produced.writes.intersects(&late.produced.reads))
}

fn conflicts<K: Kinds, M>(tasks: &[TileTask<K, M>], positions: &[BlockPos]) -> Vec<BlockPos> {
    let mut buckets: FxHashMap<u64, Vec<Seen<'_>>> = FxHashMap::default();
    for (task, produced) in tasks
        .iter()
        .enumerate()
        .flat_map(|(task, held)| held.produced.iter().map(move |produced| (task, produced)))
    {
        for cell in produced.reads.cells() {
            buckets
                .entry(cell)
                .or_default()
                .push(Seen { task, produced });
        }
    }
    let mut found = Vec::new();
    for seen in buckets.values() {
        let Some(first) = seen.first() else {
            continue;
        };
        if seen.iter().all(|other| other.task == first.task) {
            continue;
        }
        for (n, a) in seen.iter().enumerate() {
            for b in seen.iter().skip(n.saturating_add(1)) {
                if out_of_order(a, b) {
                    found.extend(positions.get(a.produced.index));
                    found.extend(positions.get(b.produced.index));
                }
            }
        }
    }
    found
}

fn lend_memory<K: Kinds, M: MemoryPartition + Default>(
    memory: &mut M,
    phase: &mut [TileTask<K, M>],
) {
    let mut parts: FxHashMap<u64, M> = memory.split().into_iter().collect();
    for task in phase.iter_mut() {
        let mut part = M::default();
        match task.tile {
            Some(tile) => {
                for cell in Region::of(tile).cells() {
                    if let Some(found) = parts.remove(&cell) {
                        part.merge(found);
                    }
                }
            }
            None => {
                for (_, found) in parts.drain() {
                    part.merge(found);
                }
            }
        }
        task.memory = Some(part);
    }
    for (_, part) in parts {
        memory.merge(part);
    }
}

fn restore_memory<K: Kinds, M: MemoryPartition + Default>(
    memory: &mut M,
    tasks: &mut [TileTask<K, M>],
) {
    let mut restored: FxHashMap<u64, M> = FxHashMap::default();
    let mut cleared: FxHashSet<u64> = FxHashSet::default();
    let mut clear_all = false;
    for task in tasks.iter_mut() {
        let Some(mut before) = task.before.take() else {
            continue;
        };
        for (cell, part) in before.split() {
            if !clear_all && !cleared.contains(&cell) {
                restored.entry(cell).or_insert(part);
            }
        }
        match task.tile {
            Some(tile) => {
                for cell in Region::of(tile).cells() {
                    if !restored.contains_key(&cell) {
                        cleared.insert(cell);
                    }
                }
            }
            None => clear_all = true,
        }
    }
    for (cell, part) in memory.split() {
        if clear_all || cleared.contains(&cell) || restored.contains_key(&cell) {
            continue;
        }
        memory.merge(part);
    }
    for (_, part) in restored {
        memory.merge(part);
    }
}
