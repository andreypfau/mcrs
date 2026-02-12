//! Standalone tool to dump the density function computation graph as a DOT file.
//!
//! Usage:
//!   cargo run --example density_graph -- [--seed SEED] [--pos X:Y:Z] [--assets PATH] [--root ROOT] [--output FILE]
//!
//! Defaults:
//!   seed = 2, pos = 0:60:0, assets = ./assets, root = final_density, output = stdout
//!
//! Example:
//!   cargo run --release --example density_graph -p mcrs_minecraft_worldgen -- --seed 2 --pos 0:60:0 --root final_density > graph.dot
//!   dot -Tsvg graph.dot -o graph.svg

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
) -> (
    BTreeMap<Ident<String>, ProtoDensityFunction>,
    BTreeMap<Ident<String>, NoiseParam>,
    NoiseGeneratorSettings,
) {
    // Load noise settings
    let settings_path = assets_path.join("minecraft/worldgen/noise_settings/overworld.json");
    let settings_data = std::fs::read(&settings_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", settings_path.display(), e));
    let settings: NoiseGeneratorSettings = serde_json::from_slice(&settings_data)
        .unwrap_or_else(|e| panic!("Failed to parse noise settings: {}", e));

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
        "Loaded {} density functions, {} noises",
        functions.len(),
        noises.len()
    );

    (functions, noises, settings)
}

fn main() {
    let mut seed: u64 = 2;
    let mut pos = IVec3::new(0, 60, 0);
    let mut assets_path = PathBuf::from("./assets");
    let mut root_name = "final_density".to_string();
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
            "--output" | "-o" => {
                i += 1;
                output_path = Some(PathBuf::from(&args[i]));
            }
            "--help" | "-h" => {
                eprintln!(
                    "Usage: density_graph [--seed SEED] [--pos X:Y:Z] [--assets PATH] [--root ROOT] [--output FILE]"
                );
                eprintln!();
                eprintln!(
                    "Roots: final_density, temperature, vegetation, continents, erosion, depth, ridges, preliminary_surface_level, all"
                );
                eprintln!();
                eprintln!("Defaults: seed=2, pos=0:60:0, assets=./assets, root=final_density");
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

    let (functions, noises, settings) = load_all(&assets_path);
    let router = build_functions(&functions, &noises, &settings, seed);

    let dot = if root_name == "all" {
        let graphs = router.dump_all_roots_dot_graph(pos);
        graphs
            .into_iter()
            .map(|(_, dot)| dot)
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        let (name, idx) = match root_name.as_str() {
            "final_density" => ("final_density", router.final_density_index()),
            "temperature" => ("temperature", router.temperature_index()),
            "vegetation" => ("vegetation", router.vegetation_index()),
            "continents" => ("continents", router.continents_index()),
            "erosion" => ("erosion", router.erosion_index()),
            "depth" => ("depth", router.depth_index()),
            "ridges" => ("ridges", router.ridges_index()),
            "preliminary_surface_level" => (
                "preliminary_surface_level",
                router.preliminary_surface_level_index(),
            ),
            other => {
                eprintln!("Unknown root: {}", other);
                std::process::exit(1);
            }
        };
        router.dump_dot_graph(name, idx, pos)
    };

    match output_path {
        Some(path) => {
            std::fs::write(&path, &dot)
                .unwrap_or_else(|e| panic!("Failed to write {}: {}", path.display(), e));
            eprintln!("Written to {}", path.display());
        }
        None => {
            print!("{}", dot);
        }
    }
}
