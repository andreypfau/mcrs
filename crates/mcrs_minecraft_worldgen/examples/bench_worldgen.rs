//! Benchmark for world generation over a view-distance area.
//!
//! Generates a square of (2*R+1)^2 chunk columns centered on the given chunk,
//! where R is the view distance.  This represents the work the server does when
//! a player first joins or teleports.
//!
//! Usage:
//!   cargo run --example bench_worldgen -p mcrs_minecraft_worldgen -- [OPTIONS]
//!   cargo run --release --example bench_worldgen -p mcrs_minecraft_worldgen -- [OPTIONS]
//!
//! Options:
//!   --seed SEED          World seed (default: 0)
//!   --center X:Z         Center chunk coordinates (default: 0:0)
//!   --view-distance N    View distance in chunks (default: 10)
//!   --assets PATH        Assets directory (default: ./assets)
//!   --settings NAME      Noise settings name (default: overworld)

use bevy_math::IVec3;
use mcrs_minecraft_worldgen::density_function::build_functions;
use mcrs_minecraft_worldgen::density_function::proto::{
    DensityFunctionHolder, NoiseParam, ProtoDensityFunction,
};
use mcrs_minecraft_worldgen::proto::NoiseGeneratorSettings;
use mcrs_protocol::Ident;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn walk_json_files(
    base: &Path,
    dir: &Path,
    namespace: &str,
    out: &mut Vec<(Ident<String>, Vec<u8>)>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_json_files(base, &path, namespace, out);
        } else if path.extension().is_some_and(|e| e == "json") {
            let rel = path.strip_prefix(base).unwrap();
            let name = rel.with_extension("").to_string_lossy().replace('\\', "/");
            let ident_str = format!("{}:{}", namespace, name);
            if let Ok(ident) = Ident::new(ident_str) {
                let data = std::fs::read(&path).unwrap();
                out.push((ident.into(), data));
            }
        }
    }
}

fn resolve_holder(
    id: &Ident<String>,
    holder: &DensityFunctionHolder,
    all: &BTreeMap<Ident<String>, DensityFunctionHolder>,
    out: &mut BTreeMap<Ident<String>, ProtoDensityFunction>,
) {
    if out.contains_key(id) {
        return;
    }
    match holder {
        DensityFunctionHolder::Value(v) => {
            out.insert(id.clone(), ProtoDensityFunction::Constant(v.clone()));
        }
        DensityFunctionHolder::Reference(r) => {
            if let Some(dep) = all.get(r) {
                resolve_holder(id, dep, all, out);
            }
        }
        DensityFunctionHolder::Owned(proto) => {
            out.insert(id.clone(), *proto.clone());
        }
    }
}

