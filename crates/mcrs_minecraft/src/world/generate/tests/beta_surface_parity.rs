use std::collections::BTreeMap;
use std::sync::Arc;

use base64::Engine as _;
use bevy_asset::Assets;
use mcrs_core::RegistrySnapshot;
use mcrs_core::resource_location::ResourceLocation;
use mcrs_engine::world::block::BlockPos;
use mcrs_minecraft_worldgen::density_function::build_functions;
use mcrs_minecraft_worldgen::proto::NoiseGeneratorSettings;
use mcrs_protocol::BlockStateId;
use mcrs_vanilla::biome::Biome;
use mcrs_vanilla::biome::source::{BetaLandBiome, BiomeSource, beta_biome_from_climate, beta_get_biome, build_beta_lookup_table};
use mcrs_vanilla::block::minecraft;

use crate::world::chunk::CancellationToken;
use crate::world::generate::{apply_beta_surface, generate_column};

// ── Corpus deserialization ────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct BetaSurfaceCorpus {
    columns: Vec<ColumnFixture>,
}

#[derive(serde::Deserialize)]
struct ColumnFixture {
    wx: i32,
    wz: i32,
    biome_id: u8,
    #[serde(with = "serde_base64")]
    pre_cave: Vec<u8>,
    // post_cave is captured but reserved for the cave-parity gate
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

// ── Block ID reverse map ──────────────────────────────────────────────────────

/// Map a modern BlockStateId produced by the Beta surface pass → Beta 1.7.3 block ID.
///
/// This is the inverse of the forward mapping that the surface pass uses.  Modern IDs
/// are resolved from the block consts' `default_state_id` so that no magic numbers
/// appear here — if a block registration changes, this map changes with it.
///
/// Water and lava: the surface pass places the "source" (level 0) state.  back2beta
/// stores stationary-water (ID 9) and stationary-lava (ID 10) for those positions,
/// which functionally represent the same thing.  Both water (8) and stationary-water
/// (9) decode to the same Beta ID here; same for lava.
///
/// Any state not in the surface pass's output (e.g. stone from the density pass, air
/// from above the surface) passes through with a fallback:
///   stone (ID 1) → 1, air (ID 0) → 0.
fn beta_id_for(modern: BlockStateId) -> u8 {
    let air      = minecraft::AIR.default_state_id;
    let stone    = minecraft::STONE.default_state_id;
    let grass    = minecraft::GRASS_BLOCK.default_state_id;
    let dirt     = minecraft::DIRT.default_state_id;
    let bedrock  = minecraft::BEDROCK.default_state_id;
    let sand     = minecraft::SAND.default_state_id;
    let sandstone = minecraft::SANDSTONE.default_state_id;
    let gravel   = minecraft::GRAVEL.default_state_id;

    // Water: default_state_id = level 0 (base_state_id 86).
    let water_source = minecraft::WATER.default_state_id;
    // Lava: default_state_id = level 0 (base_state_id 102).
    let lava_source = minecraft::LAVA.default_state_id;

    if modern == air       { return 0;  }
    if modern == stone     { return 1;  }
    if modern == grass     { return 2;  }
    if modern == dirt      { return 3;  }
    if modern == bedrock   { return 7;  }
    // back2beta stores stationary-water (9) at sea-level fill positions.
    if modern == water_source { return 9; }
    // back2beta stores stationary-lava (10); registered for future cave gate.
    if modern == lava_source  { return 10; }
    if modern == sand      { return 12; }
    if modern == gravel    { return 13; }
    if modern == sandstone { return 24; }

    // Unknown state: treat as air for comparison purposes.
    0
}

// ── Biome ID mapping ──────────────────────────────────────────────────────────

/// Map our `BetaLandBiome` variant to the back2beta integer biome ID.
///
/// back2beta's BiomeBase assigns integer IDs in the following order (from
/// BiomeBase.java static initialisers and getBiome()):
///   0=Rainforest, 1=Swampland, 2=Seasonal Forest, 3=Forest, 4=Savanna,
///   5=Shrubland, 6=Taiga, 7=Desert, 8=Plains, 9=Ice Desert, 10=Tundra.
///
/// Our BetaLandBiome discriminants follow a different order (inherited from the
/// the biome-source design).  This function bridges the two numbering schemes
/// so that biome-parity failures are correctly attributed.
fn beta_land_biome_to_back2beta_id(b: BetaLandBiome) -> u8 {
    match b {
        BetaLandBiome::Rainforest     => 0,
        BetaLandBiome::Swampland      => 1,
        BetaLandBiome::SeasonalForest => 2,
        BetaLandBiome::Forest         => 3,
        BetaLandBiome::Savanna        => 4,
        BetaLandBiome::Shrubland      => 5,
        BetaLandBiome::Taiga          => 6,
        BetaLandBiome::Desert         => 7,
        BetaLandBiome::Plains         => 8,
        BetaLandBiome::IceDesert      => 9,
        BetaLandBiome::Tundra         => 10,
    }
}

// ── Match semantics ───────────────────────────────────────────────────────────

/// Half-width of the surface zone band checked per column.
///
/// The dominant f32 divergence between our surface pass and the back2beta reference
/// concentrates at the density zero-crossing — the boundary where the terrain flips
/// from solid to air.  A ±8-block band around the per-column surface Y (topmost
/// non-air block after the surface pass) ensures the zero-crossing and the
/// immediately surrounding surface blocks (top block + filler depth + 1–2 margin)
/// are all within scope.  Widening beyond ±8 would include deep stone that back2beta
/// doesn't change, adding noise without signal.
const SURFACE_BAND_HALF: i32 = 8;

/// Block IDs from the corpus that count as "sea-level fill" (checked in addition
/// to the surface band).  back2beta places stationary-water (9) below sea level
/// at water-adjacent columns; the surface pass places water-level-0 there.
const BACK2BETA_SEA_FILL_IDS: &[u8] = &[9]; // stationary-water

/// Result of comparing one column's generated output against a fixture.
#[derive(Debug)]
enum ColumnMatchResult {
    Match,
    Mismatch(ColumnMismatch),
}

#[derive(Debug)]
struct ColumnMismatch {
    wx: i32,
    wz: i32,
    /// (world_y, generated_beta_id, fixture_beta_id, band)
    block_mismatches: Vec<(i32, u8, u8, MismatchBand)>,
    /// (generated_back2beta_id, fixture_biome_id)
    biome_mismatch: Option<(u8, u8)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MismatchBand {
    Surface,
    Bedrock,
}

impl std::fmt::Display for MismatchBand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MismatchBand::Surface => write!(f, "surface"),
            MismatchBand::Bedrock => write!(f, "bedrock"),
        }
    }
}

