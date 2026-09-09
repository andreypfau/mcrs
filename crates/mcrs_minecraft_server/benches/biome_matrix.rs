//! Every overworld biome's surface rules over one fixed patch of terrain.
//!
//! The biome grid is an argument to `apply_material_surface`, so pinning it to a
//! single id runs that biome's rules exactly, over terrain the climate would
//! never have paired it with. That measures the rules, not the landscape — see
//! the `natural` mode, which quantifies how much the difference matters.
//!
//! usage: cargo bench --bench biome_matrix -- [mode] [columns_per_side] [seed] [offset]
//!   mode = matrix   per-biome surface cost over one fixed patch of terrain
//!        = natural  same columns with the real grid vs. the pinned grid

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use mcrs_minecraft_core::RegistrySnapshot;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_server::world::chunk::CancellationToken;
use mcrs_minecraft_server::world::generate::multi_noise_biomes::MultiNoiseBiomeTable;
use mcrs_minecraft_server::world::generate::{
    ColumnBlocks, SurfaceIds, apply_material_surface, fill_column_dense_any,
};
use mcrs_minecraft_world::biome::Biome;
use mcrs_minecraft_world::biome::source::{BiomeSource, MultiNoiseBiomeSource};
use mcrs_minecraft_worldgen::compile::build_router;
use mcrs_minecraft_worldgen::material::{
    MaterialConditionHolder, MaterialInputs, MaterialRuleHolder, MaterialScratch,
};
use mcrs_minecraft_worldgen::router::{NoiseGeneratorSettings, NoiseRouter};

#[path = "../src/world/generate/tests/support.rs"]
mod support;

use support::{
    assets_root, corpus, density_function_registry, load_json_dir, noise_registry, router_blocks,
};

const NETHER: [&str; 5] = [
    "nether_wastes",
    "crimson_forest",
    "warped_forest",
    "soul_sand_valley",
    "basalt_deltas",
];
const END: [&str; 5] = [
    "the_end",
    "end_barrens",
    "end_highlands",
    "end_midlands",
    "small_end_islands",
];

/// Every biome the corpus ships, numbered by sorted file name. The material
/// rules and the biome grid share this numbering, and it must cover the whole
/// corpus rather than one preset: a biome the rules name but the map does not
/// hold would fall back to a substitute id and quietly make those rules
/// unreachable.
fn corpus_biome_ids() -> Vec<String> {
    let dir = assets_root().join("biome");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(|e| {
            let path = e.ok()?.path();
            (path.extension()? == "json").then(|| path.file_stem()?.to_str().map(str::to_owned))?
        })
        .collect();
    names.sort();
    assert!(names.len() <= 256, "the biome grid stores a u8");
    names
}

/// The overworld subset: the `beta_*` biomes belong to the legacy generator and
/// the nether and end sets to their own dimensions.
fn overworld_subset(names: &[String]) -> Vec<(usize, String)> {
    names
        .iter()
        .enumerate()
        .filter(|(_, n)| {
            !n.starts_with("beta_") && !NETHER.contains(&n.as_str()) && !END.contains(&n.as_str())
        })
        .map(|(i, n)| (i, n.clone()))
        .collect()
}

fn material_router(seed: u64, names: &[String]) -> NoiseRouter {
    let path = assets_root().join("noise_settings/overworld.json");
    let settings: NoiseGeneratorSettings =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let rules: BTreeMap<ResourceLocation, MaterialRuleHolder> = load_json_dir("material_rule");
    let conditions: BTreeMap<ResourceLocation, MaterialConditionHolder> =
        load_json_dir("material_condition");
    let index: BTreeMap<&str, u32> = names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i as u32))
        .collect();
    let inputs = MaterialInputs {
        rules: &rules,
        conditions: &conditions,
        block: &|state| {
            corpus()
                .block(state.name.as_str())
                .map(|b| b.default_state_id.into())
        },
        biome: &|id| {
            index
                .get(id.path())
                .copied()
                .or_else(|| panic!("the corpus has no biome {id}"))
        },
    };
    build_router(
        &settings,
        &density_function_registry(),
        &noise_registry(),
        seed,
        router_blocks(corpus()),
        Some(&inputs),
    )
    .expect("the overworld compiles")
}

fn surface_ids(names: &[String]) -> SurfaceIds {
    let id = |name: &str| names.iter().position(|n| n == name).unwrap() as u32;
    SurfaceIds {
        eroded_badlands: id("eroded_badlands"),
        frozen_ocean: id("frozen_ocean"),
        deep_frozen_ocean: id("deep_frozen_ocean"),
        snow_block: corpus().default_state("minecraft:snow_block").into(),
        packed_ice: corpus().default_state("minecraft:packed_ice").into(),
    }
}

