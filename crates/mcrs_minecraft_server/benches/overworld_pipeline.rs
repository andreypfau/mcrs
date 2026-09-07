//! Overworld column pipeline, one stage at a time.
//!
//! usage: cargo bench --bench overworld_pipeline -- [columns_per_side] [seed]

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::{Duration, Instant};

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_server::world::chunk::CancellationToken;
use mcrs_minecraft_server::world::generate::modern_carvers::{
    CarverBiomeTable, ModernCarverBlockIds, apply_modern_carvers,
};
use mcrs_minecraft_server::world::generate::multi_noise_biomes::MultiNoiseBiomeTable;
use mcrs_minecraft_server::world::generate::{
    ColumnBlocks, SurfaceIds, apply_material_surface, fill_column_dense_any, multi_noise_palettes,
};
use mcrs_minecraft_world::biome::overworld_preset::overworld_parameter_list;
use mcrs_minecraft_world::biome::source::MultiNoiseBiomeSource;
use mcrs_minecraft_worldgen::carver::CarverConfig;
use mcrs_minecraft_worldgen::compile::build_router;
use mcrs_minecraft_worldgen::material::{
    MaterialConditionHolder, MaterialInputs, MaterialRuleHolder, MaterialScratch,
};
use mcrs_minecraft_worldgen::program::Workspace;
use mcrs_minecraft_worldgen::router::{NoiseGeneratorSettings, NoiseRouter};
use mcrs_minecraft_worldgen::value_provider::HeightContext;

#[path = "../src/world/generate/tests/support.rs"]
mod support;

use support::{assets_root, corpus, density_function_registry, load_json_dir, noise_registry};

const ABSENT_BIOME: u32 = 250;

fn biome_ids() -> HashMap<String, u32> {
    let mut ids = HashMap::new();
    for (_, biome) in overworld_parameter_list().values() {
        let next = ids.len() as u32;
        ids.entry((*biome).to_owned()).or_insert(next);
    }
    ids
}

fn material_router(seed: u64, ids: &HashMap<String, u32>) -> NoiseRouter {
    let path = assets_root().join("noise_settings/overworld.json");
    let settings: NoiseGeneratorSettings =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let rules: BTreeMap<ResourceLocation, MaterialRuleHolder> = load_json_dir("material_rule");
    let conditions: BTreeMap<ResourceLocation, MaterialConditionHolder> =
        load_json_dir("material_condition");
    let inputs = MaterialInputs {
        rules: &rules,
        conditions: &conditions,
        block: &|state| {
            corpus()
                .block(state.name.as_str())
                .map(|b| b.default_state_id.into())
        },
        biome: &|id| Some(ids.get(id.as_str()).copied().unwrap_or(ABSENT_BIOME)),
    };
    build_router(
        &settings,
        &density_function_registry(),
        &noise_registry(),
        seed,
        corpus().default_state("minecraft:stone").into(),
        corpus().default_state("minecraft:water").into(),
        Some(&inputs),
    )
    .expect("the overworld compiles")
}

fn carvers_of(biome: &str) -> Arc<[CarverConfig]> {
    let path = assets_root().join(format!(
        "biome/{}.json",
        biome.strip_prefix("minecraft:").unwrap_or(biome)
    ));
    let raw: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
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
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap()
        })
        .collect()
}

#[derive(Default, Clone, Copy)]
struct Stages {
    fill: Duration,
    biomes: Duration,
    surface: Duration,
    carve: Duration,
    pack: Duration,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let side: i32 = args.first().and_then(|s| s.parse().ok()).unwrap_or(8);
    let seed: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(845);