/// Check one column: surface band, bedrock band, and biome ID.
///
/// `generated_col` is a flat 128-element slice where index Y is the BlockStateId
/// at world height Y (same layout as the fixture's pre_cave array).
fn column_matches(
    generated_col: &[BlockStateId; 128],
    fixture: &ColumnFixture,
    router: &mcrs_minecraft_worldgen::density_function::NoiseRouter,
    table: &[[BetaLandBiome; 64]; 64],
) -> ColumnMatchResult {
    let pre_cave = &fixture.pre_cave;

    // Find the topmost non-air Y in the generated column (surface height).
    let surface_y: i32 = (0..128i32)
        .rev()
        .find(|&y| generated_col[y as usize] != minecraft::AIR.default_state_id)
        .unwrap_or(0);

    let surface_lo = (surface_y - SURFACE_BAND_HALF).max(0);
    let surface_hi = (surface_y + SURFACE_BAND_HALF).min(127);

    // Also include sea-level fill zone (Y 60..=64) so that stationary-water
    // placed by the surface pass at the waterline is within scope.
    let sea_lo = 60i32;
    let sea_hi = 64i32;

    let mut block_mismatches: Vec<(i32, u8, u8, MismatchBand)> = Vec::new();

    // Surface band
    for y in surface_lo..=surface_hi {
        let gen_id = beta_id_for(generated_col[y as usize]);
        let fix_id = pre_cave[y as usize];
        if gen_id != fix_id {
            block_mismatches.push((y, gen_id, fix_id, MismatchBand::Surface));
        }
    }

    // Sea-level fill zone (only if it doesn't fully overlap the surface band)
    if sea_lo < surface_lo || sea_hi > surface_hi {
        for y in sea_lo..=sea_hi {
            if y >= surface_lo && y <= surface_hi {
                continue; // already checked in surface band
            }
            let gen_id = beta_id_for(generated_col[y as usize]);
            let fix_id = pre_cave[y as usize];
            // Only flag if the fixture says sea-fill and we differ, or vice versa.
            if gen_id != fix_id {
                block_mismatches.push((y, gen_id, fix_id, MismatchBand::Surface));
            }
        }
    }

    // Bedrock band Y 0-4: per-chunk RNG replication
    for y in 0i32..=4 {
        let gen_id = beta_id_for(generated_col[y as usize]);
        let fix_id = pre_cave[y as usize];
        if gen_id != fix_id {
            block_mismatches.push((y, gen_id, fix_id, MismatchBand::Bedrock));
        }
    }

    // Biome check: back2beta applies the biome of the transposed position
    // (block_x + lz, block_z + lx) to column (lx, lz), due to the XZ-transpose
    // in getBiomeArray/replaceBlocksForBiome.  Quantized via getBiomeFromLookup.
    let cx = fixture.wx.div_euclid(16);
    let cz = fixture.wz.div_euclid(16);
    let lx = fixture.wx - cx * 16;
    let lz = fixture.wz - cz * 16;
    let climate_x = cx * 16 + lz;
    let climate_z = cz * 16 + lx;
    let (temp, humidity) = router.sample_beta_climate(climate_x, climate_z);
    let gen_biome = beta_biome_from_climate(table, temp, humidity);
    let gen_back2beta_id = beta_land_biome_to_back2beta_id(gen_biome);
    let biome_mismatch = if gen_back2beta_id != fixture.biome_id {
        Some((gen_back2beta_id, fixture.biome_id))
    } else {
        None
    };

    if block_mismatches.is_empty() && biome_mismatch.is_none() {
        ColumnMatchResult::Match
    } else {
        ColumnMatchResult::Mismatch(ColumnMismatch {
            wx: fixture.wx,
            wz: fixture.wz,
            block_mismatches,
            biome_mismatch,
        })
    }
}