/// A registry snapshot over the whole corpus. `build` sorts by location and
/// assigns dense ids, which is the same order `corpus_biome_ids` produces, so a
/// biome's network id here equals the id the material rules were compiled with.
fn biome_registry(names: &[String]) -> (RegistrySnapshot<Biome>, bevy_asset::Assets<Biome>) {
    let mut assets = bevy_asset::Assets::<Biome>::default();
    let pairs: Vec<_> = names
        .iter()
        .map(|name| {
            let handle = assets.add(Biome {
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
            });
            (
                ResourceLocation::parse(&format!("minecraft:{name}")).expect("a biome name"),
                handle.id(),
            )
        })
        .collect();
    let snapshot = RegistrySnapshot::<Biome>::build(pairs, &assets, |_| Ok(Default::default()));
    (snapshot, assets)
}

fn fixed_source(name: &str) -> BiomeSource {
    BiomeSource::Fixed {
        biome: bevy_asset::Handle::default(),
        biome_id: ResourceLocation::parse(&format!("minecraft:{name}")).expect("a biome name"),
    }
}

fn main() {
    let mut args = std::env::args().skip(1).filter(|a| !a.starts_with("--"));
    let mode = args.next().unwrap_or_else(|| "matrix".to_owned());
    let side: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(6);
    let seed: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(845);
    let offsets: Vec<i32> = args
        .next()
        .map(|s| s.split(',').filter_map(|p| p.parse().ok()).collect())
        .unwrap_or_else(|| vec![-128]);

    let names = corpus_biome_ids();
    let router = material_router(seed, &names);
    let ids = surface_ids(&names);
    let (registry, _assets) = biome_registry(&names);
    let y_sections: Vec<i32> = (-4..20).collect();
    let cancel = CancellationToken::new();

    match mode.as_str() {
        "natural" => {
            for offset in &offsets {
                natural(
                    &router,
                    &names,
                    &ids,
                    &registry,
                    &y_sections,
                    &cancel,
                    side,
                    *offset,
                );
            }
        }
        _ => matrix(
            &router,
            &names,
            &ids,
            &registry,
            &y_sections,
            &cancel,
            side,
            &offsets,
        ),
    }
}

/// One pass of the surface stage over every column, with the grid pinned.
#[allow(clippy::too_many_arguments)]
fn run_pinned(
    router: &NoiseRouter,
    ids: &SurfaceIds,
    y_sections: &[i32],
    cancel: &CancellationToken,
    side: i32,
    offset: i32,
    source: &BiomeSource,
    registry: &RegistrySnapshot<Biome>,
    column: &mut ColumnBlocks,
    scratch: &mut MaterialScratch,
) -> Duration {
    let mut total = Duration::ZERO;
    for z in 0..side {
        for x in 0..side {
            let (cx, cz) = (offset + x, offset + z);
            let Some(mut filled) = fill_column_dense_any(
                column,
                cx,
                cz,
                y_sections,
                router,
                Some((source, registry)),
                None,
                cancel,
            ) else {
                continue;
            };
            let grid = filled
                .biome_grid
                .take()
                .expect("a fixed source produces a grid");
            let t = Instant::now();
            apply_material_surface(
                column,
                cx,
                cz,
                &mut filled.tops,
                &grid,
                router,
                ids,
                scratch,
            );
            total += t.elapsed();
        }
    }
    total
}

#[allow(clippy::too_many_arguments)]
fn matrix(
    router: &NoiseRouter,
    names: &[String],
    ids: &SurfaceIds,
    registry: &RegistrySnapshot<Biome>,
    y_sections: &[i32],
    cancel: &CancellationToken,
    side: i32,
    offsets: &[i32],
) {
    let overworld = overworld_subset(names);

    let mut column = ColumnBlocks::new(y_sections);
    let mut scratch = MaterialScratch::default();
    let columns = (side * side) as f64 * offsets.len() as f64;

    // Warm the lazy tables and the thread-locals before anything is timed.
    run_pinned(
        router,
        ids,
        y_sections,
        cancel,
        2,
        offsets[0],
        &fixed_source(&names[0]),
        registry,
        &mut column,
        &mut scratch,
    );

    println!(
        "seed={seed} columns={n} per biome over offsets {offsets:?}\n",
        seed = router.world_seed,
        n = columns as u32,
    );

    let mut rows: Vec<(String, f64, usize)> = Vec::new();

    for (id, name) in &overworld {
        let source = fixed_source(name);
        let mut elapsed = Duration::ZERO;
        for offset in offsets {
            elapsed += run_pinned(
                router,
                ids,
                y_sections,
                cancel,
                side,
                *offset,
                &source,
                registry,
                &mut column,
                &mut scratch,
            );
        }

        rows.push((name.clone(), elapsed.as_secs_f64() * 1e3 / columns, *id));
    }

    rows.sort_by(|a, b| b.1.total_cmp(&a.1));
    println!("{:<28} {:>9}", "biome", "surface");
    for (name, ms, _) in &rows {
        println!("{name:<28} {ms:>7.3}ms");
    }

    let median = {
        let mut v: Vec<f64> = rows.iter().map(|r| r.1).collect();
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };
    println!("\nmedian {median:.3} ms/column");
    for (name, ms, ..) in &rows {
        if *ms > median * 2.0 {
            println!("  {name} is {:.1}x the median", ms / median);
        }
    }
}

