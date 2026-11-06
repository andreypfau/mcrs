use core::mem;

use crate::SimulationVolume;
use crate::behaviour::{BlockBehaviour, BlockFacts};
use crate::cx::Cx;
use crate::output::StepOutput;
use crate::scalar::GameTick;
use crate::section;
use crate::shard::{BlockShard, Env};
use crate::stream::{StreamKey, StreamSeed};
use crate::tick_queue::Kinds;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_volume::{Access, SectionBlocks};
use mcrs_voxel::{BlockStateId, ColumnPos, LocalPos, SectionPos};

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

    /// No state takes random ticks.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }

    #[must_use]
    pub fn kinds(&self, state: BlockStateId) -> RandomTicks {
        self.kinds.get(state.index()).copied().unwrap_or_default()
    }
}

/// The random ticks of one step.
#[derive(Clone, Copy, Debug)]
pub struct RandomPicks<'a> {
    /// The sections that take random ticks this step. Only a writable section takes them.
    pub sections: &'a [SectionPos],
    /// The picks in each section that holds a state that takes random ticks.
    pub speed: u32,
    pub table: &'a RandomTickTable,
    /// Each section draws its picks and its handlers draw from the stream of the section and the
    /// step, so the picks do not depend on the order of the sections or on the partition.
    pub seed: StreamSeed,
}

/// Runs the random ticks of one step: `picks.speed` uniform picks in each section of
/// `picks.sections` that holds a state that takes random ticks.
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
    let mut cx = Cx::new(shard, volume, out, env, now);
    cx.precipitation_step(picks);
    if picks.table.is_empty() {
        return;
    }
    for &section in picks.sections {
        if cx.ticks_randomly(section, picks.table) {
            let mut random = picks.seed.stream(now, StreamKey::Section { section });
            for _ in 0..picks.speed {
                cx.random_pick(section, &mut random, picks.table);
            }
        }
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
        columns.sort_unstable_by_key(|column| (tile_of(*column), column.x, column.z));
        columns.dedup();
        for group in columns.chunk_by(|a, b| tile_of(*a) == tile_of(*b)) {
            let Some(&first) = group.first() else {
                continue;
            };
            let tile = tile_of(first);
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

    fn ticks_randomly(&mut self, section: SectionPos, table: &RandomTickTable) -> bool {
        if self.volume.access(section) != Access::Write {
            return false;
        }
        let Ok(blocks) = self.volume.section(section) else {
            return false;
        };
        blocks
            .palette(&mut self.shard.palette)
            .any(|state| !table.kinds(state).is_empty())
    }

    fn random_pick(
        &mut self,
        section: SectionPos,
        random: &mut LegacyRandom,
        table: &RandomTickTable,
    ) {
        let local = LocalPos::from_index(random.next_u32_bound(4096) as usize);
        let at = section::block_pos(section, local);
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

/// The tile of `column`, in tiles.
fn tile_of(column: ColumnPos) -> ColumnPos {
    ColumnPos::new(
        column.x.div_euclid(PRECIPITATION_TILE),
        column.z.div_euclid(PRECIPITATION_TILE),
    )
}
