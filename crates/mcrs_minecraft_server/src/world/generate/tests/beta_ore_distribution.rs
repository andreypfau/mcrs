use std::cell::Cell;
use std::rc::Rc;

use bevy_math::IVec3;
use mcrs_minecraft_decoration::feature::ore_beta::{OreConfig, TargetBlockState, place_beta_ore};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_voxel_storage::{Blocks, BoxVolume, VoxelId};
use rand_xoshiro::rand_core::{Infallible, TryRng};

use crate::world::generate::{BetaOreBlockIds, place_all_ores};

// ── Counting RNG: pins total LegacyRandom advances for the ore stream ───────────

#[derive(Clone)]
struct CountingRng {
    inner: LegacyRandom,
    draws: Rc<Cell<u64>>,
}

impl CountingRng {
    fn new(seed: u64, draws: Rc<Cell<u64>>) -> Self {
        CountingRng {
            inner: LegacyRandom::new(seed),
            draws,
        }
    }
    fn inc(&self) {
        self.draws.set(self.draws.get() + 1);
    }
}

impl TryRng for CountingRng {
    type Error = Infallible;
    fn try_next_u32(&mut self) -> Result<u32, Infallible> {
        self.inc();
        self.inner.try_next_u32()
    }
    fn try_next_u64(&mut self) -> Result<u64, Infallible> {
        self.inc();
        self.inner.try_next_u64()
    }
    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Infallible> {
        for _ in 0..dst.len().div_ceil(8) {
            self.inc();
        }
        self.inner.try_fill_bytes(dst)
    }
}

impl Random for CountingRng {
    fn is_legacy(&self) -> bool {
        true
    }
    fn next_bool(&mut self) -> bool {
        self.inc();
        self.inner.next_bool()
    }
    fn next_u32_bound(&mut self, bound: u32) -> u32 {
        self.inc();
        self.inner.next_u32_bound(bound)
    }
    fn next_f32(&mut self) -> f32 {
        self.inc();
        self.inner.next_f32()
    }
    fn next_f64(&mut self) -> f64 {
        self.inc();
        self.inc();
        self.inner.next_f64()
    }

