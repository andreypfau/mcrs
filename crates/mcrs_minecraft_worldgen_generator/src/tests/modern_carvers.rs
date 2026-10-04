use mcrs_minecraft_worldgen_testing::worldgen_dir;
use std::sync::Arc;

use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::value_provider::HeightContext;
use mcrs_minecraft_worldgen_carver::config::CarverConfig;
use mcrs_minecraft_worldgen_density::aquifer::point_barrier;
use mcrs_minecraft_worldgen_density::program::Workspace;

use super::{build_settings_router, corpus};
use crate::modern_carvers::{
    CarverBiomeTable, ModernCarverBlockIds, apply_modern_carvers, climate_target_at,
};
use crate::{ColumnBlocks, column_fluid_field};

fn overworld_height() -> HeightContext {
    HeightContext {
        min_y: -64,
        depth: 384,
        sea_level: 63,
    }
}

fn y_sections() -> Vec<i32> {
    (-4..20).collect()
}

/// The carver list a biome actually ships, read the way the loader would.
fn carvers_of(biome: &str) -> Arc<[CarverConfig]> {
    let id = mcrs_minecraft_core::ResourceLocation::parse(biome).expect("a biome id");
    let biome: mcrs_minecraft_biome::Biome = mcrs_minecraft_worldgen_testing::read("biome", &id);
    let names: Vec<String> = biome
        .carvers
        .iter()
        .map(|c| c.as_str().to_owned())
        .collect();
    names
        .iter()
        .map(|name| {
            let path = worldgen_dir().join(format!(
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

fn ids() -> ModernCarverBlockIds {
    // No tag registry in a unit test, so the uncarvable set is supplied the
    // way the tag would: bedrock's states.
    ModernCarverBlockIds::for_test(vec![corpus().default_state("minecraft:bedrock").into()])
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

/// A tile of sources is filled in one pass over a strided volume; every
/// source in it has to run the carvers a single-point evaluation resolves to.
#[test]
fn the_tiled_sources_match_point_sampling() {
    let router = build_settings_router("overworld", 12345);
    let table = CarverBiomeTable::resolve("minecraft:overworld", carvers_of).unwrap();
    let mut ws = Workspace::new();
    for (chunk_x, chunk_z) in [(0, 0), (-13, 7)] {
        for source_x in (chunk_x - 8)..=(chunk_x + 8) {
            for source_z in (chunk_z - 8)..=(chunk_z + 8) {
                let point = climate_target_at(&router, &mut ws, source_x * 4, 0, source_z * 4);
                let expected = table.carvers_at_for_test(point);
                let tiled = table.carvers_of_source_for_test(&router, &mut ws, source_x, source_z);
                assert!(
                    std::ptr::eq(expected.as_ptr(), tiled.as_ptr()),
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
fn carving_an_overworld_column_frees_space_and_spares_bedrock() {
    let router = build_settings_router("overworld", 12345);
    let table = CarverBiomeTable::resolve("minecraft:overworld", carvers_of).unwrap();
    let block_ids = ids();
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
            &mut column_fluid_field(&router, chunk_x * 16, 0),
        );

        let mut oracle = column_fluid_field(&router, chunk_x * 16, 0);
        let mut oracle_ws = Workspace::new();
        let mut barrier = point_barrier(&router, &mut oracle_ws);
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
                        let expected =
                            oracle.substance(chunk_x * 16 + x, world_y, z, 0.0, &mut barrier);
                        assert_eq!(
                            Some(state),
                            expected,
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
    let table = CarverBiomeTable::resolve("minecraft:overworld", carvers_of).unwrap();
    let block_ids = ids();
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
            &mut column_fluid_field(&router, 16, 32),
        );
        (0..sections.len())
            .flat_map(|index| column.section_cells(index).iter().map(|c| c.get()))
            .collect::<Vec<_>>()
    };

    assert_eq!(snapshot(12345), snapshot(12345));
    assert_ne!(snapshot(12345), snapshot(999));
}

#[test]
#[ignore = "measurement, not an assertion"]
fn measure_modern_carvers() {
    let router = build_settings_router("overworld", 12345);
    let table = CarverBiomeTable::resolve("minecraft:overworld", carvers_of).unwrap();
    let block_ids = ids();
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
        &mut column_fluid_field(&router, 0, 0),
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
            &mut column_fluid_field(&router, cx * 16, 0),
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
}

struct MaskRun {
    millis: Vec<f64>,
    wall: std::time::Duration,
    builds: usize,
}

fn region_keys(columns: &[(i32, i32)], width: i32) -> Vec<(i32, i32)> {
    columns
        .iter()
        .map(|&(x, z)| (x.div_euclid(width), z.div_euclid(width)))
        .collect()
}

/// The regions a least-recently-used list of `capacity` builds over `keys`,
/// and how many of those builds repeat one.
fn replay_cache(keys: &[(i32, i32)], capacity: usize) -> (usize, usize) {
    let mut kept: Vec<(i32, i32)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let (mut builds, mut rebuilds) = (0, 0);
    for &key in keys {
        if let Some(at) = kept.iter().position(|k| *k == key) {
            kept.remove(at);
        } else {
            builds += 1;
            rebuilds += usize::from(!seen.insert(key));
            if kept.len() == capacity {
                kept.remove(0);
            }
        }
        kept.push(key);
    }
    (builds, rebuilds)
}

fn most_regions_in_a_window(keys: &[(i32, i32)], window: usize) -> usize {
    keys.windows(window)
        .map(|w| w.iter().collect::<std::collections::HashSet<_>>().len())
        .max()
        .unwrap_or(0)
}

#[allow(clippy::too_many_arguments)]
fn run_masks(
    router: &mcrs_minecraft_worldgen_density::router::NoiseRouter,
    height: HeightContext,
    seed: u64,
    table: &CarverBiomeTable,
    columns: &[(i32, i32)],
    threads: usize,
    rest_of_the_column: std::time::Duration,
) -> MaskRun {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let next = AtomicUsize::new(0);
    let start = std::sync::Barrier::new(threads);
    let mut millis = Vec::with_capacity(columns.len());
    let wall = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                let (next, start) = (&next, &start);
                scope.spawn(move || {
                    let mut ws = Workspace::new();
                    let mut mine = Vec::new();
                    start.wait();
                    let began = std::time::Instant::now();
                    loop {
                        let at = next.fetch_add(1, Ordering::Relaxed);
                        let Some(&(x, z)) = columns.get(at) else {
                            break;
                        };
                        let asked = std::time::Instant::now();
                        let slot = crate::modern_carvers::modern_carving_mask(
                            x,
                            z,
                            seed as i64,
                            router,
                            &mut ws,
                            table,
                            height,
                        );
                        mine.push(asked.elapsed().as_secs_f64() * 1000.0);
                        let resting = std::time::Instant::now();
                        while resting.elapsed() < rest_of_the_column {
                            std::hint::spin_loop();
                        }
                        std::hint::black_box(&*slot);
                    }
                    (mine, began.elapsed())
                })
            })
            .collect();
        let mut longest = std::time::Duration::ZERO;
        for worker in workers {
            let (mine, took) = worker.join().unwrap();
            millis.extend(mine);
            longest = longest.max(took);
        }
        longest
    });
    MaskRun {
        millis,
        wall,
        builds: table.region_builds_for_test(),
    }
}

fn summarize(label: &str, run: &MaskRun, columns: usize) {
    let mut sorted = run.millis.clone();
    sorted.sort_by(f64::total_cmp);
    let at = |q: f64| sorted[((sorted.len() as f64 * q) as usize).min(sorted.len() - 1)];
    let mean = sorted.iter().sum::<f64>() / sorted.len() as f64;
    println!(
        "MEASURE {label}: mean {mean:.4} p90 {:.4} p99 {:.4} max {:.3} ms/column, {:.0} columns/s, {} builds",
        at(0.9),
        at(0.99),
        sorted[sorted.len() - 1],
        columns as f64 / run.wall.as_secs_f64(),
        run.builds
    );
}

/// Sweeps the region width: the mask per column over a block of columns, in
/// row order and in order of distance from the middle, with one thread and
/// with the generator's workers sharing one table, and the cache replayed
/// over the same orders.
///
/// ```text
/// cargo test --release -p mcrs_minecraft_worldgen_generator --lib measure_carve_regions -- --ignored --nocapture
/// ```
#[test]
#[ignore = "measurement, not an assertion"]
fn measure_carve_regions() {
    const SIDE: i32 = 64;
    const FIRST: i32 = -35;
    const WORKERS: usize = 8;
    const ROUNDS: usize = 3;
    let rest_of_the_column = std::time::Duration::from_micros(3400);

    let row_order: Vec<(i32, i32)> = (FIRST..FIRST + SIDE)
        .flat_map(|z| (FIRST..FIRST + SIDE).map(move |x| (x, z)))
        .collect();
    let middle = FIRST + SIDE / 2;
    let mut distance_order = row_order.clone();
    distance_order.sort_by_key(|&(x, z)| ((x - middle).pow(2) + (z - middle).pow(2), x, z));
    let columns = row_order.len();

    let widths = [1, 2, 4, 8];
    let mut capacities = Vec::new();
    for width in widths {
        let rows = region_keys(&row_order, width);
        let rings = region_keys(&distance_order, width);
        let distinct = rings.iter().collect::<std::collections::HashSet<_>>().len();
        let mut line = format!("MEASURE cache width {width}: {distinct} regions in the block");
        let mut smallest = [0usize; 2];
        for (slot, keys) in [&rows, &rings].into_iter().enumerate() {
            for capacity in [4usize, 8, 16, 32, 64, 128, 256] {
                let (builds, rebuilds) = replay_cache(keys, capacity);
                if rebuilds == 0 && smallest[slot] == 0 {
                    smallest[slot] = capacity;
                }
                if slot == 1 {
                    line += &format!("; cap {capacity}: {builds} builds {rebuilds} repeated");
                }
            }
        }
        println!("{line}");
        println!(
            "MEASURE cache width {width}: smallest capacity with no repeated build, row order {}, distance order {}; regions in a window of {WORKERS}: {}",
            smallest[0],
            smallest[1],
            most_regions_in_a_window(&rings, WORKERS)
        );
        capacities.push(smallest[0].max(smallest[1]) * 2);
    }

    for seed in [777u64, 845] {
        let router = build_settings_router("overworld", seed);
        let height = crate::stages::extent(&router);
        let column_bytes = CarverBiomeTable::resolve("minecraft:overworld", carvers_of)
            .unwrap()
            .with_region(2, 1)
            .region_bytes_bound(height)
            / 4;
        let build = |width: usize, capacity: usize| {
            let table = CarverBiomeTable::resolve("minecraft:overworld", carvers_of)
                .unwrap()
                .with_region(width as i32, capacity);
            let mut ws = Workspace::new();
            for source_x in FIRST - 16..FIRST + SIDE + 16 {
                for source_z in FIRST - 16..FIRST + SIDE + 16 {
                    table.carvers_of_source_for_test(&router, &mut ws, source_x, source_z);
                }
            }
            table
        };
        run_masks(
            &router,
            height,
            seed,
            &build(1, 0),
            &distance_order,
            1,
            std::time::Duration::ZERO,
        );

        for round in 1..=ROUNDS {
            let alone = build(1, 0);
            let mut ws = Workspace::new();
            let mut millis = Vec::with_capacity(columns);
            let began = std::time::Instant::now();
            for &(x, z) in &distance_order {
                let asked = std::time::Instant::now();
                std::hint::black_box(crate::modern_carvers::carve_sources(
                    || unreachable!("no Beta carver runs in a modern dimension"),
                    x,
                    z,
                    seed as i64,
                    &router,
                    &mut ws,
                    &alone,
                    height,
                ));
                millis.push(asked.elapsed().as_secs_f64() * 1000.0);
            }
            summarize(
                &format!("seed {seed} round {round} per-column sources, distance order, 1 thread"),
                &MaskRun {
                    millis,
                    wall: began.elapsed(),
                    builds: 0,
                },
                columns,
            );
            for (width, capacity) in widths.into_iter().zip(&capacities) {
                let capacity = if width == 1 { 0 } else { *capacity };
                let entry_bytes = (width * width) as usize * column_bytes;
                let label = |what: &str| {
                    format!(
                        "seed {seed} round {round} width {width} capacity {capacity} ({entry_bytes} bytes an entry, {} bound) {what}",
                        capacity * entry_bytes
                    )
                };
                let table = build(width as usize, capacity);
                let run = run_masks(
                    &router,
                    height,
                    seed,
                    &table,
                    &row_order,
                    1,
                    std::time::Duration::ZERO,
                );
                summarize(&label("row order, 1 thread"), &run, columns);
                let table = build(width as usize, capacity);
                let run = run_masks(
                    &router,
                    height,
                    seed,
                    &table,
                    &distance_order,
                    1,
                    std::time::Duration::ZERO,
                );
                summarize(&label("distance order, 1 thread"), &run, columns);
                let table = build(width as usize, capacity);
                let run = run_masks(
                    &router,
                    height,
                    seed,
                    &table,
                    &row_order,
                    WORKERS,
                    std::time::Duration::ZERO,
                );
                summarize(
                    &label(&format!("row order, {WORKERS} threads")),
                    &run,
                    columns,
                );
                let table = build(width as usize, capacity);
                let run = run_masks(
                    &router,
                    height,
                    seed,
                    &table,
                    &distance_order,
                    WORKERS,
                    std::time::Duration::ZERO,
                );
                summarize(
                    &label(&format!("distance order, {WORKERS} threads")),
                    &run,
                    columns,
                );
                let table = build(width as usize, capacity);
                let run = run_masks(
                    &router,
                    height,
                    seed,
                    &table,
                    &distance_order,
                    WORKERS,
                    rest_of_the_column,
                );
                summarize(
                    &label(&format!(
                        "distance order, {WORKERS} threads, 3.4 ms of other work per column"
                    )),
                    &run,
                    columns,
                );
            }
        }
        println!("MEASURE seed {seed}: one column mask holds {column_bytes} bytes");
    }
}

/// The two maps the freeze system reduces the loaded assets to, built the same
/// way from the same files.
fn asset_maps() -> (
    std::collections::HashMap<String, Vec<String>>,
    std::collections::HashMap<String, CarverConfig>,
) {
    let mut carvers_by_biome = std::collections::HashMap::new();
    for (id, biome) in
        mcrs_minecraft_worldgen_testing::registry::<mcrs_minecraft_biome::Biome>("biome")
    {
        let names = biome
            .carvers
            .iter()
            .map(|c| c.as_str().to_owned())
            .collect();
        carvers_by_biome.insert(id.as_str().to_owned(), names);
    }

    let mut config_by_location = std::collections::HashMap::new();
    for entry in std::fs::read_dir(worldgen_dir().join("carver")).unwrap() {
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
    use crate::modern_carvers::resolve_carver_biomes;
    use mcrs_minecraft_biome::climate::{Parameter, ParameterPoint};

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
    let wastes = nether.carvers_at_for_test(mcrs_minecraft_biome::climate::TargetPoint::new(
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
    ));
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
            .carvers_at_for_test(mcrs_minecraft_biome::climate::TargetPoint::new(
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0
            ))
            .is_empty()
    );
}

/// The loader hands the freeze each carver under the folder its registry names;
/// a registry spelled any other way resolves no carver, and every dimension
/// carves nothing with no other symptom.
#[test]
fn a_loaded_carver_asset_names_its_carver() {
    use crate::modern_carvers::CARVER_REGISTRY;
    use mcrs_minecraft_assets::snapshot::rl_from_asset_path;

    let location = rl_from_asset_path(
        std::path::Path::new("minecraft/worldgen/carver/cave.json"),
        CARVER_REGISTRY,
    );
    assert_eq!(
        location.as_ref().map(|location| location.as_str()),
        Some("minecraft:cave")
    );
}

/// Every Beta biome carves with Beta's own carver and nothing else, which is
/// what makes the shared source loop run `MapGenCaves` over a Beta world.
#[test]
fn every_beta_biome_carves_with_the_beta_carver() {
    let (carvers_by_biome, config_by_location) = asset_maps();
    let beta: Vec<_> = carvers_by_biome
        .iter()
        .filter(|(biome, _)| biome.starts_with("minecraft:beta_"))
        .collect();
    assert_eq!(beta.len(), 11);
    for (biome, names) in beta {
        let carvers: Vec<&CarverConfig> =
            names.iter().map(|name| &config_by_location[name]).collect();
        assert_eq!(carvers, [&CarverConfig::BetaCave], "{biome}");
    }
}

/// A Beta source runs the carvers of the land biome the palette gives the
/// source chunk's first cell.
#[test]
fn a_beta_source_runs_the_carvers_of_its_palette_biome() {
    use bevy_math::IVec3;
    use mcrs_minecraft_biome::source::BiomeSource;
    use mcrs_minecraft_worldgen_density::router::{TEMPERATURE, VEGETATION};
    use mcrs_minecraft_worldgen_noise::sample_grid::SampleGrid;

    let router = super::build_beta_router();
    let (source, _) = super::beta_surface::build_beta_biome_source();
    let BiomeSource::Beta { land_biome_ids, .. } = &source else {
        unreachable!("the helper builds a Beta source");
    };
    let cave: serde_json::Value =
        serde_json::from_slice(&std::fs::read(worldgen_dir().join("carver/cave.json")).unwrap())
            .unwrap();
    // A marker per land biome: a cave whose probability is the biome's index.
    let table = CarverBiomeTable::beta(&source, |biome| {
        let index = land_biome_ids
            .iter()
            .position(|id| id.as_str() == biome)
            .unwrap();
        let mut config = cave.clone();
        config["probability"] = serde_json::json!(index as f32 / 16.0);
        Arc::from([serde_json::from_value::<CarverConfig>(config).unwrap()])
    })
    .unwrap();

    let mut ws = Workspace::new();
    let mut seen = std::collections::BTreeSet::new();
    for source_x in (-64..64).step_by(7) {
        for source_z in (-64..64).step_by(5) {
            let volume = SampleGrid::new(
                IVec3::ONE,
                IVec3::new(source_x * 16, 0, source_z * 16),
                IVec3::ONE,
            );
            let mut climate = [0.0f32; 2];
            router.fill_roots(&mut ws, &volume, &[TEMPERATURE, VEGETATION], &mut climate);
            let biome = source.beta_biome_location(climate[0], climate[1]);
            let index = land_biome_ids.iter().position(|id| id == biome).unwrap();
            let carvers = table.carvers_of_source_for_test(&router, &mut ws, source_x, source_z);
            assert_eq!(
                carvers[0].probability(),
                Some(index as f32 / 16.0),
                "source ({source_x}, {source_z})"
            );
            seen.insert(index);
        }
    }
    assert!(seen.len() > 1, "the sample crossed only one biome");
}
