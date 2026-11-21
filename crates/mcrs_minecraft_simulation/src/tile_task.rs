use core::cell::Cell;
use core::ops::Range;

use crate::SimulationVolume;
use crate::behaviour::{BlockBehaviour, BlockFacts, MemoryPartition};
use crate::cx::{Cx, TileCx};
use crate::footprints::Footprints;
use crate::message::MessageBody;
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
    before: FxHashMap<u64, M>,
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
            before: FxHashMap::default(),
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
        self.before = core::mem::take(&mut cx.task_cells);
    }

    fn publish<V: SimulationVolume>(
        &mut self,
        volume: &mut V,
        memory: &mut M,
        shard: &mut BlockShard<K>,
    ) {
        if let Some(part) = self.memory.take() {
            memory.merge(part);
        }
        for write in &self.out.writes {
            let _ = volume.set_block(write.pos, write.new);
        }
        self.overlay.clear();
        for produced in &self.produced {
            let Some(messages) = self.out.outbound.get(produced.outbound.clone()) else {
                continue;
            };
            for message in messages {
                if let MessageBody::BlockTick { pos, block, .. } = message.body {
                    shard.published_ticks.insert((pos, block));
                }
            }
        }
        shard
            .published_consumed
            .extend(self.consumed.iter().copied());
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
    Serial,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Sequence,
    Colour,
}

const HOT_SWEEP: usize = 4096;
const HOT_STREAK_LIMIT: u32 = 6;

fn hot_window(hot_ticks: i64, streak: u32) -> i64 {
    hot_ticks.saturating_mul(1_i64 << streak.min(HOT_STREAK_LIMIT))
}

pub(crate) fn is_hot<K: Kinds>(
    shard: &BlockShard<K>,
    _hot_ticks: i64,
    now: GameTick,
    at: BlockPos,
) -> bool {
    shard
        .hot
        .get(&cell_key(at))
        .is_some_and(|hot| now.0 <= hot.until)
}

fn mark_hot<K: Kinds>(shard: &mut BlockShard<K>, hot_ticks: i64, now: GameTick, at: &[BlockPos]) {
    if at.is_empty() {
        return;
    }
    if shard.hot.len() >= HOT_SWEEP {
        let forget = hot_window(hot_ticks, HOT_STREAK_LIMIT);
        shard
            .hot
            .retain(|_, hot| now.0.saturating_sub(hot.until) <= forget);
    }
    for at in at {
        let hot = shard.hot.entry(cell_key(*at)).or_default();
        if hot.until < now.0 && hot.until != 0 {
            hot.streak = hot.streak.saturating_add(1);
        }
        hot.until = now.0.saturating_add(hot_window(hot_ticks, hot.streak));
    }
}

fn count(items: usize) -> u32 {
    u32::try_from(items).unwrap_or(u32::MAX)
}

