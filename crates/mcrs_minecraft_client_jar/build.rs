#![expect(
    clippy::expect_used,
    reason = "a build script fails the build on a descriptor it cannot read"
)]
#![expect(
    clippy::disallowed_methods,
    reason = "a build script reads the variables cargo sets for it"
)]

use std::path::Path;

use serde::Deserialize;

const DESCRIPTOR: &str = "src/release.json";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Artifact {
    url: String,
    sha1: String,
    size: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Release {
    id: String,
    jar: Artifact,
    directory: Artifact,
    json: Artifact,
}

fn artifact(artifact: &Artifact) -> String {
    format!(
        "Artifact {{ url: {:?}, sha1: {:?}, size: {} }}",
        artifact.url, artifact.sha1, artifact.size
    )
}

fn main() {
    println!("cargo::rerun-if-changed={DESCRIPTOR}");
    let text = std::fs::read_to_string(DESCRIPTOR).expect("the release descriptor reads");
    let release: Release =
        serde_json::from_str(&text).expect("the release descriptor is a client jar descriptor");
    let out = std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR");
    let code = format!(
        "Release {{ id: {:?}, jar: {}, directory: {}, json: {}, font_hint: \"\" }}",
        release.id,
        artifact(&release.jar),
        artifact(&release.directory),
        artifact(&release.json),
    );
    std::fs::write(Path::new(&out).join("release.rs"), code).expect("OUT_DIR is writable");
}
