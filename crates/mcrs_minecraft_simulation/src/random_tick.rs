use core::mem;

use crate::SimulationVolume;
use crate::behaviour::{BlockBehaviour, BlockFacts};
use crate::cx::Cx;
use crate::output::StepOutput;
use crate::scalar::GameTick;
use crate::section;
use crate::shard::{BlockShard, Env};
use crate::stream::{SectionStreams, StreamKey, StreamSeed};
use crate::tick_queue::Kinds;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos, LocalPos, SectionPos};

use crate::footprints::Footprints;
use crate::output::OutputMark;
use crate::precipitation::{PRECIPITATION_TILE, precipitation_hits};
use crate::region::colour;
use crate::tile_task::{Spread, TileTask};

bitflags::bitflags! {
    /// The random ticks that a block state takes.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct RandomTicks: u8 {
        /// The block of the state takes random ticks.
        const BLOCK = 1;
        /// The fluid of the state takes random ticks.
        const FLUID = 2;
    }
}

/// The random ticks of each block state, asked of the rules once for each state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RandomTickTable {
    kinds: Vec<RandomTicks>,
}

impl RandomTickTable {
    pub fn new(
        states: impl IntoIterator<Item = BlockStateId>,
        mut kinds_of: impl FnMut(BlockStateId) -> RandomTicks,
    ) -> Self {
        let mut table = Self::default();
        for state in states {
            let kinds = kinds_of(state);
            if kinds.is_empty() {
                continue;
            }
            let index = state.index();
            if table.kinds.len() <= index {
                table
                    .kinds
                    .resize(index.saturating_add(1), RandomTicks::empty());
            }
            if let Some(slot) = table.kinds.get_mut(index) {
                *slot = kinds;
            }
        }
        table
    }

    #[must_use]
    pub fn kinds(&self, state: BlockStateId) -> RandomTicks {
        self.kinds.get(state.index()).copied().unwrap_or_default()
    }

    /// `state` takes random ticks of its block, of its fluid, or of both.
    #[must_use]
    pub fn ticks(&self, state: BlockStateId) -> bool {
        !self.kinds(state).is_empty()
    }
}

/// The random ticks of one step.
#[derive(Clone, Copy, Debug)]
pub struct RandomPicks<'a> {
    /// The writable columns that hold a random-ticked section: they take precipitation picks.
    /// Grouped by block tile, and in a tile sorted by x, then z.
    pub columns: &'a [ColumnPos],
    /// The picks of the step that [`draw_random_picks`] drew, grouped by block tile, in a stable
    /// order.
    pub hits: &'a [RandomHit],
    /// The precipitation picks of each column.
    pub speed: u32,
    pub table: &'a RandomTickTable,
    pub footprints: &'a Footprints,
    /// Each tile draws its precipitation from the stream of the tile and the step, so the hits do
    /// not depend on the order of the tiles or on the partition.
    pub seed: StreamSeed,
}

/// A random pick that found a position whose state ticked randomly when the step started.
#[derive(Clone, Debug)]
pub struct RandomHit {
    pub at: BlockPos,
    /// The stream of the handlers of the pick.
    pub random: LegacyRandom,
}

impl RandomHit {
    /// The hit of `pick` in `section`, whose state ticks randomly at the local indices `ticking`.
    #[must_use]
    pub fn of(section: SectionPos, ticking: &[u16], pick: RandomPick) -> Option<Self> {
        let local = *ticking.get(usize::from(pick.index))?;
        Some(Self {
            at: section::block_pos(section, LocalPos::from_index(usize::from(local))),
            random: pick.random,
        })
    }
}

/// A random pick that drew the position at `index` in the list of ticking positions of its
/// section.
#[derive(Clone, Debug)]
pub struct RandomPick {
    pub index: u16,
    /// The stream of the handlers of the pick.
    pub random: LegacyRandom,
}

/// Draws the `speed` picks of `section`, which holds `ticking` positions whose state ticks
/// randomly, from the stream of the section in `streams`, and appends those that hit to `picks`.
///
/// Each pick draws `u` in `0..4096`. A `u` below `ticking` picks the position at `u` in the list
/// of ticking positions; any other `u` picks a position that does not tick. So each position
/// takes a pick with the chance 1/4096, as a pick over the whole section. Each hit then forks the
/// stream of its handlers from the stream of the section, so the hits of a section do not depend
/// on the other sections.
pub fn draw_random_picks(
    section: SectionPos,
    ticking: usize,
    speed: u32,
    streams: SectionStreams,
    picks: &mut Vec<RandomPick>,
) {
    let mut random = streams.stream(section);
    let first = picks.len();
    for _ in 0..speed {
        let drawn = random.next_u32_bound(SectionPos::VOLUME as u32) as usize;
        if drawn < ticking {
            picks.push(RandomPick {
                // Below a count of ticking positions, at most 4096.
                index: drawn as u16,
                random: LegacyRandom::new(0),
            });
        }
    }
    for pick in picks.get_mut(first..).unwrap_or_default() {
        pick.random = random.fork();
    }
}

