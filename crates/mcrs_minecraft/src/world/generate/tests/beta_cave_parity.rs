use std::collections::BTreeMap;
use std::sync::Arc;

use bevy_asset::Assets;
use mcrs_core::RegistrySnapshot;
use mcrs_core::resource_location::ResourceLocation;
use mcrs_engine::world::block::BlockPos;
use mcrs_minecraft_block::palette::{BiomePalette, BlockPalette};
use mcrs_minecraft_worldgen::carver::cave::CaveWorldCarver;
use mcrs_minecraft_worldgen::carver::config::BetaCaveCarverConfig;
use mcrs_minecraft_worldgen::carver::WorldCarver;
use mcrs_minecraft_worldgen::density_function::build_functions;
use mcrs_minecraft_worldgen::proto::NoiseGeneratorSettings;
use mcrs_protocol::BlockStateId;
use mcrs_random::Random;
use mcrs_random::legacy::LegacyRandom;
use rand_xoshiro::rand_core::{Infallible, TryRng};
use mcrs_vanilla::biome::Biome;
use mcrs_vanilla::biome::source::{BiomeSource, build_beta_lookup_table};
use mcrs_vanilla::block::minecraft;

use crate::world::chunk::CancellationToken;
use crate::world::generate::{apply_beta_caves, apply_beta_surface, generate_column, BetaCaveBlockIds};

// ── Corpus deserialization ────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct BetaSurfaceCorpus {
    columns: Vec<ColumnFixture>,
}

#[derive(serde::Deserialize)]
struct ColumnFixture {
    wx: i32,
    wz: i32,
    #[allow(dead_code)]
    biome_id: u8,
    #[serde(with = "serde_base64")]
    pre_cave: Vec<u8>,
    #[serde(with = "serde_base64")]
    post_cave: Vec<u8>,
}

mod serde_base64 {
    use base64::Engine as _;
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(de)?;
        base64::engine::general_purpose::STANDARD
            .decode(s)
            .map_err(serde::de::Error::custom)
    }
}

fn load_corpus() -> BetaSurfaceCorpus {
    serde_json::from_str(include_str!("fixtures/beta_surface_corpus.json"))
        .expect("valid corpus JSON")
}

// ── Block ID mapping ──────────────────────────────────────────────────────────

fn modern_id_for_beta(beta_id: u8) -> BlockStateId {
    match beta_id {
        0  => minecraft::AIR.default_state_id,
        1  => minecraft::STONE.default_state_id,
        2  => minecraft::GRASS_BLOCK.default_state_id,
        3  => minecraft::DIRT.default_state_id,
        7  => minecraft::BEDROCK.default_state_id,
        9  => minecraft::WATER.default_state_id,
        12 => minecraft::SAND.default_state_id,
        13 => minecraft::GRAVEL.default_state_id,
        24 => minecraft::SANDSTONE.default_state_id,
        79 => minecraft::ICE.default_state_id,
        _  => minecraft::AIR.default_state_id,
    }
}

fn beta_id_for_modern(modern: BlockStateId) -> u8 {
    let air       = minecraft::AIR.default_state_id;
    let stone     = minecraft::STONE.default_state_id;
    let grass     = minecraft::GRASS_BLOCK.default_state_id;
    let dirt      = minecraft::DIRT.default_state_id;
    let bedrock   = minecraft::BEDROCK.default_state_id;
    let sand      = minecraft::SAND.default_state_id;
    let gravel    = minecraft::GRAVEL.default_state_id;
    let sandstone = minecraft::SANDSTONE.default_state_id;
    let water     = minecraft::WATER.default_state_id;
    let lava      = minecraft::LAVA.default_state_id;
    let ice       = minecraft::ICE.default_state_id;

    if modern == air        { return 0;  }
    if modern == stone      { return 1;  }
    if modern == grass      { return 2;  }
    if modern == dirt       { return 3;  }
    if modern == bedrock    { return 7;  }
    if modern == water      { return 9;  }
    if modern == lava       { return 10; }
    if modern == sand       { return 12; }
    if modern == gravel     { return 13; }
    if modern == sandstone  { return 24; }
    if modern == ice        { return 79; }
    0
}

