use std::io::Write;
use std::path::{Path, PathBuf};

/// Asset folders the browser build carries inside the binary. The native build
/// reads `assets/` off disk; this list covers every folder the client resolves
/// through the `AssetServer`, which is the registry corpus and the tag tree,
/// not the rendering corpus (textures, models, blockstates).
const WEB_ASSET_FOLDERS: &[&str] = &[
    "mcrs/block_definition",
    "minecraft/banner_pattern",
    "minecraft/cat_sound_variant",
    "minecraft/cat_variant",
    "minecraft/chat_type",
    "minecraft/chicken_sound_variant",
    "minecraft/chicken_variant",
    "minecraft/cow_sound_variant",
    "minecraft/cow_variant",
    "minecraft/damage_type",
    "minecraft/dialog",
    "minecraft/dimension_type",
    "minecraft/enchantment",
    "minecraft/frog_variant",
    "minecraft/instrument",
    "minecraft/jukebox_song",
    "minecraft/painting_variant",
    "minecraft/pig_sound_variant",
    "minecraft/pig_variant",
    "minecraft/tags",
    "minecraft/test_environment",
    "minecraft/test_instance",
    "minecraft/timeline",
    "minecraft/trim_material",
    "minecraft/trim_pattern",
    "minecraft/wolf_sound_variant",
    "minecraft/wolf_variant",
    "minecraft/world_clock",
    "minecraft/worldgen/biome",
    "minecraft/zombie_nautilus_variant",
];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("web_assets.bin");

    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("wasm32") {
        std::fs::write(&out, []).unwrap();
        return;
    }

    let assets = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .ancestors()
        .nth(2)
        .expect("the crate sits two levels below the workspace root")
        .join("assets");

    let mut blob = Vec::new();
    for folder in WEB_ASSET_FOLDERS {
        let root = assets.join(folder);
        println!("cargo:rerun-if-changed={}", root.display());
        pack(&assets, &root, &mut blob);
    }

    let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(&blob).unwrap();
    std::fs::write(&out, encoder.finish().unwrap()).unwrap();
}

fn pack(assets: &Path, dir: &Path, blob: &mut Vec<u8>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| Some(e.ok()?.path())).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            pack(assets, &path, blob);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let name = path.strip_prefix(assets).unwrap().to_str().unwrap();
        let bytes = std::fs::read(&path).unwrap();
        blob.extend_from_slice(&(name.len() as u32).to_le_bytes());
        blob.extend_from_slice(name.as_bytes());
        blob.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        blob.extend_from_slice(&bytes);
    }
}