#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a distinct part of the world that the tiles read or write"
)]
pub(crate) fn run_tiled<K, V, F, B, P, S, R, L, Before>(
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
    live: &L,
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
    L: Fn(&mut Cx<'_, K, V, F, B>, usize),
    Before: Fn(&mut BlockShard<K>, usize),
{
    if mode == Mode::Sequence && positions.len() < env.config.tile_threshold {
        return Tiled::Serial;
    }
    let hot_ticks = env.config.hot_ticks;
    shard.published_ticks.clear();
    shard.published_consumed.clear();
    let hot: Vec<usize> = positions
        .iter()
        .enumerate()
        .filter(|(_, at)| mode == Mode::Sequence && is_hot(shard, hot_ticks, now, **at))
        .map(|(index, _)| index)
        .collect();
    out.report.hot_items = out.report.hot_items.saturating_add(count(hot.len()));
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
            task.publish(volume, memory, shard);
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
        task.publish(volume, memory, shard);
        reruns = core::mem::take(&mut task.reruns);
        tasks.push(task);
    }
    let left = count(overflowed.len());
    let mut bad: FxHashSet<usize> = FxHashSet::default();
    if mode == Mode::Sequence {
        if !reruns.is_empty() {
            bad.extend(0..tasks.len());
        }
        undo_conflicts(
            &mut tasks, &mut bad, shard, volume, memory, out, now, hot_ticks, positions,
        );
        if !reruns.is_empty() {
            shard.published_ticks.clear();
            shard.published_consumed.clear();
            return Tiled::Redo { left };
        }
    }
    finish(
        &mut tasks,
        &bad,
        positions.len(),
        shard,
        volume,
        env,
        memory,
        shared,
        out,
        now,
        live,
        before,
        mode,
    );
    let overflowed: Vec<BlockPos> = overflowed
        .iter()
        .filter_map(|&index| positions.get(index))
        .copied()
        .collect();
    if mode == Mode::Sequence {
        mark_hot(shard, hot_ticks, now, &overflowed);
    }
    Tiled::Done { reruns, left }
}

#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a distinct part of the world that the undo restores"
)]
fn undo_conflicts<K, V, M>(
    tasks: &mut [TileTask<K, M>],
    bad: &mut FxHashSet<usize>,
    shard: &mut BlockShard<K>,
    volume: &mut V,
    memory: &mut M,
    out: &mut StepOutput<K>,
    now: GameTick,
    hot_ticks: i64,
    positions: &[BlockPos],
) where
    K: Kinds,
    V: SimulationVolume,
    M: MemoryPartition + Default + Clone,
{
    let (conflicting, found) = redo_set(tasks, positions);
    bad.extend(found);
    if bad.is_empty() {
        return;
    }
    for (slot, task) in tasks.iter().enumerate().rev() {
        if bad.contains(&slot) {
            task.unpublish(volume);
        }
    }
    restore_cells(memory, tasks, bad);
    shard.chain.reset();
    mark_hot(shard, hot_ticks, now, &conflicting);
    out.report.tile_redos = out.report.tile_redos.saturating_add(1);
    let redone: usize = bad
        .iter()
        .filter_map(|slot| tasks.get(*slot))
        .map(|task| task.items.len())
        .sum();
    out.report.tile_redo_items = out.report.tile_redo_items.saturating_add(count(redone));
}

#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a distinct part of the world that the commit writes"
)]
fn finish<K, V, F, B, P, L, Before>(
    tasks: &mut [TileTask<K, B::Memory>],
    bad: &FxHashSet<usize>,
    items: usize,
    shard: &mut BlockShard<K>,
    volume: &mut V,
    env: &Env<'_, F, B, P>,
    memory: &mut B::Memory,
    shared: &B::Shared,
    out: &mut StepOutput<K>,
    now: GameTick,
    live: &L,
    before: &Before,
    mode: Mode,
) where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    P: OrderingPolicy,
    L: Fn(&mut Cx<'_, K, V, F, B>, usize),
    Before: Fn(&mut BlockShard<K>, usize),
{
    let mut held: FxHashMap<usize, (usize, usize)> = FxHashMap::default();
    let mut order: Vec<usize> = Vec::new();
    for (slot, task) in tasks.iter().enumerate() {
        if bad.contains(&slot) {
            continue;
        }
        for (entry, produced) in task.produced.iter().enumerate() {
            held.insert(produced.index, (slot, entry));
            order.push(produced.index);
        }
    }
    if mode == Mode::Sequence {
        order = (0..items).collect();
    }
    for item in order {
        before(shard, item);
        let Some(&(slot, entry)) = held.get(&item) else {
            let mut cx = Cx::new(shard, volume, out, env, Some(&mut *memory), shared, now);
            live(&mut cx, item);
            continue;
        };
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
        let mut cx = Cx::new(shard, volume, out, env, Some(&mut *memory), shared, now);
        if let Some(messages) = task.out.outbound.get(produced.outbound.clone()) {
            for message in messages {
                let mut message = *message;
                message.stamp = cx.shard.clock.stamp(now);
                cx.apply(message);
            }
        }
    }
    for (slot, task) in tasks.iter_mut().enumerate() {
        if bad.contains(&slot) {
            continue;
        }
        for consumed in task.consumed.drain(..) {
            shard.ticking.remove(&consumed);
        }
        out.report.absorb(task.out.report);
    }
    shard.published_ticks.clear();
    shard.published_consumed.clear();
}

struct Seen<'a> {
    task: usize,
    produced: &'a Produced,
}

