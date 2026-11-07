//! Random ticks of one step over the sections of 1,250 players (1/8 of 10k players: 262,144
//! listed sections), on one core. Every section holds the same 120 ticking positions, so 2.9% of
//! the picks run a handler, the rate of 185k handler runs in 6.3M picks. The handler does
//! nothing: the cost of a real handler is added to the measured time.
#![expect(clippy::expect_used, reason = "a bench reports a failure by panicking")]

use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};

use criterion::{Criterion, criterion_group, criterion_main};
use mcrs_minecraft_core::BoundingBox;
use mcrs_minecraft_engine_world::palette::{ChunkBlocks, SectionStates};
use mcrs_minecraft_engine_world::world::storage::random_ticks::RandomTickIndex;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockFacts, BlockShard, Cx, Env, GameTick, KernelConfig, Kinds, PartitionKind,
    RandomPicks, RandomTickTable, RandomTicks, SectionProperties, SemanticOrder, ShardId,
    SimulatedSections, SimulationVolume, StepOutput, StreamSeed, random_step,
};
use mcrs_minecraft_volume::{
    Access, BlockVolume, BlockVolumeMut, Denied, PalettedSection, PalettedSectionMut,
    SectionBlocksMut, Unavailable, Volume,
};
use mcrs_voxel::{BlockPos, BlockStateId, LocalPos, SectionPos};

const TICKING_PER_SECTION: usize = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Bench;

impl Kinds for Bench {
    type Block = u16;
    type Fluid = u16;
}

struct Facts;

impl BlockFacts<Bench> for Facts {
    fn block_of(&self, state: BlockStateId) -> u16 {
        state.index() as u16
    }

    fn fluid_of(&self, _: BlockStateId) -> Option<u16> {
        None
    }

    fn void_air(&self) -> BlockStateId {
        BlockStateId::AIR
    }

    fn is_air(&self, state: BlockStateId) -> bool {
        state == BlockStateId::AIR
    }

    fn is_rail(&self, _: BlockStateId) -> bool {
        false
    }

    fn is_redstone_wire(&self, _: BlockStateId) -> bool {
        false
    }

    fn legacy_block_of_fluid(&self, state: BlockStateId) -> BlockStateId {
        state
    }
}

#[derive(Default)]
struct Counted {
    runs: AtomicU64,
}

impl BlockBehaviour<Bench> for Counted {
    fn random_tick<V: SimulationVolume, F: BlockFacts<Bench>>(
        &self,
        _: &mut Cx<'_, Bench, V, F, Self>,
        _: BlockPos,
        _: BlockStateId,
        _: &mut LegacyRandom,
    ) {
        self.runs.fetch_add(1, Ordering::Relaxed);
    }
}

/// Every section holds the blocks of `template`; the index lists their ticking positions.
struct World {
    template: ChunkBlocks,
    index: RandomTickIndex,
}

impl Volume for World {
    fn bounds(&self) -> BoundingBox {
        BoundingBox {
            min: BlockPos::new(i32::MIN, -64, i32::MIN),
            max: BlockPos::new(i32::MAX, 319, i32::MAX),
        }
    }

    fn access(&self, _: SectionPos) -> Access {
        Access::Write
    }
}

impl BlockVolume for World {
    type Section<'a> = PalettedSection<'a>;

    fn section(&self, _: SectionPos) -> Result<PalettedSection<'_>, Unavailable> {
        Ok(PalettedSection(&self.template.0.0))
    }
}

impl BlockVolumeMut for World {
    type SectionMut<'a> = PalettedSectionMut<'a>;

    fn section_mut(&mut self, _: SectionPos) -> Result<PalettedSectionMut<'_>, Denied> {
        Err(Denied::ReadOnly)
    }
}

impl SimulatedSections for World {
    fn properties(&self, _: SectionPos) -> SectionProperties {
        SectionProperties::all()
    }

    fn random_ticking(&self, section: SectionPos) -> &[u16] {
        self.index.ticking(section)
    }
}

fn world(stone: BlockStateId, grass: BlockStateId, table: &RandomTickTable) -> World {
    let mut template = ChunkBlocks::new(SectionStates::homogeneous(stone));
    let mut blocks = PalettedSectionMut(&mut template.make_mut().0);
    for slot in 0..TICKING_PER_SECTION {
        blocks.set(
            LocalPos::from_index(slot * 4096 / TICKING_PER_SECTION),
            grass,
        );
    }
    let mut index = RandomTickIndex::new(table.clone());
    for x in 0..128 {
        for z in 0..128 {
            for y in -4..12 {
                let section = PalettedSection(&template.0.0);
                index.insert(SectionPos::new(x, y, z), &section, true);
            }
        }
    }
    World { template, index }
}

fn random_ticks(c: &mut Criterion) {
    let stone = BlockStateId::checked(1, 3).expect("a state of the table");
    let grass = BlockStateId::checked(2, 3).expect("a state of the table");
    let table = RandomTickTable::new([grass], |_| RandomTicks::BLOCK);
    let mut world = world(stone, grass, &table);
    let behaviour = Counted::default();
    let config = KernelConfig::default();
    let env = Env {
        partition: ShardId(0),
        facts: &Facts,
        behaviour: &behaviour,
        policy: &SemanticOrder,
        kind: PartitionKind::Shard,
        config: &config,
    };
    let seed = StreamSeed::new(4, "minecraft:overworld");
    let mut listed = Vec::new();
    world.index.listed(&mut listed);
    let mut shard = BlockShard::new();
    let mut out = StepOutput::default();
    let mut now = 0;

    c.bench_function("list the sections", |b| {
        b.iter(|| world.index.listed(black_box(&mut listed)));
    });
    c.bench_function("random step", |b| {
        b.iter(|| {
            now += 1;
            let picks = RandomPicks {
                sections: &[],
                listed: &listed,
                speed: 3,
                table: &table,
                seed,
            };
            random_step(&mut shard, GameTick(now), &mut world, &env, picks, &mut out);
        });
    });
    let runs = behaviour.runs.load(Ordering::Relaxed) as f64;
    let picks = now as f64 * listed.len() as f64 * 3.0;
    println!(
        "{} listed sections; {:.2}% of the picks ran a handler",
        listed.len(),
        100.0 * runs / picks
    );
}

criterion_group!(benches, random_ticks);
criterion_main!(benches);
