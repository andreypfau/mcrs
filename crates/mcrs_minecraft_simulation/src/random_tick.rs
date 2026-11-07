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
use mcrs_minecraft_volume::Access;
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos, LocalPos, SectionPos};

use crate::precipitation::{PRECIPITATION_TILE, precipitation_hits};

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
    /// The sections that take random ticks this step. The columns of the writable ones take
    /// precipitation picks.
    pub sections: &'a [SectionPos],
    /// The sections of `sections` that held a position whose state takes random ticks when the
    /// step started, in a stable order. Only a writable one takes picks.
    pub listed: &'a [ListedSection],
    /// The picks in each section that holds a state that takes random ticks.
    pub speed: u32,
    pub table: &'a RandomTickTable,
    /// Each section draws its picks and its handlers draw from the stream of the section and the
    /// step, so the picks do not depend on the order of the sections or on the partition.
    pub seed: StreamSeed,
}

/// A section that takes random picks, with the number of its positions whose state ticks
/// randomly when the step started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListedSection {
    pub section: SectionPos,
    pub ticking: u16,
}

/// Runs the random ticks of one step: `picks.speed` uniform picks in each section of
/// `picks.listed` that holds a state that takes random ticks.
pub fn random_step<K, V, F, B, P>(
    shard: &mut BlockShard<K>,
    now: GameTick,
    volume: &mut V,
    env: &Env<'_, F, B, P>,
    picks: RandomPicks<'_>,
    out: &mut StepOutput<K>,
) where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    shard.chain.reset();
    shard.random_touched.clear();
    let written = out.writes.len();
    let mut cx = Cx::new(shard, volume, out, env, now);
    cx.precipitation_step(picks);
    cx.touch_written_sections(written);
    let streams = picks.seed.sections(now);
    for &listed in picks.listed {
        cx.random_picks(listed, picks, streams);
    }
}

impl<K, V, F, B> Cx<'_, K, V, F, B>
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    /// Runs the precipitation picks of each column that holds a writable section of `picks`,
    /// tile by tile. Each tile draws from its own stream, so the hits do not depend on the
    /// partition or on the order of the sections.
    fn precipitation_step(&mut self, picks: RandomPicks<'_>) {
        let mut columns = mem::take(&mut self.shard.rained_columns);
        let mut hits = mem::take(&mut self.shard.rain_hits);
        columns.clear();
        columns.extend(
            picks
                .sections
                .iter()
                .filter(|&&section| self.volume.access(section) == Access::Write)
                .map(|&section| ColumnPos::from(section)),
        );
        columns.sort_unstable_by_key(|column| (block_tile(*column), column.x, column.z));
        columns.dedup();
        for group in columns.chunk_by(|a, b| block_tile(*a) == block_tile(*b)) {
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
        self.shard.rained_columns = columns;
        self.shard.rain_hits = hits;
    }

    /// Draws `u` in `0..4096` for each pick. A `u` below the number of listed positions picks the
    /// position at `u` in the list; any other `u` picks a position that does not tick. So each
    /// position takes a pick with the chance 1/4096, as a pick over the whole section.
    fn random_picks(
        &mut self,
        listed: ListedSection,
        picks: RandomPicks<'_>,
        streams: SectionStreams,
    ) {
        let section = listed.section;
        let mut listed = if self.shard.random_touched.contains(&section) {
            self.volume.random_ticking(section).len()
        } else {
            usize::from(listed.ticking)
        };
        if listed == 0 {
            return;
        }
        // Access costs a lookup, and most sections draw no value below `listed`.
        let mut writable = None;
        let mut random = streams.stream(section);
        for _ in 0..picks.speed {
            let drawn = random.next_u32_bound(SectionPos::VOLUME as u32) as usize;
            if drawn >= listed {
                continue;
            }
            if !*writable.get_or_insert_with(|| self.volume.access(section) == Access::Write) {
                return;
            }
            let Some(&local) = self.volume.random_ticking(section).get(drawn) else {
                continue;
            };
            let at = section::block_pos(section, LocalPos::from_index(usize::from(local)));
            let written = self.out.writes.len();
            self.random_pick(at, &mut random, picks.table);
            if self.out.writes.len() > written {
                self.touch_written_sections(written);
                listed = self.volume.random_ticking(section).len();
            }
        }
    }

    /// Marks the sections of the writes past `written`: the count of ticking positions that
    /// [`ListedSection`] gave for them is stale.
    fn touch_written_sections(&mut self, written: usize) {
        let writes = self.out.writes.get(written..).unwrap_or_default();
        self.shard
            .random_touched
            .extend(writes.iter().map(|write| SectionPos::from(write.pos)));
    }

    fn random_pick(&mut self, at: BlockPos, random: &mut LegacyRandom, table: &RandomTickTable) {
        let Ok(state) = self.volume.block(at) else {
            return;
        };
        let kinds = table.kinds(state);
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