fn touches(a: &Seen<'_>, b: &Seen<'_>) -> bool {
    a.produced.reads.intersects(&b.produced.writes)
        || a.produced.writes.intersects(&b.produced.reads)
}

fn out_of_order(a: &Seen<'_>, b: &Seen<'_>) -> bool {
    let (early, late) = if a.produced.index < b.produced.index {
        (a, b)
    } else {
        (b, a)
    };
    late.task < early.task && touches(a, b)
}

fn buckets<'a, K: Kinds, M>(tasks: &'a [TileTask<K, M>]) -> Vec<Vec<Seen<'a>>> {
    let mut by_cell: FxHashMap<u64, Vec<Seen<'a>>> = FxHashMap::default();
    for (task, produced) in tasks
        .iter()
        .enumerate()
        .flat_map(|(task, held)| held.produced.iter().map(move |produced| (task, produced)))
    {
        for cell in produced.reads.cells() {
            by_cell
                .entry(cell)
                .or_default()
                .push(Seen { task, produced });
        }
    }
    let mut mixed: Vec<Vec<Seen<'a>>> = by_cell
        .into_values()
        .filter(|seen| {
            seen.first()
                .is_some_and(|first| seen.iter().any(|other| other.task != first.task))
                && seen.iter().any(|other| !other.produced.writes.is_empty())
        })
        .collect();
    for seen in &mut mixed {
        seen.sort_by_key(|seen| seen.task);
    }
    mixed
}

fn cross_pairs<'a, 'b>(
    seen: &'b [Seen<'a>],
) -> impl Iterator<Item = (&'b Seen<'a>, &'b Seen<'a>)> + 'b {
    seen.iter().enumerate().flat_map(move |(n, a)| {
        seen.iter()
            .skip(n.saturating_add(1))
            .skip_while(move |b| b.task == a.task)
            .map(move |b| (a, b))
    })
}

fn redo_set<K: Kinds, M>(
    tasks: &[TileTask<K, M>],
    positions: &[BlockPos],
) -> (Vec<BlockPos>, FxHashSet<usize>) {
    let buckets = buckets(tasks);
    let mut bad: FxHashSet<usize> = FxHashSet::default();
    let mut conflicting = Vec::new();
    for seen in &buckets {
        for (a, b) in cross_pairs(seen) {
            if out_of_order(a, b) {
                bad.insert(a.task);
                bad.insert(b.task);
                conflicting.extend(positions.get(a.produced.index));
                conflicting.extend(positions.get(b.produced.index));
            }
        }
    }
    if bad.is_empty() {
        return (conflicting, bad);
    }
    let mut cells: FxHashMap<u64, Vec<usize>> = FxHashMap::default();
    for (slot, task) in tasks.iter().enumerate() {
        for cell in task.before.keys() {
            cells.entry(*cell).or_default().push(slot);
        }
    }
    loop {
        let known = bad.len();
        for seen in &buckets {
            for (a, b) in cross_pairs(seen) {
                let (good, broken) = match (bad.contains(&a.task), bad.contains(&b.task)) {
                    (false, true) => (a, b),
                    (true, false) => (b, a),
                    _ => continue,
                };
                if broken.task < good.task && touches(good, broken) {
                    bad.insert(good.task);
                }
            }
        }
        for slots in cells.values() {
            let Some(first) = slots.iter().copied().find(|slot| bad.contains(slot)) else {
                continue;
            };
            bad.extend(slots.iter().copied().filter(|slot| *slot > first));
        }
        if bad.len() == known {
            return (conflicting, bad);
        }
    }
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

fn restore_cells<K: Kinds, M: MemoryPartition + Default>(
    memory: &mut M,
    tasks: &mut [TileTask<K, M>],
    bad: &FxHashSet<usize>,
) {
    let mut restored: FxHashMap<u64, M> = FxHashMap::default();
    for (slot, task) in tasks.iter_mut().enumerate() {
        if !bad.contains(&slot) {
            continue;
        }
        for (cell, snapshot) in task.before.drain() {
            restored.entry(cell).or_insert(snapshot);
        }
    }
    for (cell, snapshot) in restored {
        memory.drop_cell(cell);
        memory.merge(snapshot);
    }
}
