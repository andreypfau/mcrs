use std::sync::Arc;

use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_worldgen::carver::CarverConfig;
use mcrs_minecraft_worldgen::program::Workspace;
use mcrs_minecraft_worldgen::value_provider::HeightContext;
use mcrs_voxel_storage::VoxelId;

use super::{assets_root, build_settings_router, corpus};
use crate::world::generate::ColumnBlocks;
use crate::world::generate::modern_carvers::{
    CarverBiomeTable, ModernCarverBlockIds, apply_modern_carvers, climate_target_at,
    large_feature_seed,
};

fn overworld_height() -> HeightContext {
    HeightContext {
        min_y: -64,
        depth: 384,
    }
}

fn y_sections() -> Vec<i32> {
    (-4..20).collect()
}

/// The carver list a biome actually ships, read the way the loader would.
fn carvers_of(biome: &str) -> Arc<[CarverConfig]> {
    let path = assets_root().join(format!(
        "biome/{}.json",
        biome.strip_prefix("minecraft:").unwrap_or(biome)
    ));
    let raw: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).expect("biome must exist")).unwrap();
    let names: Vec<String> = match raw.get("carvers") {
        Some(serde_json::Value::String(one)) => vec![one.clone()],
        Some(serde_json::Value::Array(many)) => many
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    };
    names
        .iter()
        .map(|name| {
            let path = assets_root().join(format!(
                "carver/{}.json",
                name.strip_prefix("minecraft:").unwrap_or(name)
            ));
            serde_json::from_slice(&std::fs::read(&path).expect("carver must exist")).unwrap()
        })
        .collect()
}

fn stone_column() -> (ColumnBlocks, VoxelId, VoxelId) {
    let sections = y_sections();
    let column = ColumnBlocks::new(&sections);
    let stone: VoxelId = corpus().default_state("minecraft:stone").into();
    let bedrock: VoxelId = corpus().default_state("minecraft:bedrock").into();
    for (index, &section_y) in sections.iter().enumerate() {
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    let world_y = section_y * 16 + y;
                    let state = if world_y <= -60 { bedrock } else { stone };
                    column.set_in_section(index, x, y, z, state);
                }
            }
        }
    }
    (column, stone, bedrock)
}

fn ids(router: &mcrs_minecraft_worldgen::router::NoiseRouter) -> ModernCarverBlockIds {
    // No tag registry in a unit test, so the uncarvable set is supplied the
    // way the tag would: bedrock's states.
    ModernCarverBlockIds::for_test(
        corpus().default_state("minecraft:air").into(),
        router.default_fluid_state(),
        router.sea_level(),
        vec![corpus().default_state("minecraft:bedrock").into()],
    )
}

#[test]
fn large_feature_seed_matches_the_reference_formula() {
    for (seed, cx, cz) in [(12345i64, 0i32, 0i32), (-9, 17, -33), (1, -1, 1)] {
        let mut rng = LegacyRandom::new(seed as u64);
        let x_scale = rng.next_java_long();
        let z_scale = rng.next_java_long();
        let expected = (cx as i64).wrapping_mul(x_scale) ^ (cz as i64).wrapping_mul(z_scale) ^ seed;
        assert_eq!(large_feature_seed(seed, cx, cz), expected);
    }
    // Two different sources must not share a stream.
    assert_ne!(
        large_feature_seed(12345, 0, 0),
        large_feature_seed(12345, 1, 0)
    );
    // The carver index is folded into the seed, so two carvers of one biome
    // draw independently in the same source chunk.
    assert_ne!(
        large_feature_seed(12345, 4, 4),
        large_feature_seed(12346, 4, 4)
    );
}

#[test]
fn the_climate_sampler_stays_inside_the_parameter_range() {
    let router = build_settings_router("overworld", 12345);
    let mut ws = Workspace::new();
    for quart_x in [-64, 0, 37] {
        for quart_z in [-9, 0, 128] {
            let target = climate_target_at(&router, &mut ws, quart_x, 0, quart_z);
            for coord in [
                target.temperature,
                target.humidity,
                target.continentalness,
                target.erosion,
                target.depth,
                target.weirdness,
            ] {
                assert!(coord.abs() <= 40000, "climate coordinate {coord} is wild");
            }
        }
    }
}

/// The 17x17 grid is filled in one pass over a strided volume; every point of
/// it has to be the same climate a single-point evaluation gives.
#[test]
fn the_batched_grid_matches_point_sampling() {
    use crate::world::generate::modern_carvers::climate_targets_for_sources;
    let router = build_settings_router("overworld", 12345);
    let mut ws = Workspace::new();
    let mut targets = Vec::new();
    for (chunk_x, chunk_z) in [(0, 0), (-13, 7)] {
        climate_targets_for_sources(&router, &mut ws, chunk_x, chunk_z, &mut targets);
        assert_eq!(targets.len(), 17 * 17);
        for source_x in (chunk_x - 8)..=(chunk_x + 8) {
            for source_z in (chunk_z - 8)..=(chunk_z + 8) {
                let slot = (source_x - chunk_x + 8) * 17 + (source_z - chunk_z + 8);
                let point = climate_target_at(&router, &mut ws, source_x * 4, 0, source_z * 4);
                assert_eq!(
                    targets[slot as usize], point,
                    "source ({source_x}, {source_z}) of chunk ({chunk_x}, {chunk_z})"
                );
            }
        }
    }
}

