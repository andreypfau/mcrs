use std::io::Write;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("web_assets.bin");

    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("wasm32") {
        std::fs::write(&out, []).unwrap();
        return;
    }

    // The browser has no asset folder, so the whole corpus travels inside the
    // binary. Nothing is filtered: a registry added to `mcrs_minecraft_world`
    // or a folder walked by `Pack::load` would otherwise be missing only in
    // the browser, and the corpus deflates to a couple of megabytes.
    let assets = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .ancestors()
        .nth(2)
        .expect("the crate sits two levels below the workspace root")
        .join("assets");
    println!("cargo:rerun-if-changed={}", assets.display());

    let mut blob = Vec::new();
    let mut files = 0usize;
    pack(&assets, &assets, &mut blob, &mut files);

    let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(&blob).unwrap();
    let compressed = encoder.finish().unwrap();
    println!(
        "cargo:warning=baked {files} asset files, {} MB raw, {} MB deflated",
        blob.len() / 1_000_000,
        compressed.len() / 1_000_000
    );
    std::fs::write(&out, compressed).unwrap();
}

fn pack(assets: &Path, path: &Path, blob: &mut Vec<u8>, files: &mut usize) {
    if path.is_dir() {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        let mut children: Vec<PathBuf> = entries.filter_map(|e| Some(e.ok()?.path())).collect();
        children.sort();
        for child in children {
            pack(assets, &child, blob, files);
        }
        return;
    }
    let name = path.strip_prefix(assets).unwrap().to_str().unwrap();
    let bytes = std::fs::read(path).unwrap();
    blob.extend_from_slice(&(name.len() as u32).to_le_bytes());
    blob.extend_from_slice(name.as_bytes());
    blob.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    blob.extend_from_slice(&bytes);
    *files += 1;
}