// ── Cave config helper ────────────────────────────────────────────────────────

fn make_cave_config() -> (BetaCaveCarverConfig, BetaCaveBlockIds) {
    let ids = BetaCaveBlockIds::resolve();
    let config = BetaCaveCarverConfig {
        air_state:              ids.air,
        lava_state:             ids.lava,
        stone_state:            ids.stone,
        dirt_state:             ids.dirt,
        grass_state:            ids.grass,
        water_state:            ids.water,
        stationary_water_state: ids.stationary_water,
        lava_level:             10,
        range:                  8,
        horizontal_radius_multiplier: 1.0,
        vertical_radius_multiplier:   1.0,
    };
    (config, ids)
}

// ── Draw-count instrumentation ────────────────────────────────────────────────

/// A LegacyRandom wrapper that counts every LCG advance (every call that
/// advances the generator by at least one step).
///
/// Used to produce a deterministic draw-count pin for the 17x17 seed loop.
#[derive(Clone)]
struct CountingRng {
    inner: LegacyRandom,
    draws: std::rc::Rc<std::cell::Cell<u64>>,
}

impl CountingRng {
    fn new(seed: u64, draws: std::rc::Rc<std::cell::Cell<u64>>) -> Self {
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
        let n = (dst.len() + 7) / 8;
        for _ in 0..n { self.inc(); }
        self.inner.try_fill_bytes(dst)
    }
}

