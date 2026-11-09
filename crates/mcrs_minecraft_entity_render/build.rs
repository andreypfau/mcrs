#![expect(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "a build script has no caller to return an error to"
)]
#![expect(
    clippy::disallowed_methods,
    reason = "a build script reads the variables cargo sets for it"
)]

use std::fmt::Write as _;
use std::path::PathBuf;

/// The folders of the entity pack whose JSON files a client entity reads.
const FOLDERS: [&str; 4] = ["entity", "animations", "geometry", "render_controllers"];

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest
        .ancestors()
        .nth(2)
        .expect("the crate sits two levels below the workspace root")
        .join("assets/mcrs/entity");
    let mut files = Vec::new();
    for folder in FOLDERS {
        let directory = root.join(folder);
        println!("cargo::rerun-if-changed={}", directory.display());
        for entry in std::fs::read_dir(&directory).unwrap() {
            let path = entry.unwrap().path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                let name = path.file_name().unwrap().to_str().unwrap().to_owned();
                files.push((format!("{folder}/{name}"), path));
            }
        }
    }
    files.sort();
    let mut source = format!(
        "/// The files of the entity pack of the repository, by their path below `assets/mcrs/entity`.\n\
         pub const REPOSITORY_PACK: [(&str, &str); {}] = [\n",
        files.len()
    );
    for (name, path) in &files {
        writeln!(
            source,
            "    ({name:?}, include_str!({:?})),",
            path.display().to_string()
        )
        .unwrap();
    }
    source.push_str("];\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("repository_pack.rs");
    std::fs::write(out, source).unwrap();
}