fn load_all(
    assets_path: &Path,
    settings_name: &str,
) -> (
    BTreeMap<Ident<String>, ProtoDensityFunction>,
    BTreeMap<Ident<String>, NoiseParam>,
    NoiseGeneratorSettings,
) {
    let settings_path = assets_path.join(format!(
        "minecraft/worldgen/noise_settings/{}.json",
        settings_name
    ));
    let settings_data = std::fs::read(&settings_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", settings_path.display(), e));
    let settings: NoiseGeneratorSettings = serde_json::from_slice(&settings_data)
        .unwrap_or_else(|e| panic!("Failed to parse noise settings {}: {}", settings_name, e));

    let df_dir = assets_path.join("minecraft/worldgen/density_function");
    let mut df_files = Vec::new();
    walk_json_files(&df_dir, &df_dir, "minecraft", &mut df_files);

    let mut holders: BTreeMap<Ident<String>, DensityFunctionHolder> = BTreeMap::new();
    for (ident, data) in &df_files {
        if let Ok(holder) = serde_json::from_slice::<DensityFunctionHolder>(data) {
            holders.insert(ident.clone(), holder);
        }
    }

    let mut functions: BTreeMap<Ident<String>, ProtoDensityFunction> = BTreeMap::new();
    let holders_snapshot = holders.clone();
    for (ident, holder) in &holders_snapshot {
        resolve_holder(ident, holder, &holders_snapshot, &mut functions);
    }

    let noise_dir = assets_path.join("minecraft/worldgen/noise");
    let mut noise_files = Vec::new();
    walk_json_files(&noise_dir, &noise_dir, "minecraft", &mut noise_files);

    let mut noises: BTreeMap<Ident<String>, NoiseParam> = BTreeMap::new();
    for (ident, data) in &noise_files {
        if let Ok(noise) = serde_json::from_slice::<NoiseParam>(data) {
            noises.insert(ident.clone(), noise);
        }
    }

    (functions, noises, settings)
}

/// Generate a full chunk column using the NoiseRouter, returning the number of
/// solid blocks placed.
fn generate_column(
    router: &mcrs_minecraft_worldgen::density_function::NoiseRouter,
    section_x: i32,
    section_z: i32,
    y_sections: &[i32],
) -> (u64, u64) {
    let mut interp = router.new_section_interpolator();
    let block_x = section_x * 16;
    let block_z = section_z * 16;

    let mut column_cache = router.new_column_cache(block_x, block_z);
    router.populate_columns(&mut column_cache);

    #[cfg(feature = "surface-skip")]
    let skip_above_y = router.estimate_max_surface_y(&column_cache);
    let h_cell_blocks = interp.h_cell_blocks();
    let v_cell_blocks = interp.v_cell_blocks();
    let h_cells = interp.h_cells();
    let v_cells = interp.v_cells();

    let mut solid_count: u64 = 0;
    #[allow(unused_mut)]
    let mut skipped_sections: u64 = 0;

    for &sy in y_sections {
        #[cfg(feature = "surface-skip")]
        if let Some(max_y) = skip_above_y {
            if sy * 16 >= max_y {
                skipped_sections += 1;
                interp.reset_section_boundary();
                continue;
            }
        }

        let section_block_y = sy * 16;

        interp.fill_plane_cached_reuse(
            0,
            true,
            block_x,
            section_block_y,
            block_z,
            router,
            &mut column_cache,
        );

        for cell_x in 0..h_cells {
            let next_x = block_x + ((cell_x + 1) * h_cell_blocks) as i32;
            interp.fill_plane_cached_reuse(
                cell_x + 1,
                false,
                next_x,
                section_block_y,
                block_z,
                router,
                &mut column_cache,
            );

            for cell_z in 0..h_cells {
                for cell_y in (0..v_cells).rev() {
                    interp.on_sampled_cell_corners(cell_y, cell_z);

                    match interp.corners_uniform_sign() {
                        Some(false) => continue,
                        Some(true) => {
                            solid_count += (h_cell_blocks * v_cell_blocks * h_cell_blocks) as u64;
                            continue;
                        }
                        None => {}
                    }

                    for local_y in (0..v_cell_blocks).rev() {
                        let delta_y = local_y as f32 / v_cell_blocks as f32;
                        interp.interpolate_y(delta_y);

                        for local_x in 0..h_cell_blocks {
                            let delta_x = local_x as f32 / h_cell_blocks as f32;
                            interp.interpolate_x(delta_x);

                            for local_z in 0..h_cell_blocks {
                                let delta_z = local_z as f32 / h_cell_blocks as f32;
                                interp.interpolate_z(delta_z);

                                if interp.result() > 0.0 {
                                    solid_count += 1;
                                }
                            }
                        }
                    }
                }
            }

            interp.swap_buffers();
        }

        interp.end_section();
    }

    (solid_count, skipped_sections)
}

fn fmt_duration(d: Duration) -> String {
    if d.as_secs() >= 1 {
        format!("{:.3}s", d.as_secs_f64())
    } else if d.as_millis() >= 1 {
        format!("{:.3}ms", d.as_secs_f64() * 1e3)
    } else {
        format!("{:.1}us", d.as_secs_f64() * 1e6)
    }
}

fn main() {
    let mut seed: u64 = 0;
    let mut center_x: i32 = 0;
    let mut center_z: i32 = 0;
    let mut assets_path = PathBuf::from("./assets");
    let mut settings_name = "overworld".to_string();
    let mut view_distance: i32 = 10;

    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--seed" => {
                i += 1;
                seed = args[i].parse().expect("Invalid seed");
            }
            "--center" => {
                i += 1;
                let parts: Vec<&str> = args[i].split(':').collect();
                if parts.len() != 2 {
                    panic!("Center must be X:Z");
                }
                center_x = parts[0].parse().expect("Invalid center X");
                center_z = parts[1].parse().expect("Invalid center Z");
            }
            "--view-distance" => {
                i += 1;
                view_distance = args[i].parse().expect("Invalid view distance");
                if view_distance < 1 {
                    panic!("View distance must be >= 1");
                }
            }
            "--assets" => {
                i += 1;
                assets_path = PathBuf::from(&args[i]);
            }
            "--settings" => {
                i += 1;
                settings_name = args[i].clone();
            }
            "--help" | "-h" => {
                eprintln!(
                    "Usage: bench_worldgen [--seed SEED] [--center X:Z] [--view-distance N] [--assets PATH] [--settings NAME]"
                );
                eprintln!();
                eprintln!(
                    "Defaults: seed=0, center=0:0, view-distance=10, assets=./assets, settings=overworld"
                );
                std::process::exit(0);
            }
            other => {
                eprintln!("Unknown argument: {}", other);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    // --- Load assets & build NoiseRouter ---
    eprintln!("Loading assets from {} ...", assets_path.display());
    let t_load = Instant::now();
    let (functions, noises, settings) = load_all(&assets_path, &settings_name);
    let load_elapsed = t_load.elapsed();
    eprintln!(
        "Loaded {} density functions, {} noises in {}",
        functions.len(),
        noises.len(),
        fmt_duration(load_elapsed),
    );

    let min_y = settings.noise.min_y;
    let height = settings.noise.height as i32;
    let y_sections: Vec<i32> = ((min_y / 16)..((min_y + height) / 16)).collect();
    let num_sections = y_sections.len();

    eprintln!(
        "Settings: {}, Y range: {}..{} ({} sections)",
        settings_name,
        min_y,
        min_y + height,
        num_sections,
    );

    let t_build = Instant::now();
    let router = build_functions(&functions, &noises, &settings, seed);
    let build_elapsed = t_build.elapsed();
    eprintln!("Built NoiseRouter in {}", fmt_duration(build_elapsed));
    router.print_zone_stats();

    // --- A/B comparison: lazy vs full evaluation across multiple chunks ---
    {
        let test_chunks: Vec<(i32, i32)> = (-3..=3)
            .flat_map(|dx| (-3..=3).map(move |dz| (center_x + dx, center_z + dz)))
            .collect();
        let mut mismatches = 0u64;
        let mut total_checks = 0u64;
        let mut max_diff: f32 = 0.0;

        for &(cx, cz) in &test_chunks {
            let block_x = cx * 16;
            let block_z = cz * 16;

            // Create two independent column caches
            let mut cache_lazy = router.new_column_cache(block_x, block_z);
            let mut cache_full = router.new_column_cache(block_x, block_z);
            router.populate_columns(&mut cache_lazy);
            router.populate_columns(&mut cache_full);

            for &sy in &y_sections {
                let section_block_y = sy * 16;
                for px in 0..=4usize {
                    let x = block_x + (px * 4) as i32;
                    let lx = (px * 4) as i32;
                    for pz in 0..=4usize {
                        let z = block_z + (pz * 4) as i32;
                        let lz = (pz * 4) as i32;

                        cache_lazy.load_column(lx, lz);
                        cache_full.load_column(lx, lz);

                        for cy in 0..=2usize {
                            let y = section_block_y + (cy * 8) as i32;
                            let pos = IVec3::new(x, y, z);

                            let lazy_val = router.final_density_from_column_cache(pos, &mut cache_lazy);

                            // Full: evaluate all Zone B entries (no lazy skip)
                            for i in router.column_boundary()..=router.final_density_idx() {
                                cache_full.scratch[i] = router.sample_entry(i, &cache_full.scratch, pos);
                            }
                            let full_val = cache_full.scratch[router.final_density_idx()];

                            total_checks += 1;
                            let diff = (lazy_val - full_val).abs();
                            if diff > 1e-6 {
                                mismatches += 1;
                                max_diff = max_diff.max(diff);
                                if mismatches <= 5 {
                                    eprintln!(
                                        "  MISMATCH at ({},{},{}): lazy={:.8}, full={:.8}, diff={:.8e}",
                                        x, y, z, lazy_val, full_val, diff,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        if mismatches > 0 {
            eprintln!(
                "A/B CHECK FAILED: {}/{} mismatches (max diff={:.8e})",
                mismatches, total_checks, max_diff,
            );
        } else {
            eprintln!("A/B check: {}/{} ok", total_checks, total_checks);
        }
    }

    // --- Build chunk list ---
    let side = 2 * view_distance + 1;
    let total_chunks = (side * side) as usize;
    let mut chunks: Vec<(i32, i32)> = Vec::with_capacity(total_chunks);
    for dx in -view_distance..=view_distance {
        for dz in -view_distance..=view_distance {
            chunks.push((center_x + dx, center_z + dz));
        }
    }

    eprintln!(
        "\nGenerating {}x{} = {} chunk columns (view distance {}), center ({}, {}), seed {} ...",
        side, side, total_chunks, view_distance, center_x, center_z, seed,
    );

    // --- Generate all chunks, timing each one ---
    let mut times = Vec::with_capacity(total_chunks);
    let mut total_solid: u64 = 0;
    let mut total_skipped: u64 = 0;

    let t_total = Instant::now();
    for &(cx, cz) in &chunks {
        let t = Instant::now();
        let (solid, skipped) = generate_column(&router, cx, cz, &y_sections);
        times.push(t.elapsed());
        total_solid += solid;
        total_skipped += skipped;
    }
    let wall_time = t_total.elapsed();

    // --- Stats ---
    times.sort();
    let sum: Duration = times.iter().sum();
    let mean = sum / total_chunks as u32;
    let median = times[total_chunks / 2];
    let min = times[0];
    let max = times[total_chunks - 1];
    let p95 = times[((total_chunks as f64 * 0.95) as usize).min(total_chunks - 1)];
    let p99 = times[((total_chunks as f64 * 0.99) as usize).min(total_chunks - 1)];

    eprintln!();
    eprintln!("=== Results ({} chunk columns) ===", total_chunks);
    eprintln!("  Wall time:     {}", fmt_duration(wall_time));
    eprintln!("  Total solid:   {} blocks", total_solid);
    if total_skipped > 0 {
        let total_sections = total_chunks as u64 * num_sections as u64;
        eprintln!(
            "  Skipped:       {}/{} sections ({:.1}%)",
            total_skipped,
            total_sections,
            total_skipped as f64 / total_sections as f64 * 100.0,
        );
    }
    eprintln!();
    eprintln!("  Per chunk column:");
    eprintln!("    Mean:   {}", fmt_duration(mean));
    eprintln!("    Median: {}", fmt_duration(median));
    eprintln!("    Min:    {}", fmt_duration(min));
    eprintln!("    Max:    {}", fmt_duration(max));
    eprintln!("    P95:    {}", fmt_duration(p95));
    eprintln!("    P99:    {}", fmt_duration(p99));
    eprintln!();
    eprintln!(
        "  Throughput: {:.2} chunks columns/sec",
        total_chunks as f64 / wall_time.as_secs_f64(),
    );
    eprintln!(
        "  Per chunk section (mean): {}",
        fmt_duration(mean / num_sections as u32),
    );
}
