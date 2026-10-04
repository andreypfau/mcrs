use std::fs;
use std::path::Path;

use mcrs_minecraft_client_jar::{Artifact, FontHint, Release, directory_digest, sha1_hex, verify};
use mcrs_minecraft_core::Version;
use serde::{Deserialize, Serialize};

pub const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Deserialize)]
struct Manifest {
    versions: Vec<Listing>,
}

#[derive(Debug, Deserialize)]
pub struct Listing {
    pub id: String,
    pub url: String,
    pub sha1: String,
}

#[derive(Deserialize)]
struct Package {
    downloads: Downloads,
}

#[derive(Deserialize)]
struct Downloads {
    client: Download,
}

#[derive(Debug, Deserialize)]
pub struct Download {
    pub url: String,
    pub sha1: String,
    pub size: u64,
}

pub fn resolve(manifest: &[u8], id: &str) -> Result<Listing, String> {
    let manifest: Manifest = serde_json::from_slice(manifest)
        .map_err(|error| format!("{MANIFEST_URL} is not a version manifest: {error}"))?;
    manifest
        .versions
        .into_iter()
        .find(|listing| listing.id == id)
        .ok_or_else(|| format!("{id} is not a version of {MANIFEST_URL}"))
}

pub fn client(package: &[u8]) -> Result<Download, String> {
    let package: Package = serde_json::from_slice(package)
        .map_err(|error| format!("the package descriptor has no client jar: {error}"))?;
    Ok(package.downloads.client)
}

pub fn check(url: &str, sha1: &str, size: Option<u64>, bytes: &[u8]) -> Result<(), String> {
    let matches = match size {
        Some(size) => verify(&Artifact { url, sha1, size }, bytes),
        None => sha1_hex(bytes) == sha1,
    };
    if matches {
        Ok(())
    } else {
        Err(format!("{url} fails its SHA-1 or size"))
    }
}

/// The verified client jar of `id` in the launcher directory `dir`, fetched with `fetch` when it
/// is not already there.
pub fn jar(
    dir: &Path,
    id: &str,
    download: &Download,
    fetch: impl FnOnce(&str) -> Result<Vec<u8>, String>,
) -> Result<Vec<u8>, String> {
    let path = dir.join("versions").join(id).join(format!("{id}.jar"));
    let verified = |bytes: &[u8]| check(&download.url, &download.sha1, Some(download.size), bytes);
    if let Some(held) = fs::read(&path).ok().filter(|held| verified(held).is_ok()) {
        return Ok(held);
    }
    let bytes = fetch(&download.url)?;
    verified(&bytes)?;
    let failed = |path: &Path, error: std::io::Error| format!("{}: {error}", path.display());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| failed(parent, error))?;
    }
    let mut part = path.clone().into_os_string();
    part.push(".part");
    let part = std::path::PathBuf::from(part);
    fs::write(&part, &bytes)
        .and_then(|()| fs::rename(&part, &path))
        .map_err(|error| failed(&path, error))?;
    Ok(bytes)
}

pub fn check_version(version: &[u8], id: &str) -> Result<(), String> {
    let version: Version = serde_json::from_slice(version)
        .map_err(|error| format!("the version.json of the jar is not a version: {error}"))?;
    if version.id == id {
        Ok(())
    } else {
        Err(format!(
            "the jar states version {} but {id} was requested",
            version.id
        ))
    }
}

pub fn pretty(value: &impl Serialize) -> Result<String, String> {
    let mut text = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    text.push('\n');
    Ok(text)
}

pub fn descriptor(
    download: &Download,
    jar: &[u8],
    listing: &Listing,
    package: &[u8],
) -> Result<String, String> {
    let (directory_sha1, directory_size) = directory_digest(jar)?;
    pretty(&Release {
        id: &listing.id,
        jar: Artifact {
            url: &download.url,
            sha1: &download.sha1,
            size: download.size,
        },
        directory: Artifact {
            url: &download.url,
            sha1: &directory_sha1,
            size: directory_size,
        },
        json: Artifact {
            url: &listing.url,
            sha1: &listing.sha1,
            size: package.len() as u64,
        },
        font_hint: "",
    })
}