    fn next_gaussian(&mut self) -> f64 {
        self.inner.next_gaussian()
    }
    fn fork(&mut self) -> Self {
        self.inc();
        CountingRng {
            inner: self.inner.fork(),
            draws: self.draws.clone(),
        }
    }
    fn fork_at<T: Into<bevy_math::IVec3>>(&mut self, pos: T) -> Self {
        self.inc();
        CountingRng {
            inner: self.inner.fork_at(pos),
            draws: self.draws.clone(),
        }
    }
    fn fork_hash(&mut self, seed: impl AsRef<[u8]>) -> Self {
        self.inc();
        CountingRng {
            inner: self.inner.fork_hash(seed),
            draws: self.draws.clone(),
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Stone over the 3×3 of columns around chunk (0, 0), in world coordinates: the
/// region a `Run` sees, so a vein that leaves its own column still lands.
fn stone_volume(stone: VoxelId) -> BoxVolume {
    BoxVolume::filled(IVec3::new(-16, 0, -16), IVec3::new(31, 127, 31), stone)
}

/// Every placed block as (column offset from the centre, state), so a census
/// can tell the half of a vein that stayed home from the half that crossed.
fn placed(volume: &BoxVolume, stone: VoxelId) -> Vec<((i32, i32), VoxelId)> {
    volume
        .iter()
        .filter(|(_, state)| *state != stone)
        .map(|(at, state)| ((at.x.div_euclid(16), at.z.div_euclid(16)), state))
        .collect()
}

/// Beta populate seed for a chunk (matches apply_beta_ores_in's derivation).
fn populate_seed(chunk_x: i32, chunk_z: i32, world_seed: i64) -> i64 {
    let mut s = LegacyRandom::new(world_seed as u64);
    let i1 = s.next_java_long() / 2 * 2 + 1;
    let j1 = s.next_java_long() / 2 * 2 + 1;
    (chunk_x as i64)
        .wrapping_mul(i1)
        .wrapping_add((chunk_z as i64).wrapping_mul(j1))
        ^ world_seed
}

// Beta placement table (resource, vein count, vein size, Y-range bound).
// Mirrors place_all_ores / ChunkProviderGenerate.getChunkAt lines 344-406.
const NON_CLAY_TABLE: &[(&str, i32, i32, i32)] = &[
    ("dirt", 20, 32, 128),
    ("gravel", 10, 32, 128),
    ("coal", 20, 16, 128),
    ("iron", 20, 8, 64),
    ("gold", 2, 8, 32),
    ("redstone", 8, 7, 16),
    ("diamond", 1, 7, 16),
];

/// RNG-accurate replay of the ore-placement schedule on a stone region. Reproduces
/// place_all_ores' draw order exactly (clay coord-draws + water-gate, the seven
/// WorldGenMinable resources, then lapis), calling the real place_beta_ore so
/// the RNG advances identically. Returns each resource's vein count and the list
/// of origin-Y values drawn. Tied to the real driver by the draw-count pin below.
fn simulate<R: Random>(
    rng: &mut R,
    ids: &BetaOreBlockIds,
) -> (
    std::collections::BTreeMap<String, i32>,
    std::collections::BTreeMap<String, Vec<i32>>,
) {
    let stone = ids.stone;
    let mut volume = stone_volume(stone.into());

    let mut counts: std::collections::BTreeMap<String, i32> = Default::default();
    let mut ys: std::collections::BTreeMap<String, Vec<i32>> = Default::default();

    // Clay 10x32: coord draws happen every iteration; a vein starts only from a
    // water block and turns sand. On a stone region no water exists, so 0 veins place.
    let clay_cfg = OreConfig {
        targets: vec![TargetBlockState {
            target: ids.sand.into(),
            state: ids.clay.into(),
        }],
        size: 32,
    };
    let mut clay_placed = 0;
    for _ in 0..10 {
        let ox = rng.next_i32_bound(16);
        let oy = rng.next_i32_bound(128);
        let oz = rng.next_i32_bound(16);
        if volume.get(IVec3::new(ox, oy, oz)) == ids.water.into() {
            place_beta_ore(&clay_cfg, IVec3::new(ox, oy, oz), &mut volume, rng);
            clay_placed += 1;
            ys.entry("clay".into()).or_default().push(oy);
        }
    }
    counts.insert("clay".into(), clay_placed);

    for &(name, count, size, ybound) in NON_CLAY_TABLE {
        let state = match name {
            "dirt" => ids.dirt,
            "gravel" => ids.gravel,
            "coal" => ids.coal,
            "iron" => ids.iron,
            "gold" => ids.gold,
            "redstone" => ids.redstone,
            "diamond" => ids.diamond,
            _ => unreachable!(),
        };
        let cfg = OreConfig {
            targets: vec![TargetBlockState {
                target: stone.into(),
                state: state.into(),
            }],
            size,
        };
        for _ in 0..count {
            let ox = rng.next_i32_bound(16);
            let oy = rng.next_i32_bound(ybound);
            let oz = rng.next_i32_bound(16);
            place_beta_ore(&cfg, IVec3::new(ox, oy, oz), &mut volume, rng);
            ys.entry(name.into()).or_default().push(oy);
        }
        counts.insert(name.into(), count);
    }

    // Lapis 1x6: x, then Y = nextInt(16)+nextInt(16), then z.
    let lapis_cfg = OreConfig {
        targets: vec![TargetBlockState {
            target: stone.into(),
            state: ids.lapis.into(),
        }],
        size: 6,
    };
    let lx = rng.next_i32_bound(16);
    let ly = rng.next_i32_bound(16) + rng.next_i32_bound(16);
    let lz = rng.next_i32_bound(16);
    place_beta_ore(&lapis_cfg, IVec3::new(lx, ly, lz), &mut volume, rng);
    ys.entry("lapis".into()).or_default().push(ly);
    counts.insert("lapis".into(), 1);

    (counts, ys)
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[test]
fn beta_ore_distribution() {
    let ids = BetaOreBlockIds::resolve(super::corpus());
    let seed = populate_seed(0, 0, 12345);
    let mut rng = LegacyRandom::new(seed as u64);
    let (counts, ys) = simulate(&mut rng, &ids);

    // Exact vein counts (clay is water-dependent: <= 10; on stone it is 0).
    assert_eq!(counts["dirt"], 20);
    assert_eq!(counts["gravel"], 10);
    assert_eq!(counts["coal"], 20);
    assert_eq!(counts["iron"], 20);
    assert_eq!(counts["gold"], 2);
    assert_eq!(counts["redstone"], 8);
    assert_eq!(counts["diamond"], 1);
    assert_eq!(counts["lapis"], 1);
    assert!(counts["clay"] <= 10, "clay veins must be <= 10");

    // Y-range bounds on the per-vein origin draw.
    assert!(ys["iron"].iter().all(|&y| y < 64), "iron origin-Y < 64");
    assert!(ys["gold"].iter().all(|&y| y < 32), "gold origin-Y < 32");
    assert!(
        ys["redstone"].iter().all(|&y| y < 16),
        "redstone origin-Y < 16"
    );
    assert!(
        ys["diamond"].iter().all(|&y| y < 16),
        "diamond origin-Y < 16"
    );
    assert!(
        ys["lapis"].iter().all(|&y| (0..32).contains(&y)),
        "lapis origin-Y in 0..32"
    );
    assert!(ys["coal"].iter().all(|&y| y < 128), "coal origin-Y < 128");
}

/// Pinned total LegacyRandom advances for the full ore-placement stream on chunk
/// (0,0) at seed 12345. Recorded on the first green run, asserted thereafter; any
/// change to vein counts/sizes/order shifts this value.
const ORE_DRAW_COUNT_CHUNK_0_0_SEED_12345: u64 = 3737;

/// Blocks the driver places on the 3×3 stone region from chunk (0,0), seed 12345,
/// as (own column, columns it crossed into). Re-recorded when the pass moved onto
/// the ladder's `Run`: it used to write only the first number, the rest of every
/// bordering vein having been clipped away.
const ORE_BLOCKS_CHUNK_0_0_SEED_12345: (usize, usize) = (964, 2580);

fn drive(seed: i64, ids: &BetaOreBlockIds) -> (BoxVolume, u64) {
    let draws = Rc::new(Cell::new(0u64));
    let mut rng = CountingRng::new(seed as u64, draws.clone());
    let mut volume = stone_volume(ids.stone.into());
    place_all_ores(&mut volume, 0, 0, &mut rng, ids);
    (volume, draws.get())
}

#[test]
fn beta_ore_draw_count_pin() {
    let ids = BetaOreBlockIds::resolve(super::corpus());
    let seed = populate_seed(0, 0, 12345);
    let (_, driver_count) = drive(seed, &ids);

    // Mirror stream — must consume identical RNG, proving the simulate() replay
    // matches the production driver's schedule.
    let mirror_draws = Rc::new(Cell::new(0u64));
    let mut mirror_rng = CountingRng::new(seed as u64, mirror_draws.clone());
    let _ = simulate(&mut mirror_rng, &ids);
    assert_eq!(
        driver_count,
        mirror_draws.get(),
        "distribution mirror diverged from the production ore driver"
    );

    if ORE_DRAW_COUNT_CHUNK_0_0_SEED_12345 == 0 {
        println!(
            "ORE DRAW COUNT PIN (chunk 0,0 seed 12345): {}",
            driver_count
        );
        assert!(driver_count > 0);
    } else {
        assert_eq!(driver_count, ORE_DRAW_COUNT_CHUNK_0_0_SEED_12345);
    }
}

#[test]
fn veins_cross_the_column_border() {
    let ids = BetaOreBlockIds::resolve(super::corpus());
    let (volume, _) = drive(populate_seed(0, 0, 12345), &ids);
    let placed = placed(&volume, ids.stone.into());

    let own = placed.iter().filter(|(col, _)| *col == (0, 0)).count();
    let crossed = placed.len() - own;

    assert_eq!((own, crossed), ORE_BLOCKS_CHUNK_0_0_SEED_12345);

    // A vein reaches at most fourteen blocks past an origin inside its own
    // column, so the writes land in the 3×3 and never beyond it.
    for (col, _) in &placed {
        assert!(
            (-1..=1).contains(&col.0) && (-1..=1).contains(&col.1),
            "a write reached column {col:?}"
        );
    }

    // Every resource of the table places at least one block somewhere.
    for state in [
        ids.dirt,
        ids.gravel,
        ids.coal,
        ids.iron,
        ids.gold,
        ids.redstone,
        ids.diamond,
        ids.lapis,
    ] {
        assert!(
            placed.iter().any(|(_, got)| *got == state.into()),
            "no block of {state:?} was placed"
        );
    }
}

/// Beta's populate step reached as a feature writes what running it on the
/// region directly writes: the source the feature's step hands it plays no part.
#[test]
fn the_populate_feature_is_the_populate_step() {
    use std::sync::Arc;

    use mcrs_minecraft_protocol::ColumnPos;

    use crate::world::generate::ColumnBlocks;
    use crate::world::generate::beta_ores::apply_beta_ores_in;
    use crate::world::generate::stages::{ColumnRegion, run_column};

    let router = super::build_beta_router();
    let seed = router.world_seed as i64;
    let (_, registry) = super::beta_surface::build_beta_biome_source();
    let program = Arc::new(super::beta_populate_program(&registry, seed));
    let ctx = super::fill_context_with(router, Some(program));
    let stone: VoxelId = super::corpus().default_state("minecraft:stone").into();
    let center = ColumnPos::new(3, -2);
    let snapshots = super::region_of(center, |col| {
        super::flat_snapshot(
            col,
            &ctx.y_sections,
            |section_y| (0..8).contains(&section_y).then_some(stone),
            None,
        )
    });

    let writes = |populate: &dyn Fn(&mut ColumnRegion<'_>)| {
        let column = ColumnBlocks::new(&ctx.y_sections);
        column.unpack(&snapshots[4].sections);
        let mut region = ColumnRegion::new(&snapshots, &column, &ctx);
        populate(&mut region);
        region
            .finish()
            .into_iter()
            .map(|(col, delta)| (col, delta.writes))
            .collect::<Vec<_>>()
    };
    let ids = BetaOreBlockIds::resolve(super::corpus());
    let through_program = writes(&|region| run_column(&ctx, region, 0));
    let direct = writes(&|region| apply_beta_ores_in(region, center.x, center.z, seed, &ids));

    assert!(
        direct.iter().map(|(_, writes)| writes.len()).sum::<usize>() > 0,
        "the populate step wrote nothing"
    );
    assert_eq!(through_program, direct);
}