    let ids = biome_ids();
    let router = material_router(seed, &ids);
    let table = MultiNoiseBiomeTable::resolve(
        &MultiNoiseBiomeSource {
            preset: Some(ResourceLocation::parse("minecraft:overworld").unwrap()),
            biomes: None,
        },
        |biome| u8::try_from(ids[biome]).ok(),
    )
    .unwrap();
    let carvers = CarverBiomeTable::resolve("minecraft:overworld", carvers_of).unwrap();
    let carver_ids = ModernCarverBlockIds::resolve(corpus(), &router, None);
    let biome = |name: &str| ids.get(name).copied().unwrap_or(ABSENT_BIOME);
    let surface_ids = SurfaceIds {
        eroded_badlands: biome("minecraft:eroded_badlands"),
        frozen_ocean: biome("minecraft:frozen_ocean"),
        deep_frozen_ocean: biome("minecraft:deep_frozen_ocean"),
        snow_block: corpus().default_state("minecraft:snow_block").into(),
        packed_ice: corpus().default_state("minecraft:packed_ice").into(),
    };
    let height = HeightContext {
        min_y: router.noise_min_y(),
        depth: router.noise_height() as i32,
        sea_level: router.sea_level(),
    };
    let y_sections: Vec<i32> = (-4..20).collect();
    let cancel = CancellationToken::new();

    let mut column = ColumnBlocks::new(&y_sections);
    let mut scratch = MaterialScratch::default();
    let mut ws = Workspace::new();
    let mut total = Stages::default();
    let mut sink = 0u64;

    let run = |column: &mut ColumnBlocks,
               scratch: &mut MaterialScratch,
               ws: &mut Workspace,
               x: i32,
               z: i32|
     -> (Stages, u64) {
        let mut s = Stages::default();
        let t = Instant::now();
        let mut filled = fill_column_dense_any(
            column,
            x,
            z,
            &y_sections,
            &router,
            None,
            Some(&table),
            &cancel,
        )
        .unwrap();
        s.fill = t.elapsed();
        let t = Instant::now();
        let (biomes, grid) = multi_noise_palettes(&router, &table, x * 16, z * 16, &y_sections);
        s.biomes = t.elapsed();

        let t = Instant::now();
        apply_material_surface(
            column,
            x,
            z,
            &mut filled.tops,
            grid.as_ref().unwrap(),
            &router,
            &surface_ids,
            scratch,
        );
        s.surface = t.elapsed();

        let t = Instant::now();
        apply_modern_carvers(
            column,
            x,
            z,
            seed as i64,
            &router,
            ws,
            &carvers,
            height,
            &carver_ids,
        );
        s.carve = t.elapsed();

        let t = Instant::now();
        let sections = column.into_sections(&biomes);
        s.pack = t.elapsed();
        (s, sections.len() as u64)
    };

    // Warm up: caches, thread-locals, lazy tables.
    for x in 0..2 {
        run(&mut column, &mut scratch, &mut ws, x, -1);
    }

    let started = Instant::now();
    for z in 0..side {
        for x in 0..side {
            let (s, n) = run(&mut column, &mut scratch, &mut ws, x, z);
            total.fill += s.fill;
            total.biomes += s.biomes;
            total.surface += s.surface;
            total.carve += s.carve;
            total.pack += s.pack;
            sink += n;
        }
    }
    let wall = started.elapsed();
    let n = (side * side) as f64;
    let ms = |d: Duration| d.as_secs_f64() * 1e3 / n;
    let all = total.fill + total.biomes + total.surface + total.carve + total.pack;
    let pct = |d: Duration| 100.0 * d.as_secs_f64() / all.as_secs_f64();
    println!(
        "columns={} sections={sink} wall={:.1} ms",
        side * side,
        wall.as_secs_f64() * 1e3
    );
    println!("per column: {:.3} ms", ms(all));
    println!(
        "  fill    {:.3} ms  {:4.1}%",
        ms(total.fill),
        pct(total.fill)
    );
    println!(
        "  biomes  {:.3} ms  {:4.1}%",
        ms(total.biomes),
        pct(total.biomes)
    );
    println!(
        "  surface {:.3} ms  {:4.1}%",
        ms(total.surface),
        pct(total.surface)
    );
    println!(
        "  carve   {:.3} ms  {:4.1}%",
        ms(total.carve),
        pct(total.carve)
    );
    println!(
        "  pack    {:.3} ms  {:4.1}%",
        ms(total.pack),
        pct(total.pack)
    );
}