impl Random for CountingRng {
    fn is_legacy(&self) -> bool { true }

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
        // next_f64 in LegacyRandom advances twice (30 bits + one advance).
        self.inc();
        self.inc();
        self.inner.next_f64()
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

/// Count the total RNG draw operations consumed by the 17x17 loop for one chunk.
fn count_rng_draws_for_chunk(chunk_x: i32, chunk_z: i32, world_seed: i64) -> u64 {
    let draws = std::rc::Rc::new(std::cell::Cell::new(0u64));
    let (config, ids) = make_cave_config();
    let carver = CaveWorldCarver;

    // Stone-filled dummy sections so the carver has blocks to process.
    let mut sections: Vec<Option<(BlockPalette, BiomePalette)>> =
        (0..8).map(|_| {
            let mut p = BlockPalette::default();
            let b = BiomePalette::default();
            for x in 0..16i32 {
                for y in 0..16i32 {
                    for z in 0..16i32 {
                        p.set(BlockPos::new(x, y, z), ids.stone);
                    }
                }
            }
            Some((p, b))
        })
        .collect();

    let y_sections: Vec<i32> = (0..8).collect();

    let mut seed_rng = LegacyRandom::new(world_seed as u64);
    let l: i64 = seed_rng.next_i64() / 2 * 2 + 1;
    let i1: i64 = seed_rng.next_i64() / 2 * 2 + 1;
    draws.set(draws.get() + 2); // two draws for l and i1

    let radius = config.range;
    let sections_ptr = sections.as_mut_slice() as *mut [Option<(BlockPalette, BiomePalette)>];
    let ys = y_sections.as_slice();
    let air = ids.air;

    for origin_x in (chunk_x - radius)..=(chunk_x + radius) {
        for origin_z in (chunk_z - radius)..=(chunk_z + radius) {
            let seed: i64 = (origin_x as i64)
                .wrapping_mul(l)
                .wrapping_add((origin_z as i64).wrapping_mul(i1))
                ^ world_seed;

            let mut counting_rng = CountingRng::new(seed as u64, draws.clone());

            let get_block = |local_x: i32, world_y: i32, local_z: i32| -> BlockStateId {
                let sl = unsafe { &*sections_ptr };
                let section_y = world_y >> 4;
                let local_y = world_y & 0xF;
                if let Some(si) = ys.iter().position(|&sy| sy == section_y) {
                    if let Some(Some((blocks, _))) = sl.get(si) {
                        return blocks.get(BlockPos::new(local_x, local_y, local_z));
                    }
                }
                air
            };
            let set_block = |local_x: i32, world_y: i32, local_z: i32, state: BlockStateId| {
                let sl = unsafe { &mut *sections_ptr };
                let section_y = world_y >> 4;
                let local_y = world_y & 0xF;
                if let Some(si) = ys.iter().position(|&sy| sy == section_y) {
                    if let Some(Some((blocks, _))) = sl.get_mut(si) {
                        blocks.set(BlockPos::new(local_x, local_y, local_z), state);
                    }
                }
            };

            carver.carve(
                &config,
                chunk_x,
                chunk_z,
                origin_x,
                origin_z,
                get_block,
                set_block,
                &mut counting_rng,
            );
        }
    }

    draws.get()
}

// ── Router + biome source builders (verbatim from beta_surface_parity.rs) ─────

fn make_beta_biome() -> Biome {
    Biome {
        temperature: 0.5,
        downfall: 0.5,
        has_precipitation: true,
        effects: mcrs_vanilla::biome::BiomeEffects {
            water_color: None,
            foliage_color: None,
            grass_color: None,
            grass_color_modifier: None,
            dry_foliage_color: None,
        },
        carvers: Vec::new(),
        features: Vec::new(),
        spawners: mcrs_vanilla::biome::BiomeSpawners {
            ambient: Vec::new(),
            axolotls: Vec::new(),
            creature: Vec::new(),
            misc: Vec::new(),
            monster: Vec::new(),
            underground_water_creature: Vec::new(),
            water_ambient: Vec::new(),
            water_creature: Vec::new(),
        },
        spawn_costs: std::collections::HashMap::new(),
        attributes: None,
    }
}

fn load_density_functions_from_disk() -> BTreeMap<
    mcrs_protocol::Ident<String>,
    mcrs_minecraft_worldgen::density_function::proto::ProtoDensityFunction,
> {
    use mcrs_minecraft_worldgen::density_function::proto::DensityFunctionHolder;
    fn recurse(
        dir: &std::path::Path,
        prefix: &str,
        map: &mut BTreeMap<
            mcrs_protocol::Ident<String>,
            mcrs_minecraft_worldgen::density_function::proto::ProtoDensityFunction,
        >,
    ) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let subdir = entry.file_name().to_string_lossy().to_string();
                let new_prefix = if prefix.is_empty() {
                    subdir
                } else {
                    format!("{}/{}", prefix, subdir)
                };
                recurse(&path, &new_prefix, map);
            } else if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let Ok(json) = std::fs::read_to_string(&path) else { continue };
                let Ok(DensityFunctionHolder::Owned(pdf)) =
                    serde_json::from_str::<DensityFunctionHolder>(&json)
                else {
                    continue;
                };
                let stem = path.file_stem().unwrap().to_string_lossy();
                let key = if prefix.is_empty() {
                    format!("minecraft:{}", stem)
                } else {
                    format!("minecraft:{}/{}", prefix, stem)
                };
                if let Ok(ident) = key.parse::<mcrs_protocol::Ident<String>>() {
                    map.insert(ident, *pdf);
                }
            }
        }
    }
    let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/minecraft/worldgen/density_function");
    let mut map = BTreeMap::new();
    recurse(&base, "", &mut map);
    map
}

fn build_beta_router() -> mcrs_minecraft_worldgen::density_function::NoiseRouter {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/minecraft/worldgen/noise_settings/beta.json"
    );
    let json = std::fs::read_to_string(path).expect("beta.json must exist");
    let settings: NoiseGeneratorSettings =
        serde_json::from_str(&json).expect("beta.json must deserialize");
    let functions = load_density_functions_from_disk();
    let noises = BTreeMap::new();
    build_functions(
        &functions,
        &noises,
        &settings,
        12345,
        mcrs_protocol::BlockStateId(1),
        mcrs_protocol::BlockStateId(86),
    )
}

