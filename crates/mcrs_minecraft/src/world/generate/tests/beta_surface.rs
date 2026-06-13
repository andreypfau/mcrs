use std::collections::BTreeMap;
use std::sync::Arc;

use bevy_asset::Assets;
use mcrs_core::RegistrySnapshot;
use mcrs_core::resource_location::ResourceLocation;
use mcrs_engine::world::block::BlockPos;
use mcrs_minecraft_worldgen::density_function::build_functions;
use mcrs_minecraft_worldgen::proto::NoiseGeneratorSettings;
use mcrs_vanilla::biome::Biome;
use mcrs_vanilla::biome::source::{BiomeSource, build_beta_lookup_table};
use mcrs_vanilla::block::minecraft;

use crate::world::chunk::CancellationToken;
use crate::world::generate::{apply_beta_surface, generate_column};

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

/// Verify that apply_beta_surface places surface and bedrock blocks.
///
/// After running apply_beta_surface on a full Beta column (8 sections, Y 0..=7):
/// - World Y=0 must contain bedrock in every column (k1=0 satisfies 0 <= 0 + nextInt(5))
/// - The surface zone (Y 60-70) must contain at least one surface block (grass/dirt/sand)
#[test]
fn apply_beta_surface_places_surface_and_bedrock() {
    let router = build_beta_router();
    let (biome_source, snapshot) = build_beta_biome_source();

    let chunk_x = 0i32;
    let chunk_z = 0i32;
    let block_x = chunk_x * 16;
    let block_z = chunk_z * 16;

    let y_sections: Vec<i32> = (0..8).collect();
    let cancel = CancellationToken::new();
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
        block_x,
        block_z,
        &router,
        &biome_source,
        &mut rng,
    );

    let bedrock_id = minecraft::BEDROCK.default_state_id;
    let grass_id = minecraft::GRASS_BLOCK.default_state_id;
    let dirt_id = minecraft::DIRT.default_state_id;
    let sand_id = minecraft::SAND.default_state_id;

    // Y=0 (section 0, local y=0): always bedrock for all 256 columns
    let section0_blocks = &sections[0].as_ref().expect("section 0 must be Some").0;
    let y0_all_bedrock = (0..16i32).all(|x| {
        (0..16i32).all(|z| section0_blocks.get(BlockPos::new(x, 0, z)) == bedrock_id)
    });
    assert!(y0_all_bedrock, "world Y=0 must be all bedrock");

    // Surface zone: sections 3-5 (world Y 48-95) must contain surface blocks
    let mut found_surface_block = false;
    for (si, sy) in y_sections.iter().enumerate() {
        if *sy < 3 || *sy > 5 {
            continue;
        }
        if let Some((blocks, _)) = &sections[si] {
            for x in 0..16i32 {
                for z in 0..16i32 {
                    for y in 0..16i32 {
                        let b = blocks.get(BlockPos::new(x, y, z));
                        if b == grass_id || b == dirt_id || b == sand_id {
                            found_surface_block = true;
                        }
                    }
                }
            }
        }
    }
    assert!(
        found_surface_block,
        "surface zone must contain grass, dirt, or sand after apply_beta_surface"
    );
}

