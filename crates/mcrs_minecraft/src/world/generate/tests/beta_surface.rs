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

    let chunk_x = 37i32;
    let chunk_z = -42i32;
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
    // Captured verbatim from beta_surface_corpus.json at chunk (37,-42), seed 12345.
    // The divergence is visible starting at z=13: corpus [7,1,7,7,1], Rust [7,7,1,1,1].
    let oracle: &[(i32, [u8; 5])] = &[
        ( 0, [7, 1, 7, 1, 1]),
        ( 1, [7, 7, 1, 7, 7]),
        ( 2, [7, 7, 1, 1, 1]),
        ( 3, [7, 7, 7, 1, 7]),
        ( 4, [7, 7, 1, 7, 1]),
        ( 5, [7, 7, 7, 7, 1]),
        ( 6, [7, 7, 7, 7, 1]),
        ( 7, [7, 7, 7, 7, 1]),
        ( 8, [7, 7, 1, 7, 1]),
        ( 9, [7, 7, 7, 1, 7]),
        (10, [7, 7, 1, 1, 7]),
        (11, [7, 1, 7, 1, 1]),
        (12, [7, 7, 7, 7, 1]),
        (13, [7, 1, 7, 7, 1]),  // ← diverges here without the fix
        (14, [7, 7, 7, 7, 1]),
        (15, [7, 7, 7, 1, 1]),
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
        "\nbeta_surface_bedrock_matches_back2beta_oracle FAILED (chunk 37,-42, x_local=0):\n{}\n\
         Root cause: idx = x_local*16 + z_local reads geographic noise; \
         fix: use idx = z_local*16 + x_local (transposed, matching back2beta r[kk+ll*16]).",
        failures.join("\n"),
    );
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
