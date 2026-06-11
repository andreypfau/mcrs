//! Wall-clock benchmarks for column generation. Run with:
//!   cargo test -p mcrs_minecraft --release bench_ -- --ignored --nocapture
use std::collections::BTreeMap;
use std::time::Instant;

use mcrs_minecraft_worldgen::density_function::{NoiseRouter, build_functions};
use mcrs_minecraft_worldgen::proto::NoiseGeneratorSettings;

use crate::world::chunk::CancellationToken;
use crate::world::generate::generate_column;

fn assets_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/minecraft/worldgen")
}

fn load_density_functions() -> BTreeMap<
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
    let mut map = BTreeMap::new();
    recurse(&assets_root().join("density_function"), "", &mut map);
    map
}

fn load_noises() -> BTreeMap<
    mcrs_protocol::Ident<String>,
    mcrs_minecraft_worldgen::density_function::proto::NoiseParam,
> {
    let mut map = BTreeMap::new();
    let dir = assets_root().join("noise");
    let Ok(entries) = std::fs::read_dir(&dir) else { return map };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let Ok(json) = std::fs::read_to_string(&path) else { continue };
        let Ok(param) = serde_json::from_str::<
            mcrs_minecraft_worldgen::density_function::proto::NoiseParam,
        >(&json) else {
            continue;
        };
        let stem = path.file_stem().unwrap().to_string_lossy();
        if let Ok(ident) = format!("minecraft:{}", stem).parse::<mcrs_protocol::Ident<String>>() {
            map.insert(ident, param);
        }
    }
    map
}

fn build_router(settings_name: &str, seed: u64) -> NoiseRouter {
    let path = assets_root().join(format!("noise_settings/{settings_name}.json"));
    let json = std::fs::read_to_string(&path).expect("noise settings must exist");
    let settings: NoiseGeneratorSettings =
        serde_json::from_str(&json).expect("noise settings must deserialize");
    let functions = load_density_functions();
    let noises = load_noises();
    build_functions(&functions, &noises, &settings, seed, mcrs_protocol::BlockStateId(1), mcrs_protocol::BlockStateId(86))
}

fn bench_columns(label: &str, router: &NoiseRouter, columns: i32) {
    // Full overworld dimension: y -64..320 (24 sections)
    let y_sections: Vec<i32> = (-4..20).collect();
    let cancel = CancellationToken::new();

    // Warm-up
    let _ = generate_column(1000, 1000, &y_sections, router, None, &cancel);

    // Phase timing: column cache population only
    let t = Instant::now();
    for i in 0..columns {
        let (cx, cz) = (i % 8, i / 8);
        let mut cache = router.new_column_cache(cx * 16, cz * 16);
        router.populate_columns(&mut cache);
    }
    let populate = t.elapsed();

    let t = Instant::now();
    for i in 0..columns {
        let (cx, cz) = (i % 8, i / 8);
        let results = generate_column(cx, cz, &y_sections, router, None, &cancel);
        std::hint::black_box(&results);
    }
    let total = t.elapsed();

    // Content checksum over a few columns (outside the timed loop) so that
    // before/after runs can be compared for bit-identical block output.
    let mut checksum = 0u64;
    for i in 0..4 {
        let results = generate_column(i, -i, &y_sections, router, None, &cancel);
        for r in results.iter().flatten() {
            let (blocks, _) = r;
            let net = blocks.convert_network();
            checksum = checksum.wrapping_mul(31).wrapping_add(net.bits_per_entry as u64);
            for w in &net.packed_data {
                checksum = checksum.wrapping_mul(1099511628211).wrapping_add(*w as u64);
            }
        }
    }

    println!(
        "[{label}] {columns} columns: total={:?}  per-column={:.3}ms  columns/s={:.1}  (populate-only phase: {:?}, {:.3}ms/col)  checksum={:#x}",
        total,
        total.as_secs_f64() * 1000.0 / columns as f64,
        columns as f64 / total.as_secs_f64(),
        populate,
        populate.as_secs_f64() * 1000.0 / columns as f64,
        checksum,
    );
}

#[test]
#[ignore]
fn bench_beta_columns() {
    let router = build_router("beta", 845);
    let columns = std::env::var("MCRS_BENCH_COLS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(64);
    bench_columns("beta", &router, columns);
}

#[test]
#[ignore]
fn bench_overworld_columns() {
    let router = build_router("overworld", 845);
    bench_columns("overworld", &router, 64);
}
