//! Every overworld biome's surface rules over one fixed patch of terrain.
//!
//! The stored biomes are an argument to `apply_material_surface`, so pinning
//! them to a single biome runs that biome's rules exactly, over terrain the
//! climate would never have paired it with. That measures the rules, not the landscape — see
//! the `natural` mode, which quantifies how much the difference matters.
//!
//! usage: cargo bench --bench biome_matrix -- [mode] [columns_per_side] [seed] [offset]
//!   mode = matrix   per-biome surface cost over one fixed patch of terrain
//!        = natural  same columns with the real biomes vs. the pinned biome

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use mcrs_minecraft_biome::parameter_list::{MultiNoiseBiomeSourceParameterList, Preset};
use mcrs_minecraft_biome::source::{BiomeSource, MultiNoiseBiomeSource};
use mcrs_minecraft_core::{ResourceLocation, rl};
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_registry::{Entries, Registry};
use mcrs_minecraft_worldgen_density::router::{NoiseGeneratorSettings, NoiseRouter};
use mcrs_minecraft_worldgen_generator::multi_noise_biomes::MultiNoiseBiomeTable;
use mcrs_minecraft_worldgen_generator::task::CancellationToken;
use mcrs_minecraft_worldgen_generator::{
    ColumnBlocks, SurfaceIds, apply_material_surface, fill_column_dense_any,
};
use mcrs_minecraft_worldgen_surface::compile::{MaterialProgram, build_router_and_material};
use mcrs_minecraft_worldgen_surface::{
    MaterialConditionHolder, MaterialInputs, MaterialRuleHolder, MaterialScratch,
};

#[path = "../src/tests/support.rs"]
mod support;

use mcrs_minecraft_worldgen_testing::{corpus_set_numbered, registry, registry_in, worldgen_dir};
use support::{corpus, router_blocks};

fn parameter_lists() -> (
    Registry<keys::MultiNoiseBiomeSourceParameterList>,
    Entries<keys::MultiNoiseBiomeSourceParameterList, MultiNoiseBiomeSourceParameterList>,
) {
    let names =
        Registry::new(Preset::ALL.map(|preset| ResourceLocation::read(preset.name()).unwrap()))
            .unwrap();
    let lists = Entries::new(
        &names,
        Preset::ALL
            .map(|preset| MultiNoiseBiomeSourceParameterList { preset })
            .to_vec(),
    )
    .unwrap();
    (names, lists)
}

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
    let names: Vec<String> = registry::<serde::de::IgnoredAny>("biome")
        .into_keys()
        .map(|id| id.path().to_owned())
        .collect();
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

fn material_router(seed: u64, names: &[String]) -> (NoiseRouter, MaterialProgram) {
    let settings: NoiseGeneratorSettings = mcrs_minecraft_worldgen_testing::read(
        "noise_settings",
        &rl!("minecraft:overworld").to_arc(),
    );
    let qualified: Vec<String> = names
        .iter()
        .map(|name| format!("minecraft:{name}"))
        .collect();
    let leading: Vec<&str> = qualified.iter().map(String::as_str).collect();
    let set = corpus_set_numbered(&leading);
    let rules: BTreeMap<ResourceLocation, MaterialRuleHolder> = registry_in(&set, "material_rule");
    let conditions: BTreeMap<ResourceLocation, MaterialConditionHolder> =
        registry_in(&set, "material_condition");
    let biome_tags = set.tags().expect("the corpus holds the biome tags");
    let inputs = MaterialInputs {
        rules: &rules,
        conditions: &conditions,
        block: &|state| {
            corpus()
                .block(state.name.as_str())
                .map(|b| b.default_state_id.into())
        },
        biome_tags: &biome_tags,
    };
    build_router_and_material(
        &settings,
        &registry("density_function"),
        &registry("noise"),
        seed,
        router_blocks(corpus()),
        &inputs,
    )
    .expect("the overworld compiles")
}

fn surface_ids(biomes: &Registry<keys::Biome>) -> SurfaceIds {
    let id = |name: &str| {
        biomes
            .require_by_name(&format!("minecraft:{name}"))
            .unwrap()
    };
    SurfaceIds {
        eroded_badlands: id("eroded_badlands"),
        frozen_ocean: id("frozen_ocean"),
        deep_frozen_ocean: id("deep_frozen_ocean"),
        snow_block: corpus().default_state("minecraft:snow_block").into(),
        packed_ice: corpus().default_state("minecraft:packed_ice").into(),
        dirt: corpus().default_state("minecraft:dirt").into(),
    }
}

