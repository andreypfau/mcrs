//! Wall-clock benchmarks for column generation. Run with:
//!   cargo test -p mcrs_minecraft --release bench_ -- --ignored --nocapture
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use mcrs_core::ResourceLocation;
use mcrs_minecraft_worldgen::density_function::proto::{
    DensityFunctionHolder, NoiseParam, ProtoDensityFunction,
};
use mcrs_minecraft_worldgen::density_function::{NoiseRouter, build_functions};
use mcrs_minecraft_worldgen::proto::NoiseGeneratorSettings;

use crate::world::chunk::CancellationToken;
use crate::world::generate::generate_column;

fn assets_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/minecraft/worldgen")
}

fn walk_json(base: &Path, dir: &Path, out: &mut Vec<(ResourceLocation, String)>) {
    let entries =
        std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    for entry in entries {
        let path = entry.unwrap().path();
        if path.is_dir() {
            walk_json(base, &path, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let rel = path.strip_prefix(base).unwrap().with_extension("");
            let key = format!("minecraft:{}", rel.to_string_lossy().replace('\\', "/"));
            let ident = key
                .parse::<ResourceLocation>()
                .unwrap_or_else(|e| panic!("{key}: {e:?}"));
            let json = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            out.push((ident, json));
        }
    }
}

fn resolve_holder(
    id: &ResourceLocation,
    holder: &DensityFunctionHolder,
    all: &BTreeMap<ResourceLocation, DensityFunctionHolder>,
    out: &mut BTreeMap<ResourceLocation, ProtoDensityFunction>,
) {
    match holder {
        DensityFunctionHolder::Value(v) => {
            out.insert(id.clone(), ProtoDensityFunction::Constant(v.clone()));
        }
        DensityFunctionHolder::Reference(r) => {
            let dep = all
                .get(r)
                .unwrap_or_else(|| panic!("{id} references missing {r}"));
            resolve_holder(id, dep, all, out);
        }
        DensityFunctionHolder::Owned(proto) => {
            out.insert(id.clone(), (**proto).clone());
        }
    }
}

fn load_density_functions() -> BTreeMap<ResourceLocation, ProtoDensityFunction> {
    let dir = assets_root().join("density_function");
    let mut files = Vec::new();
    walk_json(&dir, &dir, &mut files);
    let holders: BTreeMap<ResourceLocation, DensityFunctionHolder> = files
        .into_iter()
        .map(|(id, json)| {
            let holder = serde_json::from_str(&json)
                .unwrap_or_else(|e| panic!("{id}: {e}"));
            (id, holder)
        })
        .collect();
    let mut out = BTreeMap::new();
    for (id, holder) in &holders {
        resolve_holder(id, holder, &holders, &mut out);
    }
    assert_eq!(out.len(), holders.len(), "every density function must resolve");
    out
}

fn load_noises() -> BTreeMap<ResourceLocation, NoiseParam> {
    let dir = assets_root().join("noise");
    let mut files = Vec::new();
    walk_json(&dir, &dir, &mut files);
    files
        .into_iter()
        .map(|(id, json)| {
            let param =
                serde_json::from_str(&json).unwrap_or_else(|e| panic!("{id}: {e}"));
            (id, param)
        })
        .collect()
}

fn build_router(settings_name: &str, seed: u64) -> NoiseRouter {
    let path = assets_root().join(format!("noise_settings/{settings_name}.json"));
    let json = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let settings: NoiseGeneratorSettings =
        serde_json::from_str(&json).unwrap_or_else(|e| panic!("{settings_name}: {e}"));
    let functions = load_density_functions();
    let noises = load_noises();
    println!(
        "[{settings_name}] {} density functions, {} noises loaded",
        functions.len(),
        noises.len()
    );
    let router = build_functions(
        &functions,
        &noises,
        &settings,
        seed,
        super::corpus().default_state("minecraft:stone"),
        super::corpus().default_state("minecraft:water"),
    );
    println!(
        "[{settings_name}] zone A entries={} final_density index={} noise height={} sea level={}",
        router.column_boundary(),
        router.final_density_index(),
        router.noise_height(),
        router.sea_level()
    );
    router
}

fn summarize(label: &str, rep: usize, mut ms: Vec<f64>) {
    let n = ms.len();
    let total: f64 = ms.iter().sum();
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pick = |q: f64| ms[(((n - 1) as f64) * q).round() as usize];
    println!(
        "[{label}] rep {rep}: n={n} total={total:.1}ms mean={:.3} median={:.3} p95={:.3} min={:.3} max={:.3} ms/col  ({:.1} col/s)",
        total / n as f64,
        pick(0.5),
        pick(0.95),
        ms[0],
        ms[n - 1],
        n as f64 * 1000.0 / total,
    );
}

fn bench_columns(label: &str, router: &NoiseRouter, columns: i32, reps: usize) {
    // Full overworld dimension: y -64..320 (24 sections)
    let y_sections: Vec<i32> = (-4..20).collect();
    let cancel = CancellationToken::new();

    for i in 0..8 {
        std::hint::black_box(generate_column(
            1000 + i,
            1000,
            &y_sections,
            router,
            None,
            super::corpus(),
            &cancel,
        ));
    }

    // Phase timing: column cache population only
    let t = Instant::now();
    for i in 0..columns {
        let (cx, cz) = (i % 8, i / 8);
        let mut cache = router.new_column_cache(cx * 16, cz * 16);
        router.populate_columns(&mut cache);
    }
    let populate = t.elapsed();
    println!(
        "[{label}] populate-only phase: {:?} total, {:.3}ms/col",
        populate,
        populate.as_secs_f64() * 1000.0 / columns as f64
    );

    for rep in 0..reps {
        let mut samples = Vec::with_capacity(columns as usize);
        for i in 0..columns {
            let (cx, cz) = (i % 8, i / 8);
            let t = Instant::now();
            let results = generate_column(
                cx,
                cz,
                &y_sections,
                router,
                None,
                super::corpus(),
                &cancel,
            );
            samples.push(t.elapsed().as_secs_f64() * 1000.0);
            std::hint::black_box(&results);
        }
        summarize(label, rep, samples);
    }

    // Content checksum over a few columns (outside the timed loop) so that
    // before/after runs can be compared for bit-identical block output.
    let mut checksum = 0u64;
    for i in 0..4 {
        let results = generate_column(i, -i, &y_sections, router, None, super::corpus(), &cancel);
        for r in results.iter().flatten() {
            let (blocks, _) = r;
            let net = blocks.convert_network();
            checksum = checksum
                .wrapping_mul(31)
                .wrapping_add(net.bits_per_entry as u64);
            for w in &net.packed_data {
                checksum = checksum.wrapping_mul(1099511628211).wrapping_add(*w as u64);
            }
        }
    }
    println!("[{label}] checksum={checksum:#x}");
}

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[test]
#[ignore]
fn bench_beta_columns() {
    let router = build_router("beta", 845);
    bench_columns(
        "beta",
        &router,
        env_usize("MCRS_BENCH_COLS", 64) as i32,
        env_usize("MCRS_BENCH_REPS", 5),
    );
}

#[test]
#[ignore]
fn bench_overworld_columns() {
    let router = build_router("overworld", 845);
    bench_columns(
        "overworld",
        &router,
        env_usize("MCRS_BENCH_COLS", 64) as i32,
        env_usize("MCRS_BENCH_REPS", 5),
    );
}
