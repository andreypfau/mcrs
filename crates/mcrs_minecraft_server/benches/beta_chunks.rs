//! Beta 1.7.3 chunk-generation benchmark.
//!
//! usage: cargo bench --bench beta_chunks -- [chunks_per_side] [threads] [seed]
//!
//! Mirrors the server's Beta column pipeline: terrain, surface, caves, ores.

use std::sync::Arc;
use std::time::Instant;

use bevy_asset::Assets;
use mcrs_minecraft_core::RegistrySnapshot;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_server::world::chunk::CancellationToken;
use mcrs_minecraft_server::world::generate::ColumnBlocks;
use mcrs_minecraft_server::world::generate::{
    BetaCaveBlockIds, BetaOreBlockIds, apply_beta_caves, apply_beta_ores, apply_beta_surface,
    fill_column_dense_any,
};
use mcrs_minecraft_world::biome::Biome;
use mcrs_minecraft_world::biome::source::{BiomeSource, build_beta_lookup_table};
use mcrs_minecraft_worldgen::router::NoiseRouter;

#[path = "../src/world/generate/tests/support.rs"]
mod support;

use support::{build_settings_router, corpus};

fn make_beta_biome() -> Biome {
    Biome {
        temperature: 0.5,
        downfall: 0.5,
        has_precipitation: true,
        temperature_modifier: None,
        effects: mcrs_minecraft_world::biome::BiomeEffects {
            water_color: None,
            foliage_color: None,
            grass_color: None,
            grass_color_modifier: None,
            dry_foliage_color: None,
        },
        carvers: Vec::new(),
        features: Vec::new(),
        attributes: Default::default(),
    }
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
    let snapshot = RegistrySnapshot::<Biome>::build(all_pairs, &assets, |_| {
        Ok(mcrs_minecraft_nbt::compound::NbtCompound::new())
    });
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

#[derive(Default, Clone, Copy)]
struct Stages {
    terrain: f64,
    surface: f64,
    caves: f64,
    ores: f64,
    pack: f64,
}

impl Stages {
    fn add(&mut self, other: Stages) {
        self.terrain += other.terrain;
        self.surface += other.surface;
        self.caves += other.caves;
        self.ores += other.ores;
        self.pack += other.pack;
    }
}

/// One chunk through the full Beta pipeline. Returns its wall time in ms and
/// the per-stage split.
fn generate_chunk(
    column: &mut ColumnBlocks,
    y_sections: &[i32],
    chunk_x: i32,
    chunk_z: i32,
    router: &NoiseRouter,
    biome_source: &BiomeSource,
    snapshot: &RegistrySnapshot<Biome>,
    cancel: &CancellationToken,
) -> (f64, Stages) {
    let world_seed = router.world_seed() as i64;
    let mut stages = Stages::default();
    let started = Instant::now();

    let t = Instant::now();
    let biome_palette = fill_column_dense_any(
        column,
        chunk_x,
        chunk_z,
        y_sections,
        router,
        Some((biome_source, snapshot)),
        cancel,
    )
    .expect("the column is not cancelled");
    stages.terrain = t.elapsed().as_secs_f64() * 1000.0;

    let t = Instant::now();
    let seed = (chunk_x as i64)
        .wrapping_mul(341873128712)
        .wrapping_add((chunk_z as i64).wrapping_mul(132897987541));
    let mut rng = LegacyRandom::new(seed as u64);
    apply_beta_surface(
        &column,
        chunk_x * 16,
        chunk_z * 16,
        router,
        biome_source,
        corpus(),
        &mut rng,
    );
    stages.surface = t.elapsed().as_secs_f64() * 1000.0;

    let t = Instant::now();
    let cave_ids = BetaCaveBlockIds::resolve(corpus());
    let cave_config = mcrs_minecraft_decoration::carver::config::BetaCaveCarverConfig {
        air_state: cave_ids.air.into(),
        lava_state: cave_ids.lava.into(),
        stone_state: cave_ids.stone.into(),
        dirt_state: cave_ids.dirt.into(),
        grass_state: cave_ids.grass.into(),
        water_state: cave_ids.water.into(),
        stationary_water_state: cave_ids.stationary_water.into(),
        lava_level: 10,
        range: 8,
        horizontal_radius_multiplier: 1.0,
        vertical_radius_multiplier: 1.0,
    };
    apply_beta_caves(
        &column,
        chunk_x,
        chunk_z,
        world_seed,
        &cave_config,
        &cave_ids,
    );
    stages.caves = t.elapsed().as_secs_f64() * 1000.0;

    let t = Instant::now();
    let ore_ids = BetaOreBlockIds::resolve(corpus());
    apply_beta_ores(&column, chunk_x, chunk_z, world_seed, &ore_ids);
    stages.ores = t.elapsed().as_secs_f64() * 1000.0;

    let t = Instant::now();
    let sections: Vec<_> = column.into_sections(&biome_palette);
    stages.pack = t.elapsed().as_secs_f64() * 1000.0;

    std::hint::black_box(&sections);
    (started.elapsed().as_secs_f64() * 1000.0, stages)
}

fn generate_range(
    seed: u64,
    side: i32,
    from: i32,
    to: i32,
    y_sections: &[i32],
) -> (Vec<f64>, Stages) {
    let router = build_settings_router("beta", seed);
    let (biome_source, snapshot) = build_beta_biome_source();
    let cancel = CancellationToken::new();
    let mut times = Vec::new();
    let mut stages = Stages::default();
    let mut column = ColumnBlocks::new(y_sections);
    for cx in from..to {
        for cz in 0..side {
            let (ms, s) = generate_chunk(
                &mut column,
                y_sections,
                cx,
                cz,
                &router,
                &biome_source,
                &snapshot,
                &cancel,
            );
            times.push(ms);
            stages.add(s);
        }
    }
    (times, stages)
}

/// Non-air blocks in one column, to confirm a section span actually carries terrain.
fn report_content(y_sections: &[i32], seed: u64) {
    let router = build_settings_router("beta", seed);
    let (biome_source, snapshot) = build_beta_biome_source();
    let cancel = CancellationToken::new();
    let mut column = ColumnBlocks::new(y_sections);
    let _ = fill_column_dense_any(
        &mut column,
        0,
        0,
        y_sections,
        &router,
        Some((&biome_source, &snapshot)),
        &cancel,
    )
    .expect("column");
    let seedr = (0i64).wrapping_mul(341873128712);
    let mut rng = LegacyRandom::new(seedr as u64);
    apply_beta_surface(&column, 0, 0, &router, &biome_source, corpus(), &mut rng);
    let world_seed = router.world_seed() as i64;
    let cave_ids = BetaCaveBlockIds::resolve(corpus());
    let cave_config = mcrs_minecraft_decoration::carver::config::BetaCaveCarverConfig {
        air_state: cave_ids.air.into(),
        lava_state: cave_ids.lava.into(),
        stone_state: cave_ids.stone.into(),
        dirt_state: cave_ids.dirt.into(),
        grass_state: cave_ids.grass.into(),
        water_state: cave_ids.water.into(),
        stationary_water_state: cave_ids.stationary_water.into(),
        lava_level: 10,
        range: 8,
        horizontal_radius_multiplier: 1.0,
        vertical_radius_multiplier: 1.0,
    };
    apply_beta_caves(&column, 0, 0, world_seed, &cave_config, &cave_ids);
    let ore_ids = BetaOreBlockIds::resolve(corpus());
    apply_beta_ores(&column, 0, 0, world_seed, &ore_ids);

    let mut non_air = 0u64;
    let mut per_section = Vec::new();
    for section in 0..y_sections.len() {
        let mut count = 0u32;
        for cell in column.section_cells(section) {
            if cell.get() != mcrs_voxel_storage::VoxelId::default() {
                count += 1;
            }
        }
        non_air += count as u64;
        if count > 0 {
            per_section.push((y_sections[section], count));
        }
    }
    println!("  non-empty sections: {per_section:?}");
    println!(
        "sections {:?}: {non_air} non-air blocks of {}",
        y_sections,
        y_sections.len() * 4096
    );
}

fn main() {
    let args: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .collect();
    let side: i32 = args
        .first()
        .and_then(|a| a.parse().ok())
        .filter(|&v| v > 0)
        .unwrap_or(32);
    let threads: i32 = args
        .get(1)
        .and_then(|a| a.parse().ok())
        .filter(|&v| v > 0)
        .unwrap_or(1);
    let seed: u64 = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(12345);

    if args.first().map(String::as_str) == Some("content") {
        let seed: u64 = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(12345);
        report_content(&(0..8).collect::<Vec<_>>(), seed);
        report_content(&(-4..20).collect::<Vec<_>>(), seed);
        return;
    }

    // Section span: Beta's own world is 8 sections; the preset currently runs in
    // the vanilla overworld dimension, which is 24.
    let min_section: i32 = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(0);
    let section_count: i32 = args.get(4).and_then(|a| a.parse().ok()).unwrap_or(8);
    let y_sections: Vec<i32> = (min_section..min_section + section_count).collect();

    // Warm-up: the block corpus, the router tables and first-touch page faults
    // shouldn't land in the measurement.
    let _ = generate_range(seed, 4, -4, 0, &y_sections);

    let wall_start = Instant::now();
    let mut times: Vec<f64> = Vec::new();
    let mut stages = Stages::default();
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                let from = side * t / threads;
                let to = side * (t + 1) / threads;
                let y_sections = y_sections.as_slice();
                scope.spawn(move || generate_range(seed, side, from, to, y_sections))
            })
            .collect();
        for h in handles {
            let (t, s) = h.join().expect("worker");
            times.extend(t);
            stages.add(s);
        }
    });
    let wall = wall_start.elapsed().as_secs_f64() * 1000.0;

    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = times.len();
    let sum: f64 = times.iter().sum();

    println!(
        "mcrs beta worldgen: {n} chunks, {threads} thread(s), seed {seed}, {} sections from {min_section}",
        y_sections.len()
    );
    println!("  wall        {wall:.1} ms");
    println!("  throughput  {:.1} chunks/s", n as f64 / (wall / 1000.0));
    println!(
        "  per chunk   mean {:.3} ms | p50 {:.3} | p95 {:.3} | max {:.3}",
        sum / n as f64,
        times[n / 2],
        times[((n as f64 * 0.95) as usize).min(n - 1)],
        times[n - 1],
    );
    let per = |v: f64| v / n as f64;
    println!(
        "  stages      terrain {:.3} ms | surface {:.3} | caves {:.3} | ores {:.3} | pack {:.3}",
        per(stages.terrain),
        per(stages.surface),
        per(stages.caves),
        per(stages.ores),
        per(stages.pack),
    );
}
