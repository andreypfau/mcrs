//! Standalone tool to dump the density function computation graph as a DOT file.
//!
//! Usage:
//!   cargo run --example density_graph -- [--seed SEED] [--pos X:Y:Z] [--assets PATH] [--root ROOT] [--settings NAME] [--output FILE]
//!
//! Defaults:
//!   seed = 0, pos = 0:63:0, assets = ./assets, root = final_density, settings = overworld, output = stdout
//!
//! Example:
//!   cargo run --release --example density_graph -p mcrs_minecraft_worldgen -- --seed 2 --pos 0:60:0 --root final_density > graph.dot
//!   dot -Tsvg graph.dot -o graph.svg
//!
//!   # Generate combined graphs for all noise settings:
//!   cargo run --release --example density_graph -p mcrs_minecraft_worldgen -- --settings all --root all --output graphs/

use bevy_math::IVec3;
use mcrs_minecraft_worldgen::density_function::build_functions;
use mcrs_minecraft_worldgen::density_function::proto::{
    DensityFunctionHolder, NoiseParam, ProtoDensityFunction,
};
use mcrs_minecraft_worldgen::proto::NoiseGeneratorSettings;
use mcrs_protocol::Ident;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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
    // Load noise settings
    let settings_path = assets_path.join(format!(
        "minecraft/worldgen/noise_settings/{}.json",
        settings_name
    ));
    let settings_data = std::fs::read(&settings_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", settings_path.display(), e));
    let settings: NoiseGeneratorSettings = serde_json::from_slice(&settings_data)
        .unwrap_or_else(|e| panic!("Failed to parse noise settings {}: {}", settings_name, e));

    // Load all density function files
    let df_dir = assets_path.join("minecraft/worldgen/density_function");
    let mut df_files = Vec::new();
    walk_json_files(&df_dir, &df_dir, "minecraft", &mut df_files);

    let mut holders: BTreeMap<Ident<String>, DensityFunctionHolder> = BTreeMap::new();
    for (ident, data) in &df_files {
        match serde_json::from_slice::<DensityFunctionHolder>(data) {
            Ok(holder) => {
                holders.insert(ident.clone(), holder);
            }
            Err(e) => {
                eprintln!("Warning: failed to parse {}: {}", ident, e);
            }
        }
    }

    // Resolve references to proto functions
    let mut functions: BTreeMap<Ident<String>, ProtoDensityFunction> = BTreeMap::new();
    let holders_snapshot = holders.clone();
    for (ident, holder) in &holders_snapshot {
        resolve_holder(ident, holder, &holders_snapshot, &mut functions);
    }

    // Load all noise files
    let noise_dir = assets_path.join("minecraft/worldgen/noise");
    let mut noise_files = Vec::new();
    walk_json_files(&noise_dir, &noise_dir, "minecraft", &mut noise_files);

    let mut noises: BTreeMap<Ident<String>, NoiseParam> = BTreeMap::new();
    for (ident, data) in &noise_files {
        match serde_json::from_slice::<NoiseParam>(data) {
            Ok(noise) => {
                noises.insert(ident.clone(), noise);
            }
            Err(e) => {
                eprintln!("Warning: failed to parse noise {}: {}", ident, e);
            }
        }
    }

    eprintln!(
        "Loaded {} density functions, {} noises (settings: {})",
        functions.len(),
        noises.len(),
        settings_name
    );

    (functions, noises, settings)
}

fn list_noise_settings(assets_path: &Path) -> Vec<String> {
    let dir = assets_path.join("minecraft/worldgen/noise_settings");
    let mut names = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                if let Some(stem) = path.file_stem() {
                    names.push(stem.to_string_lossy().into_owned());
                }
            }
        }
    }
    names.sort();
    names
}