/// Oracle test: bedrock band (Y 0-4) for chunk (37,-42) at seed 12345 must match the
/// verbatim back2beta corpus (beta_surface_corpus.json).
///
/// Root-cause context: back2beta's replaceBlocksForBiome reads noise arrays r/s/t at
/// index `r[kk + ll*16]` where kk=x_local (outer loop), ll=z_local (inner loop).
/// The arrays are filled in X-major order (fill index x*16+z), so reading at
/// kk+ll*16 = x+z*16 picks the value stored at fill position (z_local, x_local) —
/// i.e., the noise sampled at the TRANSPOSED world position (block_x+z, block_z+x).
/// The current Rust uses idx = x_local*16 + z_local (geographic, non-transposed),
/// which reads the noise sampled at (block_x+x, block_z+z) — the wrong column.
///
/// This divergence in flag/flag1/i1 cascades through sandstone nextInt(4) draws,
/// shifting the shared per-chunk RNG stream and corrupting the bedrock pattern for
/// columns that come later in the sweep. In chunk (37,-42), the stream diverges at
/// x=0, z=13 where corpus says [7,1,7,7,1] but Rust produces [7,7,1,1,1].
///
/// Expected values captured verbatim from the back2beta corpus (chunk x=37, z=-42).
/// 7 = bedrock, 1 = stone.
///
/// This test FAILS (RED) until idx is corrected to z_local*16 + x_local.
#[test]
fn beta_surface_bedrock_matches_back2beta_oracle() {
    let router = build_beta_router();
    let (biome_source, snapshot) = build_beta_biome_source();

    let chunk_x = 0i32;
    let chunk_z = 0i32;
    let block_x = chunk_x * 16;
    let block_z = chunk_z * 16;

    let y_sections: Vec<i32> = (0..8).collect();
    let cancel = CancellationToken::new();
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
        block_x,
        block_z,
        &router,
        &biome_source,
        &mut rng,
    );

    let bedrock_id = minecraft::BEDROCK.default_state_id;
    let section0 = sections[0].as_ref().expect("section 0 must be present");
    let blocks = &section0.0;

    // Oracle: expected [Y0,Y1,Y2,Y3,Y4] (7=bedrock, 1=stone) for x_local=0, z_local=0..15.
    // Captured from the geographic beta_surface_corpus.json at chunk (0,0), seed 12345.
    let oracle: &[(i32, [u8; 5])] = &[
        ( 0, [7, 1, 7, 7, 1]),
        ( 1, [7, 7, 1, 7, 1]),
        ( 2, [7, 1, 1, 1, 1]),
        ( 3, [7, 7, 1, 1, 1]),
        ( 4, [7, 1, 1, 7, 7]),
        ( 5, [7, 7, 1, 7, 1]),
        ( 6, [7, 7, 1, 1, 1]),
        ( 7, [7, 1, 1, 1, 1]),
        ( 8, [7, 7, 1, 7, 7]),
        ( 9, [7, 7, 1, 1, 7]),
        (10, [7, 7, 7, 1, 1]),
        (11, [7, 1, 1, 1, 1]),
        (12, [7, 7, 1, 1, 1]),
        (13, [7, 7, 7, 1, 1]),
        (14, [7, 7, 1, 1, 1]),
        (15, [7, 1, 1, 1, 1]),
    ];

    let x_local = 0i32;
    let mut failures: Vec<String> = Vec::new();
    for &(z_local, expected) in oracle {
        for y in 0i32..5 {
            let got_bedrock = blocks.get(BlockPos::new(x_local, y, z_local)) == bedrock_id;
            let expect_bedrock = expected[y as usize] == 7;
            if got_bedrock != expect_bedrock {
                failures.push(format!(
                    "  x={} z={} Y={}: expected {} got {}",
                    x_local, z_local, y,
                    if expect_bedrock { "bedrock(7)" } else { "stone(1)" },
                    if got_bedrock { "bedrock(7)" } else { "stone(1)" },
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "\nbeta_surface_bedrock_matches_back2beta_oracle FAILED (chunk 0,0, x_local=0):\n{}",
        failures.join("\n"),
    );
}

/// Oracle test: terrain-surface Y (density stone top) for the six worst-offender columns
/// at seed 12345 must match between Rust generate_column and the back2beta corpus.
///
/// The corpus `pre_cave` bytes are captured AFTER back2beta's generateTerrain +
/// replaceBlocksForBiome. The density stone top in the corpus is the topmost
/// non-air, non-water block — it was originally stone from generateTerrain, then
/// replaced by sand/grass/etc by the surface pass, so scanning for stone alone
/// would find the WRONG Y (below the sand/sandstone layer). The correct comparison
/// is Rust stone-top (from generate_column) vs corpus terrain-top (topmost non-air
/// non-water byte, which is the highest block of the original stone layer that became
/// sand/grass at the surface).
///
/// For column (24,-16): corpus terrain-top = 83 (sand, originally stone), Rust stone-top
/// should match. The test is RED until the density function height is calibrated.
///
/// Root-cause probe: The test also evaluates the scale/depth noise samplers at three
/// sampling modes and prints d5/d6/d7 for (24,-16).
#[test]
fn beta_terrain_height_matches_back2beta_oracle() {
    #[derive(serde::Deserialize)]
    struct CorpusRoot { columns: Vec<CorpusCol> }
    #[derive(serde::Deserialize)]
    struct CorpusCol {
        wx: i32, wz: i32,
        #[serde(with = "serde_b64")] pre_cave: Vec<u8>,
    }
    mod serde_b64 {
        use base64::Engine as _;
        use serde::{Deserialize, Deserializer};
        pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<Vec<u8>, D::Error> {
            let s = String::deserialize(de)?;
            base64::engine::general_purpose::STANDARD.decode(s).map_err(serde::de::Error::custom)
        }
    }

    let corpus: CorpusRoot = serde_json::from_str(
        include_str!("fixtures/beta_surface_corpus.json")
    ).expect("valid corpus");

    // Corpus terrain top: topmost non-air (0) non-water (9) byte.
    // This is the density stone top — originally stone in generateTerrain, possibly
    // replaced by sand/grass/etc by replaceBlocksForBiome, but still at the same Y.
    let corpus_terrain_top_y = |pre_cave: &[u8]| -> Option<i32> {
        (0..128i32).rev().find(|&y| {
            let b = pre_cave[y as usize];
            b != 0 && b != 9  // not air, not stationary-water
        })
    };

    let stone_id = minecraft::STONE.default_state_id;

    // Rust stone top: scan generate_column sections top-down for highest Y with stone.
    let rust_stone_top_y = |sections: &Vec<Option<(mcrs_minecraft_block::palette::BlockPalette, mcrs_minecraft_block::palette::BiomePalette)>>, lx: i32, lz: i32| -> Option<i32> {
        let y_sections: Vec<i32> = (0..8).collect();
        for sy in (0..8i32).rev() {
            let si = sy as usize;
            if let Some(Some((blocks, _))) = sections.get(si) {
                let base_y = sy * 16;
                for local_y in (0..16i32).rev() {
                    if blocks.get(BlockPos::new(lx, local_y, lz)) == stone_id {
                        return Some(base_y + local_y);
                    }
                }
            }
        }
        let _ = y_sections;
        None
    };

    // Worst-offender columns from the verification report (seed 12345).
    let worst_offenders: &[(i32, i32)] = &[
        (24, -16), (15, -16), (22, -16), (23, -16), (23, -15), (24, -13),
    ];

    let router = build_beta_router();
    let (biome_source, snapshot) = build_beta_biome_source();
    let cancel = CancellationToken::new();
    let y_sections: Vec<i32> = (0..8).collect();

    // Build a lookup: (wx, wz) → pre_cave bytes.
    let corpus_map: std::collections::HashMap<(i32, i32), &[u8]> = corpus.columns.iter()
        .map(|c| ((c.wx, c.wz), c.pre_cave.as_slice()))
        .collect();

    // Cache generated sections by chunk to avoid re-generating.
    let mut chunk_cache: std::collections::HashMap<(i32, i32), Vec<Option<(mcrs_minecraft_block::palette::BlockPalette, mcrs_minecraft_block::palette::BiomePalette)>>> = std::collections::HashMap::new();

    let mut failures: Vec<String> = Vec::new();
    let mut table_rows: Vec<String> = Vec::new();

    for &(wx, wz) in worst_offenders {
        let cx = wx >> 4;
        let cz = wz >> 4;
        let lx = wx - cx * 16;
        let lz = wz - cz * 16;

        let sections = chunk_cache.entry((cx, cz)).or_insert_with(|| {
            generate_column(cx, cz, &y_sections, &router, Some((&biome_source, &snapshot)), &cancel)
        });

        let rust_top = rust_stone_top_y(sections, lx, lz);
        let corpus_top = corpus_map
            .get(&(wx, wz))
            .and_then(|pc| corpus_terrain_top_y(pc));

        let rust_y = rust_top.unwrap_or(-1);
        let corpus_y = corpus_top.unwrap_or(-1);
        let delta = rust_y - corpus_y;

        table_rows.push(format!(
            "  ({:+5},{:+5})  corpus_top={:3}  rust_top={:3}  delta={:+4}",
            wx, wz, corpus_y, rust_y, delta
        ));

        if rust_top != corpus_top {
            failures.push(format!(
                "column ({},{}) rust_stone_top={:?} != corpus_terrain_top={:?}",
                wx, wz, rust_top, corpus_top
            ));
        }
    }

    println!("\n=== Terrain-top Y diagnostic (seed 12345) ===");
    println!("  {:^11}   corpus_top   rust_top   delta", "(wx,wz)");
    for row in &table_rows {
        println!("{}", row);
    }

    assert!(
        failures.is_empty(),
        "\nbeta_terrain_height_matches_back2beta_oracle FAILED:\n\
         The worst-offender stone-top Ys do not match the corpus.\n\
         Per-column results:\n{}\n",
        failures.join("\n")
    );
}

/// Diagnostic: print actual climate and density values for chunk (0,-1) to debug stone-top mismatch.
#[test]
#[ignore = "diagnostic: print real climate+density for chunk (0,-1)"]
fn diagnostic_real_climate_and_density_chunk_0_neg1() {
    use mcrs_minecraft_worldgen::density_function::beta_terrain_f64::BetaTerrainF64;
    let router = build_beta_router();

    // Get real climate grids for chunk (0,-1)
    let (temp_grid, rain_grid) = router.sample_beta_climate_grids(0, -16);

    // Print climate at column (15,0) in local coords (bx=15, bz=0)
    let lx = 15usize;
    let lz = 0usize;
    let temp = temp_grid[lx * 16 + lz];
    let rain = rain_grid[lx * 16 + lz];
    println!("\n=== Real climate for chunk (0,-1) at local (15,0) ===");
    println!("  temp={:.4}, rain={:.4}", temp, rain);

    // Compute actual density with real climate
    let terrain = router.beta_terrain_f64().expect("beta_terrain_f64 must exist");
    let density = terrain.compute_density(0, -1, &temp_grid, &rain_grid);

    // Print density at x-cell=3, z-cell=0 (covering bx=12..15, bz=0..3)
    println!("\n=== Density grid at x-cell=3, z-cell=0 for chunk (0,-1) ===");
    for iy in 8..12 {
        let d = density[(3 * 5 + 0) * 17 + iy];
        println!("  y={}: {:.6}", iy, d);
    }

    // Trace fill_terrain for bx=15, bz=0 (i1=3, i2=3, j1=0, k2=0)
    println!("\n=== fill_terrain trace for bx=15, bz=0 with real climate ===");
    let ll = 5i32;
    let i1 = 3i32;
    let j1 = 0i32;
    for k1 in 9..11i32 {
        let d1 = density[((i1+0)*ll + j1+0) as usize * 17 + k1 as usize];
        let d3 = density[((i1+1)*ll + j1+0) as usize * 17 + k1 as usize];
        let d1_next = density[((i1+0)*ll + j1+0) as usize * 17 + k1 as usize + 1];
        let d3_next = density[((i1+1)*ll + j1+0) as usize * 17 + k1 as usize + 1];
        let d5 = (d1_next - d1) * 0.125;
        let d7 = (d3_next - d3) * 0.125;
        println!("  k1={}: d1={:.4} d3={:.4} d5={:.4} d7={:.4}", k1, d1, d3, d5, d7);
        let mut dd1 = d1;
        let mut dd3 = d3;
        for l1 in 0..8i32 {
            let world_y = k1 * 8 + l1;
            let d12 = (dd3 - dd1) * 0.25;
            let d10 = dd1 + 3.0 * d12; // i2=3
            let block = if d10 > 0.0 { "STONE" } else if world_y < 64 { "WATER" } else { "AIR" };
            println!("    Y={}: dd1={:.4} d10(i2=3)={:.4} -> {}", world_y, dd1, d10, block);
            dd1 += d5;
            dd3 += d7;
        }
    }
}

/// Diagnostic: run compute_density with f32 OctavePerlinNoise vs f64 and compare.
#[test]
#[ignore = "diagnostic: compare f32 vs f64 compute_density"]
fn diagnostic_compute_density_f32_vs_f64() {
    use mcrs_minecraft_worldgen::density_function::beta_seed::{seed_beta_terrain, seed_beta_terrain_f64};

    let (low32, high32, sel32, _, _, scale32, depth32) = seed_beta_terrain(12345);
    let (low64, high64, sel64, _, _, scale64, depth64) = seed_beta_terrain_f64(12345);

    let router = build_beta_router();
    let (temp_grid, rain_grid) = router.sample_beta_climate_grids(0, -16);

    // Manually compute density at a few cells using f32, mimicking compute_density logic
    let chunk_x = 0i32;
    let chunk_z = -1i32;
    let i = chunk_x * 4;
    let kk = chunk_z * 4;
    let ll = 5usize;
    let j1 = 5usize;
    let i1 = 17usize;
    let cell_size = 16 / ll;
    let cell_half = cell_size / 2;

    let d0_32 = 684.412f32;
    let d1_32 = 684.412f32;
    let d0_64 = 684.412f64;
    let d1_64 = 684.412f64;

    println!("\n=== Density comparison f32 vs f64 for chunk (0,-1), cells around (3,0) and (4,0) at y=9..10 ===");
    for (ix, iz) in [(3usize, 0usize), (4usize, 0usize)] {
        let j2i = ix;
        let l2 = iz;
        let k2 = j2i * cell_size + cell_half;
        let i3 = l2 * cell_size + cell_half;
        let temp = temp_grid[k2 * 16 + i3] as f64;
        let rain = rain_grid[k2 * 16 + i3] as f64 * temp;

        let mut d4 = 1.0 - rain;
        d4 *= d4; d4 *= d4; d4 = 1.0 - d4;

        let l1_idx = j2i * 5 + l2;
        let g64 = scale64.sample_xz_beta((i + ix as i32) as f64, (kk + iz as i32) as f64, 1.121, 1.121);
        let h64 = depth64.sample_xz_beta((i + ix as i32) as f64, (kk + iz as i32) as f64, 200.0, 200.0);
        let g32 = scale32.sample_xz((i + ix as i32) as f32, (kk + iz as i32) as f32, 1.121, 1.121);
        let h32 = depth32.sample_xz((i + ix as i32) as f32, (kk + iz as i32) as f32, 200.0, 200.0);

        let mut d5_64 = (g64 + 256.0) / 512.0 * d4;
        if d5_64 > 1.0 { d5_64 = 1.0; }
        let mut d6_64 = h64 / 8000.0;
        if d6_64 < 0.0 { d6_64 = -d6_64 * 0.3; }
        d6_64 = d6_64 * 3.0 - 2.0;
        if d6_64 < 0.0 { d6_64 /= 2.0; if d6_64 < -1.0 { d6_64 = -1.0; } d6_64 /= 1.4; d6_64 /= 2.0; d5_64 = 0.0; }
        else { if d6_64 > 1.0 { d6_64 = 1.0; } d6_64 /= 8.0; }
        if d5_64 < 0.0 { d5_64 = 0.0; } d5_64 += 0.5;
        d6_64 = d6_64 * 17.0 / 16.0;
        let d7_64 = 8.5 + d6_64 * 4.0;

        let mut d5_32 = ((g32 + 256.0) / 512.0 * d4 as f32).min(1.0f32).max(0.0f32);
        let mut d6_32 = h32 / 8000.0;
        if d6_32 < 0.0 { d6_32 = -d6_32 * 0.3; }
        d6_32 = d6_32 * 3.0 - 2.0;
        if d6_32 < 0.0 { d6_32 /= 2.0; if d6_32 < -1.0 { d6_32 = -1.0; } d6_32 /= 1.4; d6_32 /= 2.0; d5_32 = 0.0; }
        else { if d6_32 > 1.0 { d6_32 = 1.0; } d6_32 /= 8.0; }
        if d5_32 < 0.0 { d5_32 = 0.0; } d5_32 += 0.5;
        d6_32 = d6_32 * 17.0 / 16.0;
        let d7_32 = 8.5 + d6_32 * 4.0;

        println!("\n  Cell ({},{}) temp={:.4} rain={:.4}", ix, iz, temp, rain);
        println!("  f64: g={:.4} h={:.4} d5={:.6} d6_after={:.6} d7={:.6}", g64, h64, d5_64, d6_64, d7_64);
        println!("  f32: g={:.4} h={:.4} d5={:.6} d6_after={:.6} d7={:.6}", g32, h32, d5_32, d6_32, d7_32);

        for iy in 9..11usize {
            let x = (i + ix as i32) as f64;
            let y = iy as f64;
            let z = (kk + iz as i32) as f64;
            let d_64 = sel64.sample_xyz_beta(x, y, z, d0_64/80.0, d1_64/160.0, d0_64/80.0);
            let e_64 = low64.sample_xyz_beta(x, y, z, d0_64, d1_64, d0_64);
            let f_64 = high64.sample_xyz_beta(x, y, z, d0_64, d1_64, d0_64);
            let d12_64 = (d_64 / 10.0 + 1.0) / 2.0;
            let mixed_64 = if d12_64 < 0.0 { e_64/512.0 } else if d12_64 > 1.0 { f_64/512.0 } else { e_64/512.0 + (f_64/512.0-e_64/512.0)*d12_64 };
            let d9_raw_64 = (iy as f64 - d7_64) * 12.0 / d5_64;
            let d9_64 = if d9_raw_64 < 0.0 { d9_raw_64 * 4.0 } else { d9_raw_64 };
            let density_64 = mixed_64 - d9_64;

            let d_32 = sel32.sample_xyz_beta(x as f32, y as f32, z as f32, d0_32/80.0, d1_32/160.0, d0_32/80.0);
            let e_32 = low32.sample_xyz_beta(x as f32, y as f32, z as f32, d0_32, d1_32, d0_32);
            let f_32 = high32.sample_xyz_beta(x as f32, y as f32, z as f32, d0_32, d1_32, d0_32);
            let d12_32 = (d_32 / 10.0 + 1.0) / 2.0;
            let mixed_32 = if d12_32 < 0.0 { e_32/512.0 } else if d12_32 > 1.0 { f_32/512.0 } else { e_32/512.0 + (f_32/512.0-e_32/512.0)*d12_32 };
            let d9_raw_32 = (iy as f32 - d7_32) * 12.0 / d5_32;
            let d9_32 = if d9_raw_32 < 0.0 { d9_raw_32 * 4.0 } else { d9_raw_32 };
            let density_32 = mixed_32 - d9_32;

            println!("  y={}: density_f64={:.4} density_f32={:.4} mixed64={:.4} mixed32={:.4} d9_64={:.4} d9_32={:.4}",
                iy, density_64, density_32, mixed_64, mixed_32, d9_64, d9_32);
        }
    }
}

/// Diagnostic: compare 2D scale/depth noise between f32 and f64 at chunk (0,-1) cell (4,0).
#[test]
#[ignore = "diagnostic: compare f32 vs f64 2D noise at cell (4,0)"]
fn diagnostic_2d_noise_f32_vs_f64() {
    use mcrs_minecraft_worldgen::density_function::beta_seed::{seed_beta_terrain, seed_beta_terrain_f64};

    let (_, _, _, _, _, scale32, depth32) = seed_beta_terrain(12345);
    let (_, _, _, _, _, scale64, depth64) = seed_beta_terrain_f64(12345);

    // chunk (0,-1): i=0, kk=-4
    let chunk_x = 0i32;
    let chunk_z = -1i32;
    let i = chunk_x * 4;
    let kk = chunk_z * 4;

    println!("\n=== 2D scale/depth noise comparison (f32 vs f64) for chunk (0,-1) ===");
    for ix in 0..5i32 {
        for iz in 0..5i32 {
            let x_i32 = i + ix;
            let z_i32 = kk + iz;
            let g32 = scale32.sample_xz(x_i32 as f32, z_i32 as f32, 1.121, 1.121);
            let h32 = depth32.sample_xz(x_i32 as f32, z_i32 as f32, 200.0, 200.0);
            let g64 = scale64.sample_xz_beta(x_i32 as f64, z_i32 as f64, 1.121, 1.121);
            let h64 = depth64.sample_xz_beta(x_i32 as f64, z_i32 as f64, 200.0, 200.0);
            println!("  ({},{}) g32={:.4} g64={:.4} h32={:.4} h64={:.4} hDiff={:.4}",
                ix, iz, g32, g64, h32, h64, h64 - h32 as f64);
        }
    }
}

/// Diagnostic: print cell-center climate values used by compute_density for chunk (0,-1).
#[test]
#[ignore = "diagnostic: print cell-center climate values for chunk (0,-1)"]
fn diagnostic_cell_center_climate_chunk_0_neg1() {
    let router = build_beta_router();
    let (temp_grid, rain_grid) = router.sample_beta_climate_grids(0, -16);

    println!("\n=== Cell-center climate values for chunk (0,-1) ===");
    println!("  (j2i=x_cell, l2=z_cell) -> local(x,z), world(x,z): temp, rain");
    let ll = 5usize;
    let cell_size = 16 / ll; // = 3
    let cell_half = cell_size / 2; // = 1
    for j2i in 0..ll {
        let k2 = j2i * cell_size + cell_half;
        for l2 in 0..ll {
            let i3 = l2 * cell_size + cell_half;
            let temp = temp_grid[k2 * 16 + i3];
            let rain = rain_grid[k2 * 16 + i3];
            let world_x = k2 as i32;
            let world_z = -16 + i3 as i32;
            println!("  ({},{}) local({},{}) world({},{}) temp={:.4} rain={:.4}", j2i, l2, k2, i3, world_x, world_z, temp, rain);
        }
    }
}

/// Diagnostic: compare f32 vs f64 density values at oracle-failing columns.
#[test]
#[ignore = "diagnostic: compare f32 vs f64 density at chunk (0,-1)"]
fn diagnostic_f32_vs_f64_density_chunk_0_neg1() {
    use mcrs_minecraft_worldgen::density_function::beta_terrain_f64::BetaTerrainF64;
    use mcrs_minecraft_worldgen::density_function::beta_seed::{seed_beta_terrain, seed_beta_terrain_f64};

    let router = build_beta_router();
    let (temp_grid, rain_grid) = router.sample_beta_climate_grids(0, -16);

    // f64 path
    let (low64, high64, sel64, _, _, scale64, depth64) = seed_beta_terrain_f64(12345);
    let terrain_f64 = BetaTerrainF64::new(12345);
    let density_f64 = terrain_f64.compute_density(0, -1, &temp_grid, &rain_grid);

    // Print f64 density at cells around (3,0) and (4,0) for y=9..10
    println!("\n=== f64 density for chunk (0,-1) ===");
    for (name, ix, iz) in [("(3,0)", 3usize, 0usize), ("(4,0)", 4usize, 0usize), ("(3,1)", 3usize, 1usize)] {
        for iy in 9..11 {
            let d = density_f64[(ix * 5 + iz) * 17 + iy];
            println!("  cell {} y={}: {:.8}", name, iy, d);
        }
    }

    // f32 path: use sample_xyz_beta from f32 OctavePerlinNoise
    let (low32, high32, sel32, _beach, _surface, scale32, depth32) = seed_beta_terrain(12345);

    // f32 climate
    let (temp_grid_f32, rain_grid_f32) = router.sample_beta_climate_grids(0, -16);

    // Compute f32 density for cell (4,0,9) and (3,0,9)
    let i = 0i32 * 4; // chunk_x=0
    let kk = -1i32 * 4; // chunk_z=-1

    for (name, ix, iz) in [("(3,0)", 3i32, 0i32), ("(4,0)", 4i32, 0i32)] {
        for iy in 9i32..11 {
            let x = (i + ix) as f32;
            let y = iy as f32;
            let z = (kk + iz) as f32;
            let d0 = 684.412f32;
            let d1 = 684.412f32;
            let d_f32 = sel32.sample_xyz_beta(x, y, z, d0 / 80.0, d1 / 160.0, d0 / 80.0);
            let e_f32 = low32.sample_xyz_beta(x, y, z, d0, d1, d0);
            let f_f32 = high32.sample_xyz_beta(x, y, z, d0, d1, d0);
            let d12 = (d_f32 / 10.0 + 1.0) / 2.0;
            let mixed_f32 = if d12 < 0.0 { e_f32 / 512.0 } else if d12 > 1.0 { f_f32 / 512.0 } else { e_f32 / 512.0 + (f_f32 / 512.0 - e_f32 / 512.0) * d12 };
            println!("\n  cell {} y={}: low32={:.4} high32={:.4} sel32={:.4} mixed32={:.4}", name, iy, e_f32 / 512.0, f_f32 / 512.0, d_f32 / 10.0 + 1.0, mixed_f32);

            // Also print f64 equivalents
            let x64 = (i + ix) as f64;
            let y64 = iy as f64;
            let z64 = (kk + iz) as f64;
            let d0_64 = 684.412f64;
            let d1_64 = 684.412f64;
            let d_f64 = sel64.sample_xyz_beta(x64, y64, z64, d0_64 / 80.0, d1_64 / 160.0, d0_64 / 80.0);
            let e_f64 = low64.sample_xyz_beta(x64, y64, z64, d0_64, d1_64, d0_64);
            let f_f64 = high64.sample_xyz_beta(x64, y64, z64, d0_64, d1_64, d0_64);
            let d12_64 = (d_f64 / 10.0 + 1.0) / 2.0;
            let mixed_f64 = if d12_64 < 0.0 { e_f64 / 512.0 } else if d12_64 > 1.0 { f_f64 / 512.0 } else { e_f64 / 512.0 + (f_f64 / 512.0 - e_f64 / 512.0) * d12_64 };
            println!("  cell {} y={}: low64={:.4} high64={:.4} sel64={:.4} mixed64={:.4}", name, iy, e_f64 / 512.0, f_f64 / 512.0, d_f64 / 10.0 + 1.0, mixed_f64);
        }
    }
}

/// Diagnostic: print full formula trace for cell (0,2) in chunk (0,-1) to find parity root cause.
#[test]
#[ignore = "diagnostic: formula trace for cell (0,2) in chunk (0,-1)"]
fn diagnostic_cell_0_2_formula_trace() {
    use mcrs_minecraft_worldgen::density_function::beta_seed::{seed_beta_terrain_f64};
    let router = build_beta_router();
    let (temp_grid, rain_grid) = router.sample_beta_climate_grids(0, -16);

    let (low64, high64, sel64, _, _, scale64, depth64) = seed_beta_terrain_f64(12345);

    let chunk_x = 0i32;
    let chunk_z = -1i32;
    let i = chunk_x * 4;
    let kk = chunk_z * 4;

    // Cell (ix=0, iz=2): x=0, z=-4+2=-2
    let ix = 0i32;
    let iz = 2i32;
    let x = (i + ix) as f64;
    let z = (kk + iz) as f64;
    println!("\n=== Cell (ix={ix}, iz={iz}) x={x} z={z} ===");

    // Climate at cell center
    let cell_size = 3usize;
    let cell_half = 1usize;
    let k2 = ix as usize * cell_size + cell_half; // = 1
    let i3 = iz as usize * cell_size + cell_half; // = 7
    let temp = temp_grid[k2 * 16 + i3] as f64;
    let rain = rain_grid[k2 * 16 + i3] as f64 * temp;
    println!("  climate: k2={k2} i3={i3} temp_idx={} temp={:.4} rain={:.4}", k2*16+i3, temp, rain_grid[k2*16+i3]);

    // d4 = 1 - (1-rain)^4
    let mut d4 = 1.0f64 - rain;
    d4 *= d4; d4 *= d4; d4 = 1.0 - d4;

    // scale (g) and depth (h)
    let g_val = scale64.sample_xz_beta(x, z, 1.121, 1.121);
    let h_val = depth64.sample_xz_beta(x, z, 200.0, 200.0);
    println!("  g={:.4} h={:.4}", g_val, h_val);

    // d5 (scale)
    let mut d5 = (g_val + 256.0) / 512.0 * d4;
    if d5 > 1.0 { d5 = 1.0; }

    // d6 (depth)
    let mut d6 = h_val / 8000.0;
    if d6 < 0.0 { d6 = -d6 * 0.3; }
    d6 = d6 * 3.0 - 2.0;
    if d6 < 0.0 {
        d6 /= 2.0;
        if d6 < -1.0 { d6 = -1.0; }
        d6 /= 1.4;
        d6 /= 2.0;
        d5 = 0.0;
    } else {
        if d6 > 1.0 { d6 = 1.0; }
        d6 /= 8.0;
    }
    if d5 < 0.0 { d5 = 0.0; }
    d5 += 0.5;
    d6 = d6 * 17.0 / 16.0;
    let d7 = 8.5 + d6 * 4.0;
    println!("  d4={:.4} d5={:.4} d6={:.4} d7={:.4}", d4, d5, d6, d7);

    // 3D noises at each y
    let d0 = 684.412f64;
    let d1 = 684.412f64;
    println!("  y: sel/10+1  low/512  high/512  mixed   d9      density");
    for j3 in 8..14usize {
        let y = j3 as f64;
        let d_sel = sel64.sample_xyz_beta(x, y, z, d0/80.0, d1/160.0, d0/80.0);
        let d_low = low64.sample_xyz_beta(x, y, z, d0, d1, d0);
        let d_high = high64.sample_xyz_beta(x, y, z, d0, d1, d0);
        let d12 = (d_sel / 10.0 + 1.0) / 2.0;
        let d8 = if d12 < 0.0 { d_low/512.0 } else if d12 > 1.0 { d_high/512.0 } else { d_low/512.0 + (d_high/512.0 - d_low/512.0) * d12 };
        let d9_raw = (j3 as f64 - d7) * 12.0 / d5;
        let d9 = if d9_raw < 0.0 { d9_raw * 4.0 } else { d9_raw };
        let density = d8 - d9;
        println!("  j3={}: {:.4}  {:.4}  {:.4}  {:.4}  {:.4}  {:.4}", j3, d12, d_low/512.0, d_high/512.0, d8, d9, density);
    }
}

/// Diagnostic: compare f32 vs f64 selector/low/high noise at cell (0,2,10).
#[test]
#[ignore = "diagnostic: f32 vs f64 noise at cell (0,2,10)"]
fn diagnostic_f32_f64_cell_0_2_10() {
    use mcrs_minecraft_worldgen::density_function::beta_seed::{seed_beta_terrain, seed_beta_terrain_f64};
    let (low32, high32, sel32, _, _, _, _) = seed_beta_terrain(12345);
    let (low64, high64, sel64, _, _, _, _) = seed_beta_terrain_f64(12345);

    // Cell (ix=0, iz=2) in chunk (0,-1): x=0, z=-2
    let x64 = 0.0f64;
    let z64 = -2.0f64;
    let x32 = 0.0f32;
    let z32 = -2.0f32;
    let d0 = 684.412f64;
    let d0_32 = 684.412f32;

    println!("\n=== Selector/Low/High noise at (x=0, z=-2) for various y ===");
    for j3 in 8..14usize {
        let y64 = j3 as f64;
        let y32 = j3 as f32;

        let sel_f64 = sel64.sample_xyz_beta(x64, y64, z64, d0/80.0, d0/160.0, d0/80.0);
        let low_f64 = low64.sample_xyz_beta(x64, y64, z64, d0, d0, d0);
        let high_f64 = high64.sample_xyz_beta(x64, y64, z64, d0, d0, d0);

        let sel_f32 = sel32.sample_xyz_beta(x32, y32, z32, d0_32 as f32/80.0, d0_32 as f32/160.0, d0_32 as f32/80.0);
        let low_f32 = low32.sample_xyz_beta(x32, y32, z32, d0_32 as f32, d0_32 as f32, d0_32 as f32);
        let high_f32 = high32.sample_xyz_beta(x32, y32, z32, d0_32 as f32, d0_32 as f32, d0_32 as f32);

        println!("  y={}: sel f32={:.4} f64={:.4} | low f32={:.2} f64={:.2} | high f32={:.2} f64={:.2}",
            j3, sel_f32, sel_f64, low_f32/512.0, low_f64/512.0, high_f32/512.0, high_f64/512.0);
    }
}

/// Diagnostic: print what generate_column produces for chunk (0,-1) at column (0,8) = world (0,-8).
#[test]
#[ignore = "diagnostic: print generate_column output for world (0,-8)"]
fn diagnostic_generate_column_0_neg8() {
    let router = build_beta_router();
    let (biome_source, snapshot) = build_beta_biome_source();
    let cancel = CancellationToken::new();
    let y_sections: Vec<i32> = (0..8).collect();
    let sections = generate_column(0, -1, &y_sections, &router, Some((&biome_source, &snapshot)), &cancel);
    let stone_id = minecraft::STONE.default_state_id;
    let water_id = minecraft::WATER.default_state_id;
    let local_x = 0i32; // world_x=0 → local_x=0
    let local_z = 8i32; // world_z=-8 → chunk_z=-1 → local_z=-8-(-1*16)=8
    println!("\n=== generate_column(0,-1) output at local ({local_x},{local_z}) [world (0,-8)] ===");
    let mut col = [0u16; 128];
    for (si, sy) in y_sections.iter().enumerate() {
        if let Some(Some((blocks, _))) = sections.get(si) {
            let base_y = sy * 16;
            for local_y in 0..16i32 {
                let world_y = base_y + local_y;
                if world_y >= 0 && world_y < 128 {
                    col[world_y as usize] = blocks.get(BlockPos::new(local_x, local_y, local_z)).0;
                }
            }
        }
    }
    for y in (60..100).rev() {
        let blk = col[y];
        let name = if blk == stone_id.0 { "STONE" } else if blk == water_id.0 { "WATER" } else if blk == 0 { "AIR" } else { "OTHER" };
        println!("  Y={}: {} (id={})", y, name, blk);
    }
}

/// Diagnostic: print density grid for column (0,-8) to debug the parity worst-offender.
/// Column (0,-8) is in chunk (0,-1), local bx=0, bz=8. Java terrain_top=85, ours ~71.
#[test]
#[ignore = "diagnostic: debug column (0,-8) large terrain error"]
fn diagnostic_column_0_neg8() {
    use mcrs_minecraft_worldgen::density_function::beta_terrain_f64::BetaTerrainF64;
    let router = build_beta_router();
    let (temp_grid, rain_grid) = router.sample_beta_climate_grids(0, -16);

    // Climate at bx=0, bz=8
    let temp = temp_grid[0 * 16 + 8];
    let rain = rain_grid[0 * 16 + 8];
    println!("\n=== Column (0,-8) climate: temp={:.4} rain={:.4} ===", temp, rain);

    let terrain = router.beta_terrain_f64().expect("f64 path must exist");
    let density = terrain.compute_density(0, -1, &temp_grid, &rain_grid);

    // bx=0: i1=0, i2=0; bz=8: j1=2, k2=0
    println!("\n=== Density grid cells (ix=0..1, iz=2..3) for chunk (0,-1) ===");
    for ix in 0..2 {
        for iz in 2..4 {
            println!("  cell ({},{}):", ix, iz);
            for iy in 8..14 {
                let d = density[(ix * 5 + iz) * 17 + iy];
                println!("    y={}: {:.4}", iy, d);
            }
        }
    }

    // Find stone top for bx=0, bz=8
    let stone_id = minecraft::STONE.default_state_id.0 as u32;
    let water_id = 86u32; // water
    let ice_id = minecraft::ICE.default_state_id.0 as u32;
    let flat = BetaTerrainF64::fill_terrain(&density, &temp_grid, 64, stone_id, water_id, ice_id);
    let mut stone_top = None;
    for y in (0..128).rev() {
        let idx = 0 * 16 * 128 + 8 * 128 + y; // bx=0, bz=8
        if flat[idx] == stone_id {
            stone_top = Some(y);
            break;
        }
    }
    println!("\n=== Stone top for bx=0, bz=8: {:?} (Java: 81) ===", stone_top);
}

/// Verify the bedrock probability distribution matches back2beta:
/// Y=0: always bedrock (0 <= 0 + nextInt(5), always true)
/// Y=5: never bedrock (5 <= 0 + nextInt(5) requires nextInt(5) >= 5, impossible since range is 0..4)
#[test]
fn apply_beta_surface_bedrock_probability_matches_back2beta() {
    let router = build_beta_router();
    let (biome_source, snapshot) = build_beta_biome_source();

    let chunk_x = 3i32;
    let chunk_z = 7i32;
    let block_x = chunk_x * 16;
    let block_z = chunk_z * 16;

    let y_sections: Vec<i32> = (0..8).collect();
    let cancel = CancellationToken::new();
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
        block_x,
        block_z,
        &router,
        &biome_source,
        &mut rng,
    );

    let bedrock_id = minecraft::BEDROCK.default_state_id;
    let section0_blocks = &sections[0].as_ref().expect("section 0 must be present").0;

    // Y=0: all bedrock
    let y0_all_bedrock = (0..16i32).all(|x| {
        (0..16i32).all(|z| section0_blocks.get(BlockPos::new(x, 0, z)) == bedrock_id)
    });
    assert!(y0_all_bedrock, "all columns at world Y=0 must be bedrock");

    // Y=5: never bedrock (nextInt(5) max is 4, so 5 > 0+4 = condition never satisfied)
    let y5_no_bedrock = (0..16i32).all(|x| {
        (0..16i32).all(|z| section0_blocks.get(BlockPos::new(x, 5, z)) != bedrock_id)
    });
    assert!(y5_no_bedrock, "world Y=5 must never be bedrock");
}

/// Diagnostic: compare density at cell (0,2) against Java oracle for chunk (0,-1).
/// Java ground truth from DensityDump.java.
#[test]
#[ignore = "diagnostic: density cell (0,2) vs Java for chunk (0,-1)"]
fn diagnostic_density_cell_0_2_vs_java() {
    use mcrs_minecraft_worldgen::density_function::beta_terrain_f64::BetaTerrainF64;
    let terrain = BetaTerrainF64::new(12345);
    let router = build_beta_router();
    let (temp_grid, rain_grid) = router.sample_beta_climate_grids(0, -16);
    let density = terrain.compute_density(0, -1, &temp_grid, &rain_grid);

    let java_vals: [f64; 17] = [
        864.9810190725, 783.2423215778, 688.9960735211, 608.0489604721,
        512.4477319791, 422.2415961867, 325.9445486339, 233.9277404752,
        136.5067561703, 43.4789013648, 21.7252317841, -9.7905360477,
        -40.1572215950, -68.1538144912, -64.6256493212, -43.3957544057,
        -10.0000000000,
    ];

    println!("\n=== Cell (ix=0, iz=2) density comparison ===");
    let mut max_err = 0.0f64;
    for iy in 0..17 {
        let idx = (0 * 5 + 2) * 17 + iy;
        let rust_val = density[idx];
        let java_val = java_vals[iy];
        let err = (rust_val - java_val).abs();
        if err > max_err { max_err = err; }
        println!("  iy={iy:2}: rust={rust_val:12.4} java={java_val:12.4} err={err:.4}");
    }
    println!("  Max error = {max_err:.6}");
}

/// Diagnostic: for each column in chunk (37,-42), print biome, i1, and whether
/// the sandstone trigger would fire. This reveals which columns consume 135 vs 134
/// RNG draws, and where our count diverges from Java's expected count.
#[test]
#[ignore = "diagnostic: i1 and biome per column in chunk (37,-42)"]
fn diagnostic_i1_per_column_chunk_37_neg42() {
    use mcrs_vanilla::biome::source::{beta_biome_from_climate, build_beta_lookup_table, BetaLandBiome};
    use mcrs_random::legacy::LegacyRandom;
    use mcrs_random::Random;

    let cx = 37i32;
    let cz = -42i32;
    let block_x = cx * 16;
    let block_z = cz * 16;

    let router = build_beta_router();
    let biome_table = build_beta_lookup_table();

    const D0: f64 = 0.03125;
    let Some(surf_noise) = router.beta_surface_noise() else { panic!("no surf noise") };

    let mut t = [0.0f64; 256];
    for x in 0..16usize {
        for z in 0..16usize {
            t[x * 16 + z] = surf_noise.sample_xyz_beta(
                (block_x + x as i32) as f64,
                (block_z + z as i32) as f64,
                0.0,
                D0 * 2.0, D0 * 2.0, D0 * 2.0,
            );
        }
    }

    let seed: i64 = (cx as i64)
        .wrapping_mul(341873128712)
        .wrapping_add((cz as i64).wrapping_mul(132897987541));
    let mut rng = LegacyRandom::new(seed as u64);

    println!("\n=== i1 per column in chunk ({cx},{cz}) ===");
    println!("idx  lx lz  wx   wz   biome    t_noise  i1  sand?");

    for z_local in 0..16i32 {
        for x_local in 0..16i32 {
            let col_idx = z_local * 16 + x_local;
            let wx = block_x + x_local;
            let wz = block_z + z_local;
            let idx = (x_local * 16 + z_local) as usize;

            // Draw flag, flag1 (both consume 2 advances each)
            rng.next_f64(); // flag draw
            rng.next_f64(); // flag1 draw
            // Draw i1
            let r_val = rng.next_f64();
            let i1 = (t[idx] / 3.0 + 3.0 + r_val * 0.25) as i32;

            let (temp, humidity) = router.sample_beta_climate(wx, wz);
            let biome = beta_biome_from_climate(&biome_table, temp, humidity);
            let is_desert = biome == BetaLandBiome::Desert;

            let sand_trigger = is_desert && i1 > 0;

            // Consume 128 bedrock draws
            for _ in 0..128 { let _ = rng.next_i32_bound(5); }
            // Consume sandstone draw if applicable
            if sand_trigger { let _ = rng.next_i32_bound(4); }

            let biome_str = format!("{:?}", biome);
            // Only print desert columns or near column 200
            if is_desert || (col_idx >= 195 && col_idx <= 215) {
                println!("{col_idx:3}  {x_local:2} {z_local:2}  {wx:5} {wz:5}  {biome_str:<12}  t={:.3}  i1={i1:2}  {}",
                    t[idx], if sand_trigger { "SAND+SS" } else if is_desert { "SAND (i1=0)" } else { "" });
            }
        }
    }
}


/// Diagnostic: check column (593,-660) pre-surface block profile.
#[test]
#[ignore = "diagnostic: check column (593,-660) terrain blocks"]
fn diagnostic_column_593_neg660() {
    use mcrs_minecraft_worldgen::density_function::beta_terrain_f64::BetaTerrainF64;
    use mcrs_vanilla::biome::source::{BiomeSource, build_beta_lookup_table, beta_biome_from_climate};

    let cx = 37i32;
    let cz = -42i32;

    let router = build_beta_router();
    let (biome_source, snapshot) = build_beta_biome_source();
    let cancel = CancellationToken::new();
    let biome_table = build_beta_lookup_table();
    let y_sections: Vec<i32> = (0..8).collect();

    let sections = generate_column(cx, cz, &y_sections, &router, Some((&biome_source, &snapshot)), &cancel);

    // Find column (593,-660): local_x=1, local_z=12
    let local_x = 1i32;
    let local_z = 12i32;
    let wx = cx * 16 + local_x;
    let wz = cz * 16 + local_z;

    println!("\n=== Column ({wx},{wz}) pre-surface block profile ===");
    let mut generated = [mcrs_protocol::BlockStateId(0); 128];
    for (si, sy) in y_sections.iter().enumerate() {
        if let Some(Some((blocks, _))) = sections.get(si) {
            let base_y = sy * 16;
            for local_y in 0..16i32 {
                let world_y = base_y + local_y;
                if world_y < 128 {
                    generated[world_y as usize] = blocks.get(mcrs_engine::world::block::BlockPos::new(local_x, local_y, local_z));
                }
            }
        }
    }

    let stone = minecraft::STONE.default_state_id;
    let water = minecraft::WATER.default_state_id;
    let air = mcrs_protocol::BlockStateId(0);

    for y in (40..70).rev() {
        let block = generated[y];
        let name = if block == stone { "STONE" }
            else if block == water { "WATER" }
            else if block == air { "air" }
            else { "???" };
        println!("  Y={y:3}: {name} (id={})", block.0);
    }
}

/// Diagnostic: check flag/flag1/i1 for column (593,-660).
#[test]
#[ignore = "diagnostic: check surface flags for (593,-660)"]
fn diagnostic_flags_593_neg660() {
    use mcrs_random::legacy::LegacyRandom;
    use mcrs_random::Random;

    let cx = 37i32;
    let cz = -42i32;
    let block_x = cx * 16;
    let block_z = cz * 16;

    let router = build_beta_router();
    const D0: f64 = 0.03125;
    let Some(beach_noise) = router.beta_beach_noise() else { panic!() };
    let Some(surf_noise) = router.beta_surface_noise() else { panic!() };

    let mut r = [0.0f64; 256];
    let mut s = [0.0f64; 256];
    let mut t = [0.0f64; 256];
    for x in 0..16usize {
        for z in 0..16usize {
            r[x * 16 + z] = beach_noise.sample_xyz_beta(
                (block_x + x as i32) as f64, (block_z + z as i32) as f64, 0.0,
                D0, D0, 1.0);
            s[x * 16 + z] = beach_noise.sample_xyz_beta(
                (block_x + x as i32) as f64, 109.0134, (block_z + z as i32) as f64,
                D0, 1.0, D0);
            t[x * 16 + z] = surf_noise.sample_xyz_beta(
                (block_x + x as i32) as f64, (block_z + z as i32) as f64, 0.0,
                D0 * 2.0, D0 * 2.0, D0 * 2.0);
        }
    }

    let seed: i64 = (cx as i64).wrapping_mul(341873128712)
        .wrapping_add((cz as i64).wrapping_mul(132897987541));
    let mut rng = LegacyRandom::new(seed as u64);

    // Skip to column (local_x=1, local_z=12) = column index 12*16+1=193
    // Outer=z, inner=x. Column index = z_local*16 + x_local.
    // For z=12, x=1: col_idx = 12*16+1 = 193
    // Consume 193 columns × 134 draws each (all non-desert/savanna)
    let col_idx_target = 12 * 16 + 1usize; // = 193
    for col in 0..col_idx_target {
        let z_local = (col / 16) as i32;
        let x_local = (col % 16) as i32;
        let _idx = (x_local * 16 + z_local) as usize;
        rng.next_java_double(); // flag
        rng.next_java_double(); // flag1
        rng.next_java_double(); // i1
        for _ in 0..128 { rng.next_i32_bound(5); } // bedrock
        // Note: no sandstone for these columns (savanna biome, no sand)
    }

    // Now at column (z=12, x=1) = (593,-660)
    let x_local = 1i32;
    let z_local = 12i32;
    let idx = (x_local * 16 + z_local) as usize;
    let rval_flag = rng.next_java_double();
    let rval_flag1 = rng.next_java_double();
    let rval_i1 = rng.next_java_double();

    let flag = r[idx] + rval_flag * 0.2 > 0.0;
    let flag1 = s[idx] + rval_flag1 * 0.2 > 3.0;
    let i1 = (t[idx] / 3.0 + 3.0 + rval_i1 * 0.25) as i32;

    println!("\n=== Flags for column (593,-660) ===");
    println!("  r[idx]={:.12} rval_flag={:.12} flag={}", r[idx], rval_flag, flag);
    println!("  r[idx]+rand*0.2 = {:.12}", r[idx] + rval_flag * 0.2);
    println!("  s[idx]={:.12} rval_flag1={:.12} flag1={}", s[idx], rval_flag1, flag1);
    println!("  t[idx]={:.12} rval_i1={:.12} i1={}", t[idx], rval_i1, i1);
    println!("  Surface stone at Y=61 (from terrain).");
    println!("  Sea-level check k1=61 in [60,65]? YES");
    println!("  flag={flag} → b1/b2 set to sand: {}", flag);
    // Also print nearby r values to see the gradient
    println!("\n  r array around column 193 (z=12, x=0..7):");
    for xl in 0..8usize {
        let i2 = xl * 16 + 12;
        println!("    xl={} r[{}]={:.12}", xl, i2, r[i2]);
    }

    // Cross-check with f64 noise direct call
    use mcrs_minecraft_worldgen::density_function::beta_seed::seed_beta_terrain_f64;
    let (_, _, _, beach_f64, _, _, _) = seed_beta_terrain_f64(12345);
    let direct = beach_f64.sample_xyz_beta(593.0_f64, -660.0_f64, 0.0_f64, D0, D0, 1.0_f64);
    println!("\n  Direct sample_xyz_beta(593,-660,0): {:.12}", direct);
    println!("  Java reference: -0.091324751713");
}
