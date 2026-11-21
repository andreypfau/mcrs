use core::mem;

use crate::SimulationVolume;
use crate::behaviour::{BlockBehaviour, BlockFacts};
use crate::cx::Cx;
use crate::output::StepOutput;
use crate::policy::OrderingPolicy;
use crate::scalar::GameTick;
use crate::section;
use crate::shard::{BlockShard, Env};
use crate::stream::{SectionStreams, StreamKey, StreamSeed};
use crate::tick_queue::Kinds;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos, LocalPos, SectionPos};

use crate::footprints::Footprints;
use crate::precipitation::{PRECIPITATION_TILE, precipitation_hits};
use crate::tile_task::{Mode, Spread, Tiled, run_tiled};

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct RandomTicks: u8 {
        const BLOCK = 1;
        const FLUID = 2;
    }
}

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

    #[must_use]
    pub fn ticks(&self, state: BlockStateId) -> bool {
        !self.kinds(state).is_empty()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RandomPicks<'a> {
    pub columns: &'a [ColumnPos],
    pub hits: &'a [RandomHit],
    pub speed: u32,
    pub table: &'a RandomTickTable,
    pub footprints: &'a Footprints,
    pub seed: StreamSeed,
}

#[derive(Clone, Debug)]
pub struct RandomHit {
    pub at: BlockPos,
    pub random: LegacyRandom,
}

impl RandomHit {
    #[must_use]
    pub fn of(section: SectionPos, ticking: &[u16], pick: RandomPick) -> Option<Self> {
        let local = *ticking.get(usize::from(pick.index))?;
        Some(Self {
            at: section::block_pos(section, LocalPos::from_index(usize::from(local))),
            random: pick.random,
        })
    }
}

#[derive(Clone, Debug)]
pub struct RandomPick {
    pub index: u16,
    pub random: LegacyRandom,
}

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
                index: drawn as u16,
                random: LegacyRandom::new(0),
            });
        }
    }
    for pick in picks.get_mut(first..).unwrap_or_default() {
        pick.random = random.fork();
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a distinct part of the world that the step reads or writes"
)]
pub fn random_step<K, V, F, B, P, S>(
    shard: &mut BlockShard<K>,
    now: GameTick,
    volume: &mut V,
    env: &Env<'_, F, B, P>,
    memory: &mut B::Memory,
    shared: &B::Shared,
    picks: RandomPicks<'_>,
    out: &mut StepOutput<K>,
    spread: &S,
) where
    K: Kinds,
    V: SimulationVolume + Sync,
    F: BlockFacts<K> + Sync,
    B: BlockBehaviour<K>,
    P: OrderingPolicy,
    S: Spread,
{
    shard.chain.reset();
    let mut cx = Cx::new(shard, volume, out, env, Some(&mut *memory), shared, now);
    cx.precipitation_step(picks);

    let positions: Vec<BlockPos> = picks.hits.iter().map(|hit| hit.at).collect();
    let tiled = run_tiled(
        shard,
        volume,
        env,
        memory,
        shared,
        picks.footprints,
        out,
        now,
        spread,
        &positions,
        Mode::Colour,
        &|cx, index| {
            if let Some(hit) = picks.hits.get(index) {
                cx.random_pick(hit.at, &mut hit.random.clone(), picks.table);
            }
        },
        &|_, _| {},
    );
    let again: Vec<usize> = match tiled {
        Tiled::Done { reruns, left } => {
            out.report.random_reruns = out.report.random_reruns.saturating_add(left);
            reruns
        }
        Tiled::Redo { left } => {
            out.report.random_reruns = out.report.random_reruns.saturating_add(left);
            (0..picks.hits.len()).collect()
        }
    };
    let mut cx = Cx::new(shard, volume, out, env, Some(memory), shared, now);
    for hit in again.iter().filter_map(|&index| picks.hits.get(index)) {
        cx.random_pick(hit.at, &mut hit.random.clone(), picks.table);
    }
}

impl<K, V, F, B> Cx<'_, K, V, F, B>
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
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

    pub(crate) fn random_pick(
        &mut self,
        at: BlockPos,
        random: &mut LegacyRandom,
        table: &RandomTickTable,
    ) {
        self.touch_read(at);
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

#[must_use]
pub fn block_tile(column: ColumnPos) -> ColumnPos {
    ColumnPos::new(
        column.x.div_euclid(PRECIPITATION_TILE),
        column.z.div_euclid(PRECIPITATION_TILE),
    )
}