#[test]
fn the_overworld_preset_resolves_to_the_shipped_carvers() {
    let table = CarverBiomeTable::resolve("minecraft:overworld", carvers_of)
        .expect("the overworld preset is known");
    // Every overworld biome runs the same three, so any climate finds three.
    let mut ws = Workspace::new();
    let router = build_settings_router("overworld", 12345);
    let target = climate_target_at(&router, &mut ws, 0, 0, 0);
    let carvers = table.carvers_at_for_test(target);
    assert_eq!(carvers.len(), 3);
    assert_eq!(
        carvers
            .iter()
            .filter(|c| matches!(c, CarverConfig::Canyon { .. }))
            .count(),
        1
    );
    assert_eq!(
        carvers
            .iter()
            .filter(|c| matches!(c, CarverConfig::Cave { .. }))
            .count(),
        2
    );
}

#[test]
fn an_unknown_preset_is_not_resolved() {
    assert!(CarverBiomeTable::resolve("minecraft:the_end", carvers_of).is_none());
}

#[test]
fn carving_an_overworld_column_frees_space_and_spares_bedrock() {
    let router = build_settings_router("overworld", 12345);
    let table = CarverBiomeTable::resolve("minecraft:overworld", carvers_of).unwrap();
    let block_ids = ids(&router);
    let sections = y_sections();
    let mut ws = Workspace::new();

    let mut carved_columns = 0;
    for chunk_x in 0..3 {
        let (column, stone, bedrock) = stone_column();
        apply_modern_carvers(
            &column,
            chunk_x,
            0,
            12345,
            &router,
            &mut ws,
            &table,
            overworld_height(),
            &block_ids,
        );

        let mut freed = 0;
        for (index, &section_y) in sections.iter().enumerate() {
            for y in 0..16 {
                for z in 0..16 {
                    for x in 0..16 {
                        let world_y = section_y * 16 + y;
                        let state = column.get(x, world_y, z).unwrap();
                        if world_y <= -60 {
                            assert_eq!(state, bedrock, "bedrock carved at Y {world_y}");
                            continue;
                        }
                        if state == stone {
                            continue;
                        }
                        freed += 1;
                        let expected = if world_y < router.sea_level() {
                            router.default_fluid_state()
                        } else {
                            block_ids.air
                        };
                        assert_eq!(
                            state, expected,
                            "wrong substance at Y {world_y} of section {index}"
                        );
                        assert!(
                            world_y <= -64 + 384 - 1 - 7,
                            "carved into the protected top at Y {world_y}"
                        );
                    }
                }
            }
        }
        if freed > 0 {
            carved_columns += 1;
        }
    }
    assert!(carved_columns > 0, "three chunks carved nothing at all");
}

#[test]
fn carving_is_deterministic() {
    let router = build_settings_router("overworld", 12345);
    let table = CarverBiomeTable::resolve("minecraft:overworld", carvers_of).unwrap();
    let block_ids = ids(&router);
    let sections = y_sections();

    let snapshot = |seed: u64| {
        let router = build_settings_router("overworld", seed);
        let (column, _, _) = stone_column();
        let mut ws = Workspace::new();
        apply_modern_carvers(
            &column,
            1,
            2,
            seed as i64,
            &router,
            &mut ws,
            &table,
            overworld_height(),
            &block_ids,
        );
        (0..sections.len())
            .flat_map(|index| column.section_cells(index).iter().map(|c| c.get()))
            .collect::<Vec<_>>()
    };

    let _ = (&router, &block_ids);
    assert_eq!(snapshot(12345), snapshot(12345));
    assert_ne!(snapshot(12345), snapshot(999));
}