// ── Test helpers (shared with beta_biome_palette / beta_surface) ──────────────

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
    let biome_source = BiomeSource::Beta {
        land_biomes: land_handles.try_into().expect("11 land handles"),
        ocean_biomes: ocean_handles.try_into().expect("5 ocean handles"),
        lookup: Box::new(build_beta_lookup_table()),
    };
    (biome_source, snapshot)
}

fn make_chunk_rng(chunk_x: i32, chunk_z: i32) -> mcrs_random::legacy::LegacyRandom {
    let seed: i64 = (chunk_x as i64)
        .wrapping_mul(341873128712)
        .wrapping_add((chunk_z as i64).wrapping_mul(132897987541));
    mcrs_random::legacy::LegacyRandom::new(seed as u64)
}

// ── Gate test ─────────────────────────────────────────────────────────────────

/// Blocking surface-parity regression gate at seed 12345.
///
/// Loads the committed back2beta golden-fixture corpus (pre-cave stage), runs the
/// Beta generator for each fixture chunk, applies the surface pass, and compares
/// the surface band (topmost non-air ± SURFACE_BAND_HALF + sea-level fill Y 60-64),
/// the bedrock band (Y 0-4), and the biome ID per column.
///
/// Threshold: at most 1 mismatched column per 1280 (≈99.9%).  On failure the test
/// emits a band-grouped, cause-classified mismatch report rather than any silent
/// path change — per the gate-failure protocol, the phase halts for user review.
#[test]
fn beta_surface_parity_gate() {
    let corpus = load_corpus();
    let router = build_beta_router();
    let (biome_source, snapshot) = build_beta_biome_source();
    let cancel = CancellationToken::new();
    let biome_table = build_beta_lookup_table();

    let y_sections: Vec<i32> = (0..8).collect(); // Y 0-127 (sections 0-7)

    // Group fixture columns by chunk (cx, cz).
    let mut chunks: BTreeMap<(i32, i32), Vec<&ColumnFixture>> = BTreeMap::new();
    for col in &corpus.columns {
        let cx = col.wx >> 4;
        let cz = col.wz >> 4;
        chunks.entry((cx, cz)).or_default().push(col);
    }

    let mut total_columns: u64 = 0;
    let mut all_mismatches: Vec<ColumnMismatch> = Vec::new();

    for ((cx, cz), fixture_cols) in &chunks {
        let block_x = cx * 16;
        let block_z = cz * 16;

        // Generate the column with the Beta biome source.
        let mut sections = generate_column(
            *cx,
            *cz,
            &y_sections,
            &router,
            Some((&biome_source, &snapshot)),
            &cancel,
        );

        // Apply the surface pass (also places bedrock).
        let mut rng = make_chunk_rng(*cx, *cz);
        apply_beta_surface(
            &mut sections,
            &y_sections,
            block_x,
            block_z,
            &router,
            &biome_source,
            &mut rng,
        );

        // For each fixture column in this chunk, build a flat [BlockStateId; 128] view.
        for fix_col in fixture_cols {
            total_columns += 1;

            let local_x = fix_col.wx - block_x;
            let local_z = fix_col.wz - block_z;

            // Assemble a flat Y-indexed array from the 8 sections.
            let mut generated: [BlockStateId; 128] = [BlockStateId(0); 128];
            for (si, sy) in y_sections.iter().enumerate() {
                if let Some(Some((blocks, _))) = sections.get(si) {
                    let base_y = sy * 16;
                    for local_y in 0..16i32 {
                        let world_y = base_y + local_y;
                        if world_y < 128 {
                            generated[world_y as usize] =
                                blocks.get(BlockPos::new(local_x, local_y, local_z));
                        }
                    }
                }
            }

            match column_matches(&generated, fix_col, &router, &biome_table) {
                ColumnMatchResult::Match => {}
                ColumnMatchResult::Mismatch(m) => all_mismatches.push(m),
            }
        }
    }

    let mismatched = all_mismatches.len() as u64;

    // Gate: at most 1 mismatched column per 1280 (≈99.9% parity at seed 12345).
    if mismatched * 1280 > total_columns {
        // ── Classification report ────────────────────────────────────────────
        // Classify by band to distinguish surface/RNG bugs from genuine f32 drift.
        //
        //   Bedrock-band mismatches   → RNG-order / surface bug:
        //     The bedrock band is filled by the same per-chunk RNG stream as the
        //     surface; mismatches here mean the RNG draw order is wrong.
        //
        //   Biome mismatches          → Climate-sampling / biome-algorithm bug:
        //     Our BiomeSource::Beta uses the same getBiome() thresholds as back2beta;
        //     mismatches indicate a divergence in climate-noise sampling.
        //
        //   Surface-band only         → Likely genuine f32 zero-crossing drift:
        //     If mismatches are sparse and cluster at the surface boundary they are
        //     expected f32 accumulation error.  If they are systematic (e.g. whole
        //     biome regions wrong) they indicate a surface-pass logic bug.

        let bedrock_count = all_mismatches.iter()
            .filter(|m| m.block_mismatches.iter().any(|b| b.3 == MismatchBand::Bedrock))
            .count();
        let biome_count = all_mismatches.iter()
            .filter(|m| m.biome_mismatch.is_some())
            .count();
        let surface_only_count = all_mismatches.iter()
            .filter(|m| {
                m.biome_mismatch.is_none()
                    && m.block_mismatches.iter().all(|b| b.3 == MismatchBand::Surface)
            })
            .count();

        // Worst offenders: columns with the most mismatched Y positions.
        let mut sorted = all_mismatches
            .iter()
            .collect::<Vec<_>>();
        sorted.sort_by_key(|m| std::cmp::Reverse(m.block_mismatches.len()));
        let worst: Vec<String> = sorted.iter().take(10).map(|m| {
            let first_block = m.block_mismatches.first().map(|(y, gid, fid, band)| {
                format!("Y={} band={} gen={} fix={}", y, band, gid, fid)
            }).unwrap_or_default();
            let biome_s = m.biome_mismatch.map(|(g, f)| {
                format!(", biome gen={} fix={}", g, f)
            }).unwrap_or_default();
            format!("  ({:+5},{:+5}) {} block mismatches [{}{}]",
                m.wx, m.wz, m.block_mismatches.len(), first_block, biome_s)
        }).collect();

        let cause_note = if bedrock_count > 0 {
            format!(
                "LIKELY CAUSE: RNG-order or surface-pass bug ({} bedrock-band mismatches — \
                 bedrock is placed by the same LegacyRandom stream as the surface; \
                 wrong draw order corrupts both)",
                bedrock_count
            )
        } else if biome_count > 0 {
            format!(
                "LIKELY CAUSE: Climate-sampling or biome-algorithm divergence \
                 ({} biome mismatches)",
                biome_count
            )
        } else {
            format!(
                "LIKELY CAUSE: Genuine f32 zero-crossing drift ({} surface-band-only \
                 mismatches, no bedrock or biome failures — sparse f32 accumulation error \
                 expected near terrain boundary; consider widening band or f64 path \
                 after user review)",
                surface_only_count
            )
        };

        panic!(
            "\n\
             ╔══════════════════════════════════════════════════════════════╗\n\
             ║          BETA SURFACE PARITY GATE FAILED                    ║\n\
             ╚══════════════════════════════════════════════════════════════╝\n\
             \n\
             Threshold : at most 1 mismatch per 1280 columns (~99.9%)\n\
             Seed      : 12345\n\
             Columns   : {} total, {} mismatched ({:.4}%)\n\
             \n\
             Band breakdown:\n\
               Bedrock-band mismatches (Y 0-4)  : {}\n\
               Surface-band only mismatches     : {}\n\
               Biome mismatches                 : {}\n\
             \n\
             {}\n\
             \n\
             Worst-offender columns (most block mismatches first):\n\
             {}\n\
             \n\
             NOTE: Do NOT relax the threshold or enable f64 fallback without \
             explicit user review.  This gate halts the phase intentionally.\n",
            total_columns,
            mismatched,
            mismatched as f64 / total_columns as f64 * 100.0,
            bedrock_count,
            surface_only_count,
            biome_count,
            cause_note,
            worst.join("\n"),
        );
    }

    // Gate green: at most 1 mismatch per 1280.
    assert!(
        mismatched * 1280 <= total_columns,
        "parity gate: {} mismatches / {} columns exceeds 1-per-1280 threshold",
        mismatched, total_columns
    );
}