/// The pinned grid against the real one, on the same columns: how much of a
/// biome's measured cost is its rules and how much is the terrain the climate
/// actually pairs it with.
#[allow(clippy::too_many_arguments)]
fn natural(
    router: &NoiseRouter,
    names: &[String],
    ids: &SurfaceIds,
    registry: &RegistrySnapshot<Biome>,
    y_sections: &[i32],
    cancel: &CancellationToken,
    side: i32,
    offset: i32,
) {
    let multi = MultiNoiseBiomeSource {
        preset: Some(ResourceLocation::parse("minecraft:overworld").unwrap()),
        biomes: None,
    };
    let table = MultiNoiseBiomeTable::resolve(&multi, |biome| {
        let path = biome.strip_prefix("minecraft:").unwrap_or(biome);
        names.iter().position(|n| n == path).map(|i| i as u8)
    })
    .expect("the overworld preset resolves");
    let natural_source = BiomeSource::MultiNoise(multi.clone());

    let mut column = ColumnBlocks::new(y_sections);
    let mut scratch = MaterialScratch::default();
    // biome -> (columns, natural ms, pinned ms)
    let mut per: BTreeMap<u8, (u32, f64, f64)> = BTreeMap::new();

    for z in 0..side {
        for x in 0..side {
            let (cx, cz) = (offset + x, offset + z);
            let Some(mut filled) = fill_column_dense_any(
                &mut column,
                cx,
                cz,
                y_sections,
                router,
                Some((&natural_source, registry)),
                Some(&table),
                cancel,
            ) else {
                continue;
            };
            let Some(grid) = filled.biome_grid.take() else {
                continue;
            };

            // The biome the column is mostly made of, over the whole grid rather
            // than the surface alone, since that is what the pinned run replaces.
            let mut counts = [0u32; 256];
            for id in &grid.ids {
                counts[*id as usize] += 1;
            }
            let dominant = counts
                .iter()
                .enumerate()
                .max_by_key(|(_, c)| **c)
                .map(|(i, _)| i as u8)
                .unwrap();

            let mut tops = filled.tops;
            let t = Instant::now();
            apply_material_surface(&column, cx, cz, &mut tops, &grid, router, ids, &mut scratch);
            let real = t.elapsed().as_secs_f64() * 1e3;

            let source = fixed_source(&names[dominant as usize]);
            let pinned_grid = fill_column_dense_any(
                &mut column,
                cx,
                cz,
                y_sections,
                router,
                Some((&source, registry)),
                None,
                cancel,
            )
            .and_then(|f| f.biome_grid)
            .expect("a fixed source produces a grid");
            let mut tops = filled.tops;
            let t = Instant::now();
            apply_material_surface(
                &column,
                cx,
                cz,
                &mut tops,
                &pinned_grid,
                router,
                ids,
                &mut scratch,
            );
            let pinned = t.elapsed().as_secs_f64() * 1e3;

            let slot = per.entry(dominant).or_default();
            slot.0 += 1;
            slot.1 += real;
            slot.2 += pinned;
        }
    }

    println!(
        "seed={} columns={} at [{offset}..{})\n",
        router.world_seed,
        side * side,
        offset + side
    );
    println!(
        "{:<28} {:>6} {:>10} {:>10} {:>8}",
        "dominant biome", "cols", "natural", "pinned", "ratio"
    );
    let mut rows: Vec<_> = per.into_iter().collect();
    rows.sort_by_key(|(_, (n, ..))| std::cmp::Reverse(*n));
    for (id, (n, real, pinned)) in rows {
        let (real, pinned) = (real / n as f64, pinned / n as f64);
        println!(
            "{:<28} {n:>6} {real:>8.3}ms {pinned:>8.3}ms {:>7.2}x",
            names[id as usize],
            pinned / real
        );
    }
}
