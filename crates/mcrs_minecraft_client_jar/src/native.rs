use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::{Artifact, CLIENT_JAR, Files, Progress, entries, verify};

const INSTALLED_JAR: &str = "versions/26.3/26.3.jar";
const CHUNK: u64 = 1 << 20;

#[cfg(not(windows))]
fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .expect("HOME is set")
}

/// The game directory the official launcher uses.
pub fn official_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    return home().join("Library/Application Support/minecraft");
    #[cfg(windows)]
    return std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .expect("APPDATA is set")
        .join(".minecraft");
    #[cfg(not(any(target_os = "macos", windows)))]
    return home().join(".minecraft");
}

/// Where launchers keep an installed 26.3 client jar, most likely first.
pub fn candidates() -> Vec<PathBuf> {
    #[allow(unused_mut)]
    let mut paths = vec![official_dir().join(INSTALLED_JAR)];
    #[cfg(target_os = "macos")]
    {
        let support = home().join("Library/Application Support");
        paths.push(
            support.join(
                "PrismLauncher/libraries/com/mojang/minecraft/26.3/minecraft-26.3-client.jar",
            ),
        );
        for settings in ["tlauncher-2.0.properties", "legacy.properties"] {
            let Ok(text) = fs::read_to_string(support.join("tlauncher").join(settings)) else {
                continue;
            };
            paths.extend(
                text.lines()
                    .filter_map(|line| line.strip_prefix("minecraft.gamedir="))
                    .map(|dir| Path::new(dir.trim()).join(INSTALLED_JAR)),
            );
        }
    }
    paths
}

/// The first of `paths` whose size and SHA-1 are the artifact's. Anything else, including a
/// patched or half-written jar, is skipped rather than trusted.
pub fn locate(paths: &[PathBuf], artifact: &Artifact, progress: &Progress) -> Option<Vec<u8>> {
    paths.iter().find_map(|path| {
        if fs::metadata(path).ok()?.len() != artifact.size {
            return None;
        }
        progress.set_total(artifact.size);
        progress.set_status(format!("Checking {}", path.display()));
        let bytes = read_counting(path, progress).ok()?;
        if !verify(artifact, &bytes) {
            tracing::warn!(path = %path.display(), expected = artifact.sha1, "skipping a file with the wrong SHA-1");
            return None;
        }
        tracing::info!(path = %path.display(), "using {}", artifact.url);
        Some(bytes)
    })
}

fn read_counting(path: &Path, progress: &Progress) -> std::io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let mut bytes = Vec::new();
    progress.set_done(0);
    while (&mut file).take(CHUNK).read_to_end(&mut bytes)? > 0 {
        progress.set_done(bytes.len() as u64);
    }
    Ok(bytes)
}

/// The picked `assets/` files of the 26.3 client jar, from whichever launcher installed it.
pub fn resolve(progress: &Progress, pick: impl FnMut(&str) -> bool) -> Files {
    let candidates = candidates();
    let jar = locate(&candidates, &CLIENT_JAR, progress)
        .unwrap_or_else(|| panic!("no verified 26.3 client jar at any of {candidates:?}"));
    entries(&jar, pick)
}

#[cfg(test)]
mod tests {
    use crate::sha1_hex;

    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("mcrs_client_jar-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_candidate_of_the_wrong_size_is_not_read() {
        let dir = scratch("wrong-size");
        let jar = dir.join("client.jar");
        fs::write(&jar, b"too short").unwrap();
        let sha1 = sha1_hex(b"client jar");
        let artifact = Artifact {
            url: "",
            sha1: &sha1,
            size: 10,
        };
        let progress = Progress::default();

        assert_eq!(locate(&[jar], &artifact, &progress), None);
        assert_eq!(progress.status(), "");
        assert_eq!(progress.total(), 0);
    }

    #[test]
    fn a_candidate_with_the_wrong_digest_is_skipped_for_the_next() {
        let dir = scratch("wrong-digest");
        let patched = dir.join("patched.jar");
        let genuine = dir.join("genuine.jar");
        fs::write(&patched, b"client jaR").unwrap();
        fs::write(&genuine, b"client jar").unwrap();
        let sha1 = sha1_hex(b"client jar");
        let artifact = Artifact {
            url: "",
            sha1: &sha1,
            size: 10,
        };
        let progress = Progress::default();

        let found = locate(
            &[dir.join("missing.jar"), patched, genuine],
            &artifact,
            &progress,
        );

        assert_eq!(found.as_deref(), Some(&b"client jar"[..]));
        assert_eq!(progress.done(), 10);
    }

    #[test]
    #[ignore = "reads the launchers installed on this machine"]
    fn locates_a_launcher_jar_on_this_machine() {
        let candidates = candidates();
        let progress = Progress::default();
        let found = locate(&candidates, &CLIENT_JAR, &progress);
        println!("checked {candidates:#?}");
        println!("{}", progress.status());
        assert!(found.is_some());
    }
}