fn generate_graph(
    assets_path: &Path,
    settings_name: &str,
    root_name: &str,
    seed: u64,
    pos: IVec3,
    output_path: Option<&Path>,
) {
    let (functions, noises, settings) = load_all(assets_path, settings_name);
    let router = build_functions(&functions, &noises, &settings, seed);

    let dot = if root_name == "all" {
        router.dump_combined_dot_graph(pos)
    } else {
        let all_roots = router.roots();
        let found = all_roots.iter().find(|(name, _)| *name == &*root_name);
        let (name, idx) = match found {
            Some((name, idx)) => (*name, *idx),
            None => {
                eprintln!("Unknown root: {}. Available:", root_name);
                for (name, _) in &all_roots {
                    eprintln!("  {}", name);
                }
                std::process::exit(1);
            }
        };
        router.dump_dot_graph(name, idx, pos)
    };

    match output_path {
        Some(path) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            std::fs::write(path, &dot)
                .unwrap_or_else(|e| panic!("Failed to write {}: {}", path.display(), e));
            eprintln!("Written to {}", path.display());
        }
        None => {
            print!("{}", dot);
        }
    }
}

fn main() {
    let mut seed: u64 = 0;
    let mut pos = IVec3::new(0, 63, 0);
    let mut assets_path = PathBuf::from("./assets");
    let mut root_name = "final_density".to_string();
    let mut settings_name = "overworld".to_string();
    let mut output_path: Option<PathBuf> = None;

    // Simple arg parsing
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--seed" => {
                i += 1;
                seed = args[i].parse().expect("Invalid seed");
            }
            "--pos" => {
                i += 1;
                let parts: Vec<&str> = args[i].split(':').collect();
                if parts.len() != 3 {
                    panic!("Position must be X:Y:Z");
                }
                pos = IVec3::new(
                    parts[0].parse().expect("Invalid X"),
                    parts[1].parse().expect("Invalid Y"),
                    parts[2].parse().expect("Invalid Z"),
                );
            }
            "--assets" => {
                i += 1;
                assets_path = PathBuf::from(&args[i]);
            }
            "--root" => {
                i += 1;
                root_name = args[i].clone();
            }
            "--settings" => {
                i += 1;
                settings_name = args[i].clone();
            }
            "--output" | "-o" => {
                i += 1;
                output_path = Some(PathBuf::from(&args[i]));
            }
            "--help" | "-h" => {
                eprintln!(
                    "Usage: density_graph [--seed SEED] [--pos X:Y:Z] [--assets PATH] [--root ROOT] [--settings NAME] [--output FILE|DIR]"
                );
                eprintln!();
                eprintln!("Roots: barrier, fluid_level_floodedness, fluid_level_spread, lava,");
                eprintln!("       temperature, vegetation, continents, erosion, depth, ridges,");
                eprintln!(
                    "       preliminary_surface_level, final_density, vein_toggle, vein_ridged,"
                );
                eprintln!("       vein_gap, all");
                eprintln!();
                eprintln!(
                    "Settings: overworld, nether, end, caves, amplified, large_biomes, floating_islands, all"
                );
                eprintln!();
                eprintln!(
                    "Defaults: seed=0, pos=0:63:0, assets=./assets, root=final_density, settings=overworld"
                );
                eprintln!();
                eprintln!("When --settings all is used, --output is treated as a directory.");
                std::process::exit(0);
            }
            other => {
                eprintln!("Unknown argument: {}", other);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    eprintln!("Seed: {}, Pos: {}:{}:{}", seed, pos.x, pos.y, pos.z);

    if settings_name == "all" {
        let names = list_noise_settings(&assets_path);
        let out_dir = output_path.as_deref().unwrap_or(Path::new("graphs"));
        for name in &names {
            eprintln!("\n=== {} ===", name);
            let file_name = if root_name == "all" {
                format!("{}_combined.dot", name)
            } else {
                format!("{}_{}.dot", name, root_name)
            };
            let out_file = out_dir.join(&file_name);
            generate_graph(&assets_path, name, &root_name, seed, pos, Some(&out_file));
        }
        eprintln!("\nAll graphs written to {}/", out_dir.display());
    } else {
        generate_graph(
            &assets_path,
            &settings_name,
            &root_name,
            seed,
            pos,
            output_path.as_deref(),
        );
    }
}