/// Runs the random ticks of one step: the hits of `picks.hits`, and the precipitation picks of
/// `picks.columns`. A hit whose position no longer ticks does nothing.
///
/// The block tiles run in four colour phases, the tiles of one colour on the threads of
/// `spread`. A tile task reads and writes only its region; a hit that leaves it is undone and
/// runs again, with the same position and random values, in a serial step after the phases. The
/// result is the result of the serial order: colour, tile, hit; then the hits that ran again, in
/// the order of `picks.hits`.
pub fn random_step<K, V, F, B, P, S>(
    shard: &mut BlockShard<K>,
    now: GameTick,
    volume: &mut V,
    env: &Env<'_, F, B, P>,
    picks: RandomPicks<'_>,
    out: &mut StepOutput<K>,
    spread: &S,
) where
    K: Kinds,
    V: SimulationVolume + Sync,
    F: BlockFacts<K> + Sync,
    B: BlockBehaviour<K>,
    P: Sync,
    S: Spread,
{
    shard.chain.reset();
    let mut cx = Cx::new(shard, volume, out, env, now);
    cx.precipitation_step(picks);

    let mut tasks = mem::take(&mut shard.tiles);
    let mut reruns = mem::take(&mut shard.reruns);
    TileTask::plan(&mut tasks, picks.hits);
    for phase in tasks.chunk_by_mut(|a, b| colour(a.tile) == colour(b.tile)) {
        let (world, base) = (&*volume, &*shard);
        spread.spread(phase, &|task: &mut TileTask<K>| {
            task.run(world, base, env, picks, now);
        });
        for task in phase {
            task.commit(shard, volume, env, out, now, &mut reruns);
        }
    }
    shard.tiles = tasks;

    reruns.sort_unstable();
    out.report.random_reruns = out
        .report
        .random_reruns
        .saturating_add(u32::try_from(reruns.len()).unwrap_or(u32::MAX));
    let mut cx = Cx::new(shard, volume, out, env, now);
    for hit in reruns.iter().filter_map(|&index| picks.hits.get(index)) {
        cx.random_pick(hit.at, &mut hit.random.clone(), picks.table);
    }
    reruns.clear();
    cx.shard.reruns = reruns;
}

impl<K, V, F, B> Cx<'_, K, V, F, B>
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    /// Runs the precipitation picks of the columns of `picks`, tile by tile. Each tile draws from
    /// its own stream, so the hits do not depend on the partition.
    fn precipitation_step(&mut self, picks: RandomPicks<'_>) {
        let mut hits = mem::take(&mut self.shard.rain_hits);
        for group in picks
            .columns
            .chunk_by(|a, b| block_tile(*a) == block_tile(*b))
        {
            let Some(&first) = group.first() else {
                continue;
            };
            let tile = block_tile(first);
            let mut random = picks
                .seed
                .stream(self.now, StreamKey::Precipitation { tile });
            hits.clear();
            precipitation_hits(group, picks.speed, &mut random, &mut hits);
            let behaviour = self.behaviour;
            for hit in &hits {
                behaviour.precipitation(self, hit.x, hit.z, &mut random);
            }
        }
        self.shard.rain_hits = hits;
    }

    /// Runs the hit at `index` of `picks.hits` in a tile task: a hit that leaves the region is
    /// undone and goes to `reruns`.
    pub(crate) fn random_hit(
        &mut self,
        index: usize,
        picks: RandomPicks<'_>,
        reruns: &mut Vec<usize>,
    ) {
        let Some(hit) = picks.hits.get(index) else {
            return;
        };
        let mark = self.out.mark();
        self.random_pick(hit.at, &mut hit.random.clone(), picks.table);
        if self
            .task
            .as_mut()
            .is_some_and(|task| mem::take(&mut task.overflowed))
        {
            self.undo(mark);
            reruns.push(index);
        }
    }

    /// Restores the states that the writes past `mark` replaced, last first, and drops what the
    /// output gained since.
    fn undo(&mut self, mark: OutputMark) {
        for write in self.out.writes.drain(mark.writes..).rev() {
            // The write succeeded, so writing its old state back does too.
            let _ = self.volume.set_block(write.pos, write.old);
        }
        self.out.truncate(mark);
        self.shard.chain.reset();
    }

    fn random_pick(&mut self, at: BlockPos, random: &mut LegacyRandom, table: &RandomTickTable) {
        let Ok(state) = self.volume.block(at) else {
            return;
        };
        let kinds = table.kinds(state);
        if kinds.is_empty() || !self.fits(at, state) {
            return;
        }
        let behaviour = self.behaviour;
        if kinds.contains(RandomTicks::BLOCK) {
            behaviour.random_tick(self, at, state, random);
        }
        if kinds.contains(RandomTicks::FLUID)
            && let Some(fluid) = self.facts.fluid_of(state)
        {
            behaviour.fluid_random_tick(self, at, state, fluid, random);
        }
    }
}

/// The block tile of `column`, in tiles: the unit of the block domain and of precipitation.
#[must_use]
pub fn block_tile(column: ColumnPos) -> ColumnPos {
    ColumnPos::new(
        column.x.div_euclid(PRECIPITATION_TILE),
        column.z.div_euclid(PRECIPITATION_TILE),
    )
}
