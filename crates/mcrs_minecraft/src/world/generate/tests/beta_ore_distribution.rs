use std::cell::Cell;
use std::rc::Rc;

use mcrs_engine::world::block::BlockPos;
use mcrs_minecraft_block::palette::{BiomePalette, BlockPalette};
use mcrs_minecraft_worldgen::feature::config::{OreConfig, OreYOffset, TargetBlockState};
use mcrs_minecraft_worldgen::feature::OreFeature;
use mcrs_protocol::BlockStateId;
use mcrs_random::legacy::LegacyRandom;
use mcrs_random::Random;
use mcrs_vanilla::block::minecraft;
use rand_xoshiro::rand_core::{Infallible, TryRng};

use crate::world::generate::{place_all_ores, BetaOreBlockIds};

// ── Counting RNG: pins total LegacyRandom advances for the ore stream ───────────

#[derive(Clone)]
struct CountingRng {
    inner: LegacyRandom,
    draws: Rc<Cell<u64>>,
}

impl CountingRng {
    fn new(seed: u64, draws: Rc<Cell<u64>>) -> Self {
        CountingRng { inner: LegacyRandom::new(seed), draws }
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
        for _ in 0..(dst.len() + 7) / 8 {
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
    fn fork(&mut self) -> Self {
        self.inc();
        CountingRng { inner: self.inner.fork(), draws: self.draws.clone() }
    }
    fn fork_at<T: Into<bevy_math::IVec3>>(&mut self, pos: T) -> Self {
        self.inc();
        CountingRng { inner: self.inner.fork_at(pos), draws: self.draws.clone() }
    }
    fn fork_hash(&mut self, seed: impl AsRef<[u8]>) -> Self {
        self.inc();
        CountingRng { inner: self.inner.fork_hash(seed), draws: self.draws.clone() }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn stone_sections() -> (Vec<Option<(BlockPalette, BiomePalette)>>, Vec<i32>) {
    let y_sections: Vec<i32> = (0..8).collect();
    let stone = minecraft::STONE.default_state_id;
    let sections = (0..8)
        .map(|_| {
            let mut p = BlockPalette::default();
            for x in 0..16 {
                for y in 0..16 {
                    for z in 0..16 {
                        p.set(BlockPos::new(x, y, z), stone);
                    }
                }
            }
            Some((p, BiomePalette::default()))
        })
        .collect();
    (sections, y_sections)
}

/// Beta populate seed for a chunk (matches apply_beta_ores' derivation).
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

/// RNG-accurate replay of the ore-placement schedule on a stone chunk. Reproduces
/// place_all_ores' draw order exactly (clay coord-draws + water-gate, the seven
/// WorldGenMinable resources, then lapis), calling the real OreFeature::place so
/// the RNG advances identically. Returns each resource's vein count and the list
/// of origin-Y values drawn. Tied to the real driver by the draw-count pin below.
fn simulate<R: Random>(
    rng: &mut R,
    ids: &BetaOreBlockIds,
) -> (std::collections::BTreeMap<String, i32>, std::collections::BTreeMap<String, Vec<i32>>) {
    let stone = ids.stone;
    let feature = OreFeature;
    let (mut sections, y_sections) = stone_sections();
    let sections_ptr = sections.as_mut_slice() as *mut [Option<(BlockPalette, BiomePalette)>];

    let mut counts: std::collections::BTreeMap<String, i32> = Default::default();
    let mut ys: std::collections::BTreeMap<String, Vec<i32>> = Default::default();

    let get = |wx: i32, wy: i32, wz: i32| -> BlockStateId {
        let sl = unsafe { &*sections_ptr };
        read_block(sl, &y_sections, wx, wy, wz)
    };
    let set = |wx: i32, wy: i32, wz: i32, st: BlockStateId| {
        let sl = unsafe { &mut *sections_ptr };
        write_block(sl, &y_sections, wx, wy, wz, st);
    };

    // Clay 10x32: coord draws happen every iteration; placement only when water is
    // below the origin. On a stone chunk no water exists, so 0 veins place.
    let clay_cfg = OreConfig {
        targets: vec![TargetBlockState { target: stone, state: ids.clay }],
        size: 32,
        y_offset: OreYOffset::BetaPlus2,
    };
    let mut clay_placed = 0;
    for _ in 0..10 {
        let ox = rng.next_i32_bound(16);
        let oy = rng.next_i32_bound(128);
        let oz = rng.next_i32_bound(16);
        if get(ox, oy - 1, oz) == ids.water {
            feature.place(&clay_cfg, ox, oy, oz, &get, &set, rng);
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
            targets: vec![TargetBlockState { target: stone, state }],
            size,
            y_offset: OreYOffset::BetaPlus2,
        };
        for _ in 0..count {
            let ox = rng.next_i32_bound(16);
            let oy = rng.next_i32_bound(ybound);
            let oz = rng.next_i32_bound(16);
            feature.place(&cfg, ox, oy, oz, &get, &set, rng);
            ys.entry(name.into()).or_default().push(oy);
        }
        counts.insert(name.into(), count);
    }

    // Lapis 1x6: x, then Y = nextInt(16)+nextInt(16), then z.
    let lapis_cfg = OreConfig {
        targets: vec![TargetBlockState { target: stone, state: ids.lapis }],
        size: 6,
        y_offset: OreYOffset::BetaPlus2,
    };
    let lx = rng.next_i32_bound(16);
    let ly = rng.next_i32_bound(16) + rng.next_i32_bound(16);
    let lz = rng.next_i32_bound(16);
    feature.place(&lapis_cfg, lx, ly, lz, &get, &set, rng);
    ys.entry("lapis".into()).or_default().push(ly);
    counts.insert("lapis".into(), 1);

    (counts, ys)
}

fn read_block(
    sections: &[Option<(BlockPalette, BiomePalette)>],
    y_sections: &[i32],
    wx: i32,
    wy: i32,
    wz: i32,
) -> BlockStateId {
    if wx < 0 || wx >= 16 || wz < 0 || wz >= 16 || wy < 0 {
        return BlockStateId(0);
    }
    let si = (wy >> 4) as usize;
    let ly = wy & 0xF;
    if y_sections.get(si).copied() == Some(si as i32) {
        if let Some(Some((b, _))) = sections.get(si) {
            return b.get(BlockPos::new(wx, ly, wz));
        }
    }
    BlockStateId(0)
}

fn write_block(
    sections: &mut [Option<(BlockPalette, BiomePalette)>],
    y_sections: &[i32],
    wx: i32,
    wy: i32,
    wz: i32,
    st: BlockStateId,
) {
    if wx < 0 || wx >= 16 || wz < 0 || wz >= 16 || wy < 0 {
        return;
    }
    let si = (wy >> 4) as usize;
    let ly = wy & 0xF;
    if y_sections.get(si).copied() == Some(si as i32) {
        if let Some(Some((b, _))) = sections.get_mut(si) {
            b.set(BlockPos::new(wx, ly, wz), st);
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[test]
fn beta_ore_distribution() {
    let ids = BetaOreBlockIds::resolve();
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
    assert!(ys["redstone"].iter().all(|&y| y < 16), "redstone origin-Y < 16");
    assert!(ys["diamond"].iter().all(|&y| y < 16), "diamond origin-Y < 16");
    assert!(ys["lapis"].iter().all(|&y| (0..32).contains(&y)), "lapis origin-Y in 0..32");
    assert!(ys["coal"].iter().all(|&y| y < 128), "coal origin-Y < 128");
}

/// Pinned total LegacyRandom advances for the full ore-placement stream on chunk
/// (0,0) at seed 12345. Recorded on the first green run, asserted thereafter; any
/// change to vein counts/sizes/order shifts this value.
const ORE_DRAW_COUNT_CHUNK_0_0_SEED_12345: u64 = 3737;

#[test]
fn beta_ore_draw_count_pin() {
    let ids = BetaOreBlockIds::resolve();
    let seed = populate_seed(0, 0, 12345);

    // Real driver stream.
    let driver_draws = Rc::new(Cell::new(0u64));
    let mut driver_rng = CountingRng::new(seed as u64, driver_draws.clone());
    let (mut sections, y_sections) = stone_sections();
    place_all_ores(&mut sections, &y_sections, 0, 0, &mut driver_rng, &ids);
    let driver_count = driver_draws.get();

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
        println!("ORE DRAW COUNT PIN (chunk 0,0 seed 12345): {}", driver_count);
        assert!(driver_count > 0);
    } else {
        assert_eq!(driver_count, ORE_DRAW_COUNT_CHUNK_0_0_SEED_12345);
    }
}
