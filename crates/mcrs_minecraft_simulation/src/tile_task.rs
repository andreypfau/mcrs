use core::cell::Cell;
use core::ops::Range;

use crate::SimulationVolume;
use crate::behaviour::{BlockBehaviour, BlockFacts};
use crate::cx::{Cx, TileCx};
use crate::output::StepOutput;
use crate::overlay::{OverlaidVolume, Overlay};
use crate::policy::OrderingPolicy;
use crate::random_tick::{RandomHit, RandomPicks, block_tile};
use crate::region::{Region, colour};
use crate::scalar::GameTick;
use crate::shard::{BlockShard, Env};
use crate::tick_queue::Kinds;
use mcrs_voxel::{ColumnPos, SectionPos};

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

pub(crate) struct TileTask<K: Kinds> {
    pub(crate) tile: ColumnPos,
    hits: Range<usize>,
    shard: BlockShard<K>,
    out: StepOutput<K>,
    overlay: Overlay,
    reruns: Vec<usize>,
}

impl<K: Kinds> TileTask<K> {
    fn new(tile: ColumnPos, hits: Range<usize>) -> Self {
        Self {
            tile,
            hits,
            shard: BlockShard::new(),
            out: StepOutput::default(),
            overlay: Overlay::default(),
            reruns: Vec::new(),
        }
    }

    pub(crate) fn plan(tasks: &mut Vec<Self>, hits: &[RandomHit]) {
        let tile_of = |hit: &RandomHit| block_tile(ColumnPos::from(SectionPos::from(hit.at)));
        let mut groups = Vec::new();
        let mut start: usize = 0;
        for group in hits.chunk_by(|a, b| tile_of(a) == tile_of(b)) {
            let Some(first) = group.first() else {
                continue;
            };
            let range = start..start.saturating_add(group.len());
            start = range.end;
            groups.push((tile_of(first), range));
        }
        groups.sort_unstable_by_key(|(tile, _)| (colour(*tile), *tile));
        tasks.truncate(groups.len());
        for (used, (tile, range)) in groups.into_iter().enumerate() {
            match tasks.get_mut(used) {
                Some(task) => {
                    task.tile = tile;
                    task.hits = range;
                }
                None => tasks.push(Self::new(tile, range)),
            }
        }
    }

    pub(crate) fn run<V, F, B, P>(
        &mut self,
        world: &V,
        base: &BlockShard<K>,
        env: &Env<'_, F, B, P>,
        picks: RandomPicks<'_>,
        now: GameTick,
    ) where
        V: SimulationVolume,
        F: BlockFacts<K>,
        B: BlockBehaviour<K>,
        P: OrderingPolicy,
    {
        self.shard.chain.reset();
        let region = Region::of(self.tile);
        let mut volume = OverlaidVolume {
            world,
            region,
            overlay: &mut self.overlay,
            last: Cell::default(),
        };
        let mut cx = Cx::new(&mut self.shard, &mut volume, &mut self.out, env, None, now);
        cx.task = Some(TileCx {
            region,
            base,
            footprints: picks.footprints,
            overflowed: false,
        });
        for index in self.hits.clone() {
            cx.random_hit(index, picks, &mut self.reruns);
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
        out: &mut StepOutput<K>,
        now: GameTick,
        reruns: &mut Vec<usize>,
    ) where
        V: SimulationVolume,
        F: BlockFacts<K>,
        B: BlockBehaviour<K>,
        P: OrderingPolicy,
    {
        for write in &self.out.writes {
            let _ = volume.set_block(write.pos, write.new);
        }
        self.overlay.clear();
        out.writes.append(&mut self.out.writes);
        out.destroyed.append(&mut self.out.destroyed);
        out.report.absorb(self.out.report);
        let mut cx = Cx::new(shard, volume, out, env, Some(memory), now);
        for mut message in self.out.outbound.drain(..) {
            message.stamp = cx.shard.clock.stamp(now);
            cx.apply(message);
        }
        self.out.clear();
        reruns.append(&mut self.reruns);
    }
}