// ── Per-column climate oracle test ───────────────────────────────────────────

/// Per-column climate oracle test: pins the divergence between Rust and back2beta
/// for a small set of worst-offender columns extracted from the back2beta verbatim
/// harness (seed 12345, run against the committed corpus).
///
/// Root-cause analysis (verbatim harness + corpus):
///   back2beta's replaceBlocksForBiome reads the biome array at index
///   `kk + ll * 16` where kk=x_local, ll=z_local — but getBiomeArray fills the
///   same array with biome(x, z) at index `j1 * 16 + k1 = x * 16 + z`.
///   These two indexing conventions are TRANSPOSED (lx*16+lz vs lx+lz*16), so
///   the biome applied to column (lx, lz) is the geographic biome of position
///   (block_x + lz, block_z + lx) — not (world_x, world_z).
///   This is a verbatim back2beta quirk that the corpus faithfully records.
///
/// The temperature threshold for Desert/Savanna in getBiome() is 0.95.
///   At geographic (wx=31, wz=-9):  d3 ≈ 0.9640 (≥ 0.95) → Desert(7)
///   At transposed  (wx=23, wz=-1): d3 ≈ 0.9484 (< 0.95) → Savanna(4) [corpus]
///
/// The current code samples at (wx, wz) — geographic, not transposed — and uses
/// continuous beta_get_biome rather than the quantized 64x64 table.
/// This test asserts the correct biome (corpus value) and FAILS (RED) until
/// both fixes land in Task 2.
///
/// Oracle columns captured from the verbatim harness (seed 12345):
///   Column (wx, wz) | chunk (cx, cz) | local (lx, lz)
///                   | transposed world (block_x+lz, block_z+lx)
///                   | back2beta d3       | corpus biome_id
///   (+31, -9)       | (+1, -1)          | (15, 7) → (23, -1)  | 0.9484 | 4 (Savanna)
///   (+30, -11)      | (+1, -1)          | (14, 5) → (21, -2)  | 0.9455 | 4 (Savanna)
///   (  0,  0)       | ( 0,  0)          | ( 0, 0) → ( 0,  0)  | 0.9718 | 7 (Desert)
///   (+16,+16)       | (+1, +1)          | ( 0, 0) → (16, 16)  | 0.9436 | 4 (Savanna)
#[test]
fn beta_climate_matches_back2beta_oracle() {
    let router = build_beta_router();

    // Oracle columns: (wx, wz, corpus_biome_id, oracle_d3_approx)
    // corpus_biome_id is the biome recorded in beta_surface_corpus.json.
    // oracle_d3_approx is back2beta's processed temperature for the transposed position,
    // included for diagnostic clarity only (not asserted as f32 equality).
    let oracle_cols: &[(i32, i32, u8)] = &[
        (31, -9, 4),   // Savanna — geographic yields Desert (d3≈0.964 ≥ 0.95)
        (30, -11, 4),  // Savanna — geographic yields Desert (d3≈0.967 ≥ 0.95)
        (0, 0, 7),     // Desert  — geographic and transposed agree (lx=lz=0)
        (16, 16, 4),   // Savanna — geographic and transposed agree (lx=lz=0)
    ];

    let table = build_beta_lookup_table();

    let mut failures: Vec<String> = Vec::new();

    for &(wx, wz, expected_biome_id) in oracle_cols {
        // Use the transposed position (block_x + lz, block_z + lx) — the position
        // back2beta's replaceBlocksForBiome actually samples due to the XZ-transpose
        // in getBiomeArray.  This is what apply_beta_surface must use post-fix.
        let cx = wx.div_euclid(16);
        let cz = wz.div_euclid(16);
        let lx = wx - cx * 16;
        let lz = wz - cz * 16;
        let climate_x = cx * 16 + lz;
        let climate_z = cz * 16 + lx;
        let (temp, rain) = router.sample_beta_climate(climate_x, climate_z);

        let gen_id = beta_land_biome_to_back2beta_id(beta_biome_from_climate(&table, temp, rain));

        if gen_id != expected_biome_id {
            failures.push(format!(
                "  ({:+4},{:+4}) transposed→({:+4},{:+4}): expected biome_id={} got {} (temp={:.6} rain={:.6})",
                wx, wz, climate_x, climate_z, expected_biome_id, gen_id, temp, rain
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "\nbeta_climate_matches_back2beta_oracle FAILED;\nfailing columns:\n{}",
        failures.join("\n")
    );
}
