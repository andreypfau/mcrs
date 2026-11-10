use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail};
use mcrs_minecraft_client_jar::{Artifact, RELEASE, get, sha1_hex, verify};
use serde::Deserialize;

#[derive(Deserialize)]
struct Package {
    downloads: Downloads,
}

#[derive(Deserialize)]
struct Downloads {
    server: Download,
}

#[derive(Deserialize)]
struct Download {
    url: String,
    sha1: String,
    size: u64,
}

/// The vanilla server jar of the pinned release, fetched once into `target/vanilla-server` and
/// verified against the package descriptor.
pub fn server_jar(repo: &Path) -> anyhow::Result<PathBuf> {
    let dir = repo.join("target").join("vanilla-server").join(RELEASE.id);
    std::fs::create_dir_all(&dir)?;
    let package = fetched(&dir.join(format!("{}.json", RELEASE.id)), &RELEASE.json)?;
    let package: Package = serde_json::from_slice(&package)?;
    let server = Artifact {
        url: &package.downloads.server.url,
        sha1: &package.downloads.server.sha1,
        size: package.downloads.server.size,
    };
    let jar = dir.join("server.jar");
    fetched(&jar, &server)?;
    Ok(jar)
}

fn fetched(path: &Path, artifact: &Artifact) -> anyhow::Result<Vec<u8>> {
    if let Ok(bytes) = std::fs::read(path)
        && verify(artifact, &bytes)
    {
        return Ok(bytes);
    }
    println!("[mob] fetching {}", artifact.url);
    let bytes = get(artifact.url).map_err(|error| anyhow!("fetching {}: {error}", artifact.url))?;
    if !verify(artifact, &bytes) {
        bail!(
            "{} fails its check: expected SHA-1 {} and {} bytes, got {} and {} bytes",
            artifact.url,
            artifact.sha1,
            artifact.size,
            sha1_hex(&bytes),
            bytes.len()
        );
    }
    std::fs::write(path, &bytes)?;
    Ok(bytes)
}