pub fn font_hint(jar: &[u8]) -> Result<String, String> {
    pretty(&FontHint::of(jar, &sha1_hex(jar))?)
}

pub fn refuse_dirty(status: &str, allow_dirty: bool) -> Result<(), String> {
    if allow_dirty || status.trim().is_empty() {
        return Ok(());
    }
    Err(format!(
        "uncommitted changes in the files this tool writes; commit or discard them, or pass --allow-dirty:\n{status}"
    ))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::fs;

    use mcrs_minecraft_client_jar::{FontHint, Release, sha1_hex};

    use super::*;
    use crate::testing::{scratch, zipped};

    const MANIFEST: &str = r#"{
        "latest": {"release": "alpha", "snapshot": "beta-one"},
        "versions": [
            {"id": "first", "type": "release", "url": "https://example.test/first.json", "sha1": "aa11", "releaseTime": "2026-01-01T00:00:00+00:00"},
            {"id": "second", "type": "snapshot", "url": "https://example.test/second.json", "sha1": "bb22"}
        ]
    }"#;

    const PACKAGE: &str = r#"{
        "id": "second",
        "downloads": {
            "client": {"url": "https://example.test/client.jar", "sha1": "cc33", "size": 17},
            "server": {"url": "https://example.test/server.jar", "sha1": "dd44", "size": 3}
        }
    }"#;

    const SAMPLE_VERSION: &str = r#"{
        "id": "second",
        "name": "Second",
        "world_version": 1,
        "series_id": "side",
        "protocol_version": 2,
        "pack_version": {"resource_major": 3, "resource_minor": 4, "data_major": 5, "data_minor": 6},
        "build_time": "2026-01-01T00:00:00+00:00",
        "java_component": "runtime",
        "java_version": 7,
        "stable": false,
        "use_editor": false
    }"#;

    fn sample_jar() -> Vec<u8> {
        zipped(&[
            ("version.json", SAMPLE_VERSION.as_bytes()),
            ("assets/minecraft/font/default.json", br#"{"providers":[]}"#),
            ("data/minecraft/tags/a.json", b"{}"),
        ])
    }

    fn download_of(bytes: &[u8]) -> Download {
        Download {
            url: "https://example.test/client.jar".to_owned(),
            sha1: sha1_hex(bytes),
            size: bytes.len() as u64,
        }
    }

    #[test]
    fn a_listed_id_resolves_to_its_package_descriptor() {
        let listing = resolve(MANIFEST.as_bytes(), "second").unwrap();

        assert_eq!(listing.id, "second");
        assert_eq!(listing.url, "https://example.test/second.json");
        assert_eq!(listing.sha1, "bb22");
    }

    #[test]
    fn an_id_the_manifest_does_not_list_is_an_error_naming_it() {
        let error = resolve(MANIFEST.as_bytes(), "../third").unwrap_err();

        assert!(error.contains("../third"), "{error}");
    }

    #[test]
    fn a_package_descriptor_yields_the_client_jar() {
        let download = client(PACKAGE.as_bytes()).unwrap();

        assert_eq!(download.url, "https://example.test/client.jar");
        assert_eq!(download.sha1, "cc33");
        assert_eq!(download.size, 17);
    }

    #[test]
    fn bytes_that_do_not_match_are_rejected_with_the_url() {
        let bytes = b"client";
        let sha1 = sha1_hex(bytes);

        assert!(check("https://example.test/a", &sha1, Some(6), bytes).is_ok());
        assert!(check("https://example.test/a", &sha1, None, bytes).is_ok());
        for (sha1, size) in [(sha1.as_str(), Some(7)), ("00", Some(6)), ("00", None)] {
            let error = check("https://example.test/a", sha1, size, bytes).unwrap_err();
            assert!(error.contains("https://example.test/a"), "{error}");
        }
    }

    #[test]
    fn a_fetched_jar_is_stored_through_a_part_file_and_then_reused() {
        let dir = scratch("store");
        let bytes = b"a client jar";
        let download = download_of(bytes);
        let fetches = Cell::new(0);

        let first = jar(&dir, "second", &download, |_| {
            fetches.set(fetches.get() + 1);
            Ok(bytes.to_vec())
        })
        .unwrap();
        let again = jar(&dir, "second", &download, |_| {
            Err("fetched again".to_owned())
        })
        .unwrap();

        assert_eq!(first, bytes);
        assert_eq!(again, bytes);
        assert_eq!(fetches.get(), 1);
        let stored = dir.join("versions/second/second.jar");
        assert_eq!(fs::read(&stored).unwrap(), bytes);
        assert!(!dir.join("versions/second/second.jar.part").exists());
    }

    #[test]
    fn a_fetched_jar_that_fails_its_digest_is_not_stored() {
        let dir = scratch("tampered");
        let download = download_of(b"a client jar");

        let error = jar(&dir, "second", &download, |_| Ok(b"another jar!".to_vec())).unwrap_err();

        assert!(error.contains(&download.url), "{error}");
        assert!(!dir.join("versions/second/second.jar").exists());
        assert!(!dir.join("versions/second/second.jar.part").exists());
    }

    #[test]
    fn a_stored_jar_that_fails_its_digest_is_fetched_again() {
        let dir = scratch("stale");
        let bytes = b"a client jar";
        fs::create_dir_all(dir.join("versions/second")).unwrap();
        fs::write(dir.join("versions/second/second.jar"), b"half a jar").unwrap();

        let stored = jar(&dir, "second", &download_of(bytes), |_| Ok(bytes.to_vec())).unwrap();

        assert_eq!(stored, bytes);
        assert_eq!(
            fs::read(dir.join("versions/second/second.jar")).unwrap(),
            bytes
        );
    }

    #[test]
    fn a_jar_that_states_another_version_is_refused() {
        assert!(check_version(SAMPLE_VERSION.as_bytes(), "second").is_ok());

        let error = check_version(SAMPLE_VERSION.as_bytes(), "first").unwrap_err();
        assert!(
            error.contains("first") && error.contains("second"),
            "{error}"
        );
        assert!(check_version(b"{}", "second").is_err());
    }

    #[test]
    fn the_descriptor_parses_back_and_ends_in_one_newline() {
        let jar = sample_jar();
        let listing = resolve(MANIFEST.as_bytes(), "second").unwrap();
        let download = download_of(&jar);

        let text = descriptor(&download, &jar, &listing, PACKAGE.as_bytes()).unwrap();

        assert!(text.ends_with("}\n") && !text.ends_with("\n\n"));
        let parsed: Release = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed.id, "second");
        assert_eq!(parsed.jar.url, download.url);
        assert_eq!(parsed.jar.sha1, sha1_hex(&jar));
        assert_eq!(parsed.jar.size, jar.len() as u64);
        let (directory_sha1, directory_size) =
            mcrs_minecraft_client_jar::directory_digest(&jar).unwrap();
        assert_eq!(parsed.directory.url, download.url);
        assert_eq!(parsed.directory.sha1, directory_sha1);
        assert_eq!(parsed.directory.size, directory_size);
        assert_eq!(parsed.json.url, listing.url);
        assert_eq!(parsed.json.sha1, listing.sha1);
        assert_eq!(parsed.json.size, PACKAGE.len() as u64);
    }

    #[test]
    fn the_font_hint_parses_back_and_ends_in_one_newline() {
        let jar = sample_jar();

        let text = font_hint(&jar).unwrap();

        assert!(text.ends_with("}\n") && !text.ends_with("\n\n"));
        let hint: FontHint = serde_json::from_str(&text).unwrap();
        assert_eq!(hint, FontHint::of(&jar, &sha1_hex(&jar)).unwrap());
    }

    #[test]
    fn a_dirty_tree_is_refused_unless_the_override_is_given() {
        let status = " M assets/minecraft/version.json\n";

        let error = refuse_dirty(status, false).unwrap_err();

        assert!(error.contains("assets/minecraft/version.json"), "{error}");
        assert!(error.contains("--allow-dirty"), "{error}");
        assert!(refuse_dirty(status, true).is_ok());
        assert!(refuse_dirty("", false).is_ok());
        assert!(refuse_dirty("\n", false).is_ok());
    }
}