/// A registry over the whole corpus, numbering `names` in order, which is the
/// order `corpus_biome_ids` produces, so a biome's id here equals the id the
/// material rules were compiled with.
fn biome_registry(names: &[String]) -> Registry<keys::Biome> {
    Registry::new(
        names.iter().map(|name| {
            ResourceLocation::read(&format!("minecraft:{name}")).expect("a biome name")
        }),
    )
    .expect("the corpus names distinct biomes")
}

fn fixed_source(registry: &Registry<keys::Biome>, name: &str) -> BiomeSource {
    BiomeSource::Fixed {
        biome: registry
            .require_by_name(&format!("minecraft:{name}"))
            .expect("a corpus biome"),
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
    let (router, material) = material_router(seed, &names);
    let registry = biome_registry(&names);
    let ids = surface_ids(&registry);
    let y_sections: Vec<i32> = (-4..20).collect();
    let cancel = CancellationToken::new();

    match mode.as_str() {
        "natural" => {
            for offset in &offsets {
                natural(
                    &router,
                    &material,
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
            &material,
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
    material: &MaterialProgram,
    ids: &SurfaceIds,
    y_sections: &[i32],
    cancel: &CancellationToken,
    side: i32,
    offset: i32,
    source: &BiomeSource,
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
                Some(source),
                None,
                None,
                cancel,
            ) else {
                continue;
            };
            let t = Instant::now();
            apply_material_surface(
                column,
                cx,
                cz,
                &mut filled.tops,
                &filled.biomes,
                y_sections[0],
                router,
                material,
                ids,
                scratch,
                None,
            );
            total += t.elapsed();
        }
    }
    total
}

#[allow(clippy::too_many_arguments)]
fn matrix(
    router: &NoiseRouter,
    material: &MaterialProgram,
    names: &[String],
    ids: &SurfaceIds,
    registry: &Registry<keys::Biome>,
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
        material,
        ids,
        y_sections,
        cancel,
        2,
        offsets[0],
        &fixed_source(registry, &names[0]),
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
        let source = fixed_source(registry, name);
        let mut elapsed = Duration::ZERO;
        for offset in offsets {
            elapsed += run_pinned(
                router,
                material,
                ids,
                y_sections,
                cancel,
                side,
                *offset,
                &source,
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
    material: &MaterialProgram,
    names: &[String],
    ids: &SurfaceIds,
    registry: &Registry<keys::Biome>,
    y_sections: &[i32],
    cancel: &CancellationToken,
    side: i32,
    offset: i32,
) {
    let (list_names, lists) = parameter_lists();
    let multi = MultiNoiseBiomeSource {
        preset: Some(list_names.require_by_name("minecraft:overworld").unwrap()),
        biomes: None,
    };
    let table = MultiNoiseBiomeTable::resolve(&multi, registry, &lists)
        .expect("the overworld preset resolves");
    let natural_source = BiomeSource::MultiNoise(multi.clone());

    let mut column = ColumnBlocks::new(y_sections);
    let mut scratch = MaterialScratch::default();
    // biome -> (columns, natural ms, pinned ms)
    let mut per: BTreeMap<u8, (u32, f64, f64)> = BTreeMap::new();

    for z in 0..side {
        for x in 0..side {
            let (cx, cz) = (offset + x, offset + z);
            let Some(filled) = fill_column_dense_any(
                &mut column,
                cx,
                cz,
                y_sections,
                router,
                Some(&natural_source),
                Some(&table),
                None,
                cancel,
            ) else {
                continue;
            };
            // The biome the column is mostly made of, over every stored block
            // rather than the surface alone, since that is what the pinned run
            // replaces.
            let mut counts = [0u32; 256];
            for section in &filled.biomes {
                for y in 0..16 {
                    for z in 0..16 {
                        for x in 0..16 {
                            counts[section.get_cell(x, y, z) as usize] += 1;
                        }
                    }
                }
            }
            let dominant = counts
                .iter()
                .enumerate()
                .max_by_key(|(_, c)| **c)
                .map(|(i, _)| i as u8)
                .unwrap();

            let mut tops = filled.tops;
            let t = Instant::now();
            apply_material_surface(
                &column,
                cx,
                cz,
                &mut tops,
                &filled.biomes,
                y_sections[0],
                router,
                material,
                ids,
                &mut scratch,
                None,
            );
            let real = t.elapsed().as_secs_f64() * 1e3;

            let source = fixed_source(registry, &names[dominant as usize]);
            let pinned_fill = fill_column_dense_any(
                &mut column,
                cx,
                cz,
                y_sections,
                router,
                Some(&source),
                None,
                None,
                cancel,
            )
            .expect("a fixed source fills the column");
            let mut tops = filled.tops;
            let t = Instant::now();
            apply_material_surface(
                &column,
                cx,
                cz,
                &mut tops,
                &pinned_fill.biomes,
                y_sections[0],
                router,
                material,
                ids,
                &mut scratch,
                None,
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