#[test]
#[ignore = "measurement, not an assertion"]
fn measure_modern_carvers() {
    let router = build_settings_router("overworld", 12345);
    let table = CarverBiomeTable::resolve("minecraft:overworld", carvers_of).unwrap();
    let block_ids = ids(&router);
    let mut ws = Workspace::new();
    let (column, _, _) = stone_column();

    // Warm the caches the router keeps.
    apply_modern_carvers(
        &column,
        0,
        0,
        12345,
        &router,
        &mut ws,
        &table,
        overworld_height(),
        &block_ids,
    );

    let columns = 32;
    let started = std::time::Instant::now();
    for cx in 0..columns {
        apply_modern_carvers(
            &column,
            cx,
            0,
            12345,
            &router,
            &mut ws,
            &table,
            overworld_height(),
            &block_ids,
        );
    }
    let each = started.elapsed().as_secs_f64() * 1000.0 / columns as f64;
    println!("MEASURE modern carvers {each:.3} ms/column");

    let mut sink = 0i64;
    let started = std::time::Instant::now();
    for cx in 0..columns {
        for source_x in -8..=8 {
            for source_z in -8..=8 {
                let target =
                    climate_target_at(&router, &mut ws, (cx + source_x) * 4, 0, source_z * 4);
                sink += target.temperature;
            }
        }
    }
    let each = started.elapsed().as_secs_f64() * 1000.0 / columns as f64;
    println!("MEASURE climate point by point {each:.3} ms/column (sink {sink})");

    let mut targets = Vec::new();
    let started = std::time::Instant::now();
    for cx in 0..columns {
        crate::world::generate::modern_carvers::climate_targets_for_sources(
            &router,
            &mut ws,
            cx,
            0,
            &mut targets,
        );
        sink += targets[0].temperature;
    }
    let each = started.elapsed().as_secs_f64() * 1000.0 / columns as f64;
    println!("MEASURE climate batched {each:.3} ms/column (sink {sink})");
}

/// The two maps the freeze system reduces the loaded assets to, built the same
/// way from the same files.
fn asset_maps() -> (
    std::collections::HashMap<String, Vec<String>>,
    std::collections::HashMap<String, CarverConfig>,
) {
    let mut carvers_by_biome = std::collections::HashMap::new();
    for entry in std::fs::read_dir(assets_root().join("biome")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let raw: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let names: Vec<String> = match raw.get("carvers") {
            Some(serde_json::Value::String(one)) => vec![one.clone()],
            Some(serde_json::Value::Array(many)) => many
                .iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect(),
            _ => Vec::new(),
        };
        carvers_by_biome.insert(
            format!("minecraft:{}", path.file_stem().unwrap().to_string_lossy()),
            names,
        );
    }

    let mut config_by_location = std::collections::HashMap::new();
    for entry in std::fs::read_dir(assets_root().join("carver")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        config_by_location.insert(
            format!("minecraft:{}", path.file_stem().unwrap().to_string_lossy()),
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap(),
        );
    }
    (carvers_by_biome, config_by_location)
}

#[test]
fn the_freeze_resolution_builds_the_dimension_tables() {
    use crate::world::generate::modern_carvers::resolve_carver_biomes;
    use mcrs_minecraft_world::biome::climate::{Parameter, ParameterPoint};

    let (carvers_by_biome, config_by_location) = asset_maps();

    let overworld = resolve_carver_biomes(
        Some("minecraft:overworld"),
        None,
        &carvers_by_biome,
        &config_by_location,
    )
    .expect("the overworld preset resolves");
    assert_eq!(overworld.entry_count(), 7594);

    let nether = resolve_carver_biomes(
        Some("minecraft:nether"),
        None,
        &carvers_by_biome,
        &config_by_location,
    )
    .expect("the nether preset resolves");
    assert_eq!(nether.entry_count(), 5);
    let wastes = nether.carvers_at_for_test(
        mcrs_minecraft_world::biome::climate::TargetPoint::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
    );
    assert_eq!(wastes.len(), 1, "the nether wastes run one carver");
    assert!(matches!(wastes[0], CarverConfig::Cave { .. }));

    // A source that lists its biomes instead of naming a preset.
    let point = Parameter::point(0.0);
    let explicit = resolve_carver_biomes(
        None,
        Some(vec![(
            ParameterPoint {
                temperature: point,
                humidity: point,
                continentalness: point,
                erosion: point,
                depth: point,
                weirdness: point,
                offset: 0,
            },
            "minecraft:plains".to_owned(),
        )]),
        &carvers_by_biome,
        &config_by_location,
    )
    .expect("an explicit list resolves");
    assert_eq!(explicit.entry_count(), 1);

    // A preset nothing knows about, and a source with neither form.
    assert!(
        resolve_carver_biomes(
            Some("minecraft:end"),
            None,
            &carvers_by_biome,
            &config_by_location
        )
        .is_none()
    );
    assert!(resolve_carver_biomes(None, None, &carvers_by_biome, &config_by_location).is_none());

    // A biome with no carvers resolves to an empty list rather than to the
    // wrong one.
    let empty = resolve_carver_biomes(
        None,
        Some(vec![(
            ParameterPoint {
                temperature: point,
                humidity: point,
                continentalness: point,
                erosion: point,
                depth: point,
                weirdness: point,
                offset: 0,
            },
            "minecraft:the_end".to_owned(),
        )]),
        &carvers_by_biome,
        &config_by_location,
    )
    .unwrap();
    assert!(
        empty
            .carvers_at_for_test(mcrs_minecraft_world::biome::climate::TargetPoint::new(
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0
            ))
            .is_empty()
    );
}