fn build_beta_biome_source() -> (BiomeSource, RegistrySnapshot<Biome>) {
    let mut assets = Assets::<Biome>::default();
    let land_handles: Vec<_> = (0..11).map(|_| assets.add(make_beta_biome())).collect();
    let ocean_handles: Vec<_> = (0..5).map(|_| assets.add(make_beta_biome())).collect();
    let land_ids: Vec<_> = land_handles.iter().map(|h| h.id()).collect();
    let ocean_ids: Vec<_> = ocean_handles.iter().map(|h| h.id()).collect();
    let all_pairs: Vec<(ResourceLocation<Arc<str>>, _)> = (0..11)
        .map(|i| {
            let rl = ResourceLocation::parse(&format!("minecraft:land_biome_{i}")).unwrap();
            (rl, land_ids[i])
        })
        .chain((0..5).map(|i| {
            let rl = ResourceLocation::parse(&format!("minecraft:ocean_biome_{i}")).unwrap();
            (rl, ocean_ids[i])
        }))
        .collect();
    let snapshot = RegistrySnapshot::<Biome>::build(
        all_pairs,
        &assets,
        |_| Ok(mcrs_nbt::compound::NbtCompound::new()),
    );
    let land_biome_ids: [ResourceLocation<Arc<str>>; 11] = std::array::from_fn(|i| {
        ResourceLocation::parse(&format!("minecraft:land_biome_{i}")).unwrap()
    });
    let ocean_biome_ids: [ResourceLocation<Arc<str>>; 5] = std::array::from_fn(|i| {
        ResourceLocation::parse(&format!("minecraft:ocean_biome_{i}")).unwrap()
    });
    let biome_source = BiomeSource::Beta {
        land_biomes: land_handles.try_into().expect("11 land handles"),
        ocean_biomes: ocean_handles.try_into().expect("5 ocean handles"),
        land_biome_ids,
        ocean_biome_ids,
        lookup: Box::new(build_beta_lookup_table()),
    };
    (biome_source, snapshot)
}

fn make_chunk_rng(chunk_x: i32, chunk_z: i32) -> LegacyRandom {
    let seed: i64 = (chunk_x as i64)
        .wrapping_mul(341873128712)
        .wrapping_add((chunk_z as i64).wrapping_mul(132897987541));
    LegacyRandom::new(seed as u64)
}

// ── Parity test ───────────────────────────────────────────────────────────────

/// Carve-mask parity gate: for each chunk in the fixture corpus, build a full
/// 16x16 section palette from `pre_cave` bytes, run `apply_beta_caves` at seed
/// 12345, convert the result back to Beta block IDs, and assert equality with
/// `post_cave`.
///
/// The fixed-terrain input (pre_cave from the corpus) removes f32 terrain
/// divergence by construction, so any carve-mask mismatch is a real parity bug.
#[test]
fn beta_cave_parity_gate() {
    let corpus = load_corpus();
    let (config, ids) = make_cave_config();

    let world_seed: i64 = 12345;
    let y_sections: Vec<i32> = (0..8).collect();

    let mut chunks: BTreeMap<(i32, i32), Vec<&ColumnFixture>> = BTreeMap::new();
    for col in &corpus.columns {
        let cx = col.wx >> 4;
        let cz = col.wz >> 4;
        chunks.entry((cx, cz)).or_default().push(col);
    }

    let mut total_columns: u64 = 0;
    let mut mismatches: Vec<(i32, i32, Vec<(i32, u8, u8)>)> = Vec::new();

    for ((cx, cz), fixture_cols) in &chunks {
        let block_x = cx * 16;
        let block_z = cz * 16;

        let mut sections: Vec<Option<(BlockPalette, BiomePalette)>> =
            (0..y_sections.len())
                .map(|_| Some((BlockPalette::default(), BiomePalette::default())))
                .collect();

        for fix_col in fixture_cols.iter() {
            let local_x = fix_col.wx - block_x;
            let local_z = fix_col.wz - block_z;
            for (si, &sy) in y_sections.iter().enumerate() {
                let base_y = sy * 16;
                if let Some(Some((palette, _))) = sections.get_mut(si) {
                    for local_y in 0..16i32 {
                        let world_y = base_y + local_y;
                        if world_y < 128 {
                            let beta_id = fix_col.pre_cave[world_y as usize];
                            palette.set(
                                BlockPos::new(local_x, local_y, local_z),
                                modern_id_for_beta(beta_id),
                            );
                        }
                    }
                }
            }
        }

        apply_beta_caves(&mut sections, &y_sections, *cx, *cz, world_seed, &config, &ids);

        for fix_col in fixture_cols.iter() {
            total_columns += 1;
            let local_x = fix_col.wx - block_x;
            let local_z = fix_col.wz - block_z;

            let mut col_mismatches: Vec<(i32, u8, u8)> = Vec::new();
            for (si, &sy) in y_sections.iter().enumerate() {
                if let Some(Some((palette, _))) = sections.get(si) {
                    let base_y = sy * 16;
                    for local_y in 0..16i32 {
                        let world_y = base_y + local_y;
                        if world_y < 128 {
                            let state = palette.get(BlockPos::new(local_x, local_y, local_z));
                            let got = beta_id_for_modern(state);
                            let want = fix_col.post_cave[world_y as usize];
                            if got != want {
                                col_mismatches.push((world_y, got, want));
                            }
                        }
                    }
                }
            }
            if !col_mismatches.is_empty() {
                mismatches.push((fix_col.wx, fix_col.wz, col_mismatches));
            }
        }
    }

    if !mismatches.is_empty() {
        let first_10: Vec<String> = mismatches.iter().take(10).map(|(wx, wz, diffs)| {
            let first_diff = diffs.first().map(|(y, got, want)| {
                format!("Y={} got={} want={}", y, got, want)
            }).unwrap_or_default();
            format!("  ({:+5},{:+5}): {} block mismatches [{}]", wx, wz, diffs.len(), first_diff)
        }).collect();

        panic!(
            "\nBETA CAVE PARITY GATE FAILED\n\
             Columns: {} total, {} mismatched\n\
             First offenders:\n{}\n",
            total_columns,
            mismatches.len(),
            first_10.join("\n"),
        );
    }

    assert_eq!(
        mismatches.len(), 0,
        "cave parity: {} mismatches / {} columns",
        mismatches.len(), total_columns
    );
}

