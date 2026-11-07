use core::ops::Range;

use crate::SimulationVolume;
use crate::behaviour::{BlockBehaviour, BlockFacts};
use crate::cx::{Cx, TileCx};
use crate::output::StepOutput;
use crate::overlay::{OverlaidVolume, Overlay};
use crate::random_tick::{ListedSection, RandomPicks, block_tile};
use crate::region::{Region, colour};
use crate::scalar::GameTick;
use crate::shard::{BlockShard, Env};
use crate::stream::SectionStreams;
use crate::tick_queue::Kinds;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_voxel::{BlockPos, ColumnPos};

/// Runs a closure on each item of a slice, on any threads and in any order.
pub trait Spread {
    fn spread<T: Send, R: Fn(&mut T) + Sync>(&self, items: &mut [T], run: &R);
}

/// One thread, in the order of the slice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Serial;

impl Spread for Serial {
    fn spread<T: Send, R: Fn(&mut T) + Sync>(&self, items: &mut [T], run: &R) {
        for item in items {
            run(item);
        }
    }
}

/// A pick that left the region of its tile, to run again in the serial step.
#[derive(Clone, Debug)]
pub(crate) struct Rerun {
    /// The index of the section in the listed sections, then the index of the pick.
    pub(crate) order: (usize, u32),
    pub(crate) at: BlockPos,
    /// The stream of the section as the pick found it.
    pub(crate) random: LegacyRandom,
}

/// The random picks of one block tile in one step, and what they did, until the end of the
/// colour phase of the tile.
pub(crate) struct TileTask<K: Kinds> {
    pub(crate) tile: ColumnPos,
    /// The sections of the tile in the listed sections.
    listed: Range<usize>,
    shard: BlockShard<K>,
    out: StepOutput<K>,
    overlay: Overlay,
    reruns: Vec<Rerun>,
}

impl<K: Kinds> TileTask<K> {
    fn new(tile: ColumnPos, listed: Range<usize>) -> Self {
        Self {
            tile,
            listed,
            shard: BlockShard::new(),
            out: StepOutput::default(),
            overlay: Overlay::default(),
            reruns: Vec::new(),
        }
    }

    /// Makes `tasks` hold one task for each block tile of `listed`, in the order of the colour
    /// phases, then of the tiles. Keeps the buffers of the tasks it held.
    pub(crate) fn plan(tasks: &mut Vec<Self>, listed: &[ListedSection]) {
        let tile_of = |listed: &ListedSection| block_tile(ColumnPos::from(listed.section));
        let mut used = 0;
        let mut start: usize = 0;
        for group in listed.chunk_by(|a, b| tile_of(a) == tile_of(b)) {
            let Some(first) = group.first() else {
                continue;
            };
            let range = start..start.saturating_add(group.len());
            start = range.end;
            match tasks.get_mut(used) {
                Some(task) => {
                    task.tile = tile_of(first);
                    task.listed = range;
                }
                None => tasks.push(Self::new(tile_of(first), range)),
            }
            used = used.saturating_add(1);
        }
        tasks.truncate(used);
        tasks.sort_by_key(|task| (colour(task.tile), task.tile));
    }

    pub(crate) fn run<V, F, B, P>(
        &mut self,
        world: &V,
        base: &BlockShard<K>,
        env: &Env<'_, F, B, P>,
        picks: RandomPicks<'_>,
        streams: SectionStreams,
        now: GameTick,
    ) where
        V: SimulationVolume,
        F: BlockFacts<K>,
        B: BlockBehaviour<K>,
    {
        self.shard.chain.reset();
        self.shard.random_touched.clear();
        let region = Region::of(self.tile);
        let mut volume = OverlaidVolume {
            world,
            region,
            overlay: &mut self.overlay,
            table: picks.table,
        };
        let mut cx = Cx::new(&mut self.shard, &mut volume, &mut self.out, env, now);
        cx.task = Some(TileCx {
            region,
            base,
            footprints: picks.footprints,
            overflowed: false,
        });
        for index in self.listed.clone() {
            if let Some(&listed) = picks.listed.get(index) {
                cx.random_picks(listed, index, picks, streams, &mut self.reruns);
            }
        }
    }

    /// Writes what the task did into the world, the queues and `out`. The ticks and events of the
    /// task take their sequence here, so their order is the order of the tiles.
    pub(crate) fn commit<V, F, B, P>(
        &mut self,
        shard: &mut BlockShard<K>,
        volume: &mut V,
        env: &Env<'_, F, B, P>,
        out: &mut StepOutput<K>,
        now: GameTick,
        reruns: &mut Vec<Rerun>,
    ) where
        V: SimulationVolume,
        F: BlockFacts<K>,
        B: BlockBehaviour<K>,
    {
        for write in &self.out.writes {
            // The task wrote only sections that were writable, and nothing wrote them since.
            let _ = volume.set_block(write.pos, write.new);
        }
        self.overlay.clear();
        shard
            .random_touched
            .extend(self.shard.random_touched.drain());
        out.writes.append(&mut self.out.writes);
        out.destroyed.append(&mut self.out.destroyed);
        out.report.absorb(self.out.report);
        let mut cx = Cx::new(shard, volume, out, env, now);
        for mut message in self.out.outbound.drain(..) {
            message.stamp = cx.shard.clock.stamp(now);
            cx.apply(message);
        }
        self.out.clear();
        reruns.append(&mut self.reruns);
    }
}