/// LegacyRandom draw-count regression pin for chunk (0,0) at seed 12345.
///
/// Counts total RNG advances consumed by the 17x17 carve loop over a
/// stone-filled chunk. This pin detects accidental algorithm changes that
/// alter the draw sequence without necessarily changing the carved output.
///
/// Value recorded on the first green run and asserted on every subsequent run.
const DRAW_COUNT_CHUNK_0_0_SEED_12345: u64 = 1883;

#[test]
fn beta_cave_draw_count_pin() {
    let count = count_rng_draws_for_chunk(0, 0, 12345);

    if DRAW_COUNT_CHUNK_0_0_SEED_12345 == 0 {
        // First run: print the value for pinning.
        println!("DRAW COUNT PIN (chunk 0,0 seed 12345): {}", count);
        assert!(count > 0, "carver must consume RNG draws");
    } else {
        assert_eq!(
            count, DRAW_COUNT_CHUNK_0_0_SEED_12345,
            "LegacyRandom draw count for chunk (0,0) at seed 12345 changed: expected {}, got {}",
            DRAW_COUNT_CHUNK_0_0_SEED_12345, count,
        );
    }
}

// ── Integration smoke test ────────────────────────────────────────────────────

/// Integration smoke: a Beta column generated through the full path (terrain +
/// surface + caves) at seed 12345 has cave air below the surface and lava at/below Y 10.
#[test]
fn generate_column_beta_has_caves() {
    let router = build_beta_router();
    let (biome_source, snapshot) = build_beta_biome_source();
    let cancel = CancellationToken::new();
    let (config, ids) = make_cave_config();

    let world_seed = router.world_seed() as i64;
    let y_sections: Vec<i32> = (0..8).collect();

    let chunk_x = 0i32;
    let chunk_z = 0i32;

    let mut sections = generate_column(
        chunk_x,
        chunk_z,
        &y_sections,
        &router,
        Some((&biome_source, &snapshot)),
        &cancel,
    );

    let mut rng = make_chunk_rng(chunk_x, chunk_z);
    apply_beta_surface(
        &mut sections,
        &y_sections,
        chunk_x * 16,
        chunk_z * 16,
        &router,
        &biome_source,
        &mut rng,
    );

    apply_beta_caves(&mut sections, &y_sections, chunk_x, chunk_z, world_seed, &config, &ids);

    let air = ids.air;
    let lava = ids.lava;

    let mut found_cave_air = false;
    let mut found_lava_below_10 = false;

    for local_x in 0..16i32 {
        for local_z in 0..16i32 {
            // Find surface Y (topmost non-air).
            let mut surface_y = 0i32;
            'surface: for si in (0..y_sections.len()).rev() {
                let sy = y_sections[si];
                if let Some(Some((blocks, _))) = sections.get(si) {
                    let base_y = sy * 16;
                    for local_y in (0..16i32).rev() {
                        if blocks.get(BlockPos::new(local_x, local_y, local_z)) != air {
                            surface_y = base_y + local_y;
                            break 'surface;
                        }
                    }
                }
            }

            // Check below surface for cave air, and at/below Y 10 for lava.
            for (si, &sy) in y_sections.iter().enumerate() {
                if let Some(Some((blocks, _))) = sections.get(si) {
                    let base_y = sy * 16;
                    for local_y in 0..16i32 {
                        let world_y = base_y + local_y;
                        if world_y >= surface_y { continue; }
                        let state = blocks.get(BlockPos::new(local_x, local_y, local_z));
                        if state == air {
                            found_cave_air = true;
                        }
                        if state == lava && world_y <= 10 {
                            found_lava_below_10 = true;
                        }
                    }
                }
            }
        }
    }

    assert!(
        found_cave_air,
        "chunk (0,0) at seed 12345 must contain cave air below the surface"
    );
    assert!(
        found_lava_below_10,
        "chunk (0,0) at seed 12345 must contain lava at/below Y 10 from cave carving"
    );
}

/// Real-pipeline proof: run generate_column + apply_beta_surface + apply_beta_caves
/// (exactly as `chunk.rs` does) across an 8x8 chunk grid at seed 12345 and count
/// air voxels strictly below Y 32. Beta produces no noise caverns, so any air that
/// deep can only come from the carver. Prints per-chunk counts and requires at
/// least one chunk to contain deep cave air.
#[test]
fn beta_real_pipeline_has_cave_air_below_y32() {
    let router = build_beta_router();
    let (biome_source, snapshot) = build_beta_biome_source();
    let cancel = CancellationToken::new();
    let (config, ids) = make_cave_config();
    let world_seed = router.world_seed() as i64;
    let y_sections: Vec<i32> = (0..8).collect();
    let air = ids.air;

    let mut chunks_with_air = 0usize;
    let mut total_air = 0usize;

    for chunk_x in 0..8i32 {
        for chunk_z in 0..8i32 {
            let mut sections = generate_column(
                chunk_x,
                chunk_z,
                &y_sections,
                &router,
                Some((&biome_source, &snapshot)),
                &cancel,
            );
            let mut rng = make_chunk_rng(chunk_x, chunk_z);
            apply_beta_surface(
                &mut sections,
                &y_sections,
                chunk_x * 16,
                chunk_z * 16,
                &router,
                &biome_source,
                &mut rng,
            );
            apply_beta_caves(&mut sections, &y_sections, chunk_x, chunk_z, world_seed, &config, &ids);

            let mut air_below_32 = 0usize;
            for (si, &sy) in y_sections.iter().enumerate() {
                let base_y = sy * 16;
                if base_y >= 32 {
                    continue;
                }
                if let Some(Some((blocks, _))) = sections.get(si) {
                    for local_x in 0..16i32 {
                        for local_z in 0..16i32 {
                            for local_y in 0..16i32 {
                                if base_y + local_y >= 32 {
                                    continue;
                                }
                                if blocks.get(BlockPos::new(local_x, local_y, local_z)) == air {
                                    air_below_32 += 1;
                                }
                            }
                        }
                    }
                }
            }

            if air_below_32 > 0 {
                chunks_with_air += 1;
                total_air += air_below_32;
                eprintln!("chunk ({chunk_x},{chunk_z}): {air_below_32} air voxels below Y 32");
            }
        }
    }

    eprintln!(
        "REAL PIPELINE seed 12345: {chunks_with_air}/64 chunks have cave air below Y 32; {total_air} air voxels total"
    );

    assert!(
        chunks_with_air >= 1,
        "real Beta generation pipeline must produce at least 1 chunk with air below Y 32"
    );
}

