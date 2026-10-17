use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::Ordering;

use crate::native::{
    FIRST_BACKOFF, WORKERS, candidates, download, download_dir, home, locate, read_counting,
};
use crate::{Directory, Progress, RELEASE, Release, verify};

const DATA: &str = "data/minecraft/";
const VERSION: &str = "version.json";
const COMPLETE: &str = ".complete";
const LOCK: &str = ".lock";

/// Where the pinned client jar is read from when its data pack is not unpacked yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientJar {
    /// This file, which must be the pinned release's jar.
    At(PathBuf),
    /// A launcher's installation; nothing is downloaded.
    Installed,
    /// A launcher's installation, or else a download into the official launcher's directory.
    InstalledOrDownloaded,
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum VanillaDataError {
    #[error("the system names no cache directory for the vanilla data pack")]
    NoCacheDir,
    #[error("the system names no home for the launcher directory a download goes to")]
    NoLauncherDir,
    #[error("{} is not the client jar of {id}", path.display())]
    NotTheRelease { path: PathBuf, id: &'static str },
    #[error("no launcher has installed the client jar of {id}")]
    NotInstalled { id: &'static str },
    #[error("the client jar is not a readable zip: {0}")]
    MalformedJar(String),
    #[error("the client jar holds no version.json")]
    NoVersionFile,
    #[error("the client jar entry {name} is not a plain relative path")]
    UnsafeEntry { name: String },
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> VanillaDataError + '_ {
    move |source| VanillaDataError::Io {
        path: path.to_owned(),
        source,
    }
}

/// The directory holding the pinned client jar's `data/minecraft` tree and its `version.json`,
/// unpacked once below `cache`, or the user's cache directory when it is `None`, and shared
/// by every process after that.
pub fn vanilla_data(
    jar: &ClientJar,
    cache: Option<&Path>,
    progress: &Progress,
) -> Result<PathBuf, VanillaDataError> {
    let cache = match cache {
        Some(cache) => cache.to_owned(),
        None => cache_dir().ok_or(VanillaDataError::NoCacheDir)?,
    };
    unpacked(&cache, &RELEASE, progress, || jar_bytes(jar, progress))
}

#[cfg_attr(
    not(target_os = "macos"),
    expect(
        clippy::disallowed_methods,
        reason = "the cache directory is an operating-system location, not configuration"
    )
)]
fn cache_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    return home().map(|home| home.join("Library/Caches/mcrs"));
    #[cfg(windows)]
    return std::env::var_os("LOCALAPPDATA").map(|dir| PathBuf::from(dir).join("mcrs"));
    #[cfg(not(any(target_os = "macos", windows)))]
    return std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .or_else(|| home().map(|home| home.join(".cache")))
        .map(|dir| dir.join("mcrs"));
}

fn jar_bytes(jar: &ClientJar, progress: &Progress) -> Result<Vec<u8>, VanillaDataError> {
    let installed = || locate(&candidates(), &RELEASE.jar, progress);
    match jar {
        ClientJar::At(path) => verified(path, progress),
        ClientJar::Installed => {
            installed().ok_or(VanillaDataError::NotInstalled { id: RELEASE.id })
        }
        ClientJar::InstalledOrDownloaded => match installed() {
            Some(bytes) => Ok(bytes),
            None => {
                let dir = download_dir()?;
                let (_, remainder) = download(
                    &RELEASE,
                    &dir,
                    WORKERS,
                    FIRST_BACKOFF,
                    progress,
                    |_| false,
                    drop,
                );
                if let Some(remainder) = remainder {
                    remainder.finish();
                }
                verified(&dir.join(format!("{}.jar", RELEASE.id)), progress)
            }
        },
    }
}

fn verified(path: &Path, progress: &Progress) -> Result<Vec<u8>, VanillaDataError> {
    progress.total.store(RELEASE.jar.size, Ordering::Relaxed);
    let bytes = read_counting(path, progress).map_err(io(path))?;
    if verify(&RELEASE.jar, &bytes) {
        Ok(bytes)
    } else {
        Err(VanillaDataError::NotTheRelease {
            path: path.to_owned(),
            id: RELEASE.id,
        })
    }
}

// Unpacking runs under an exclusive lock, so one process reads the jar while the others wait
// and then find the tree complete. A tree is renamed into place only once it holds everything,
// so a reader that checks for the marker never sees half of one.
// chisle: the trees of other jars stay until removed by hand, since a process of another
// version may still read its own; a last-used stamp per tree would let them be swept.
// chisle: files are not synced before the rename, so a power loss right after it can leave a
// marked tree with truncated files; syncing every file first lifts this at a cost per file.
fn unpacked(
    cache: &Path,
    release: &Release,
    progress: &Progress,
    jar: impl FnOnce() -> Result<Vec<u8>, VanillaDataError>,
) -> Result<PathBuf, VanillaDataError> {
    let parent = cache.join("vanilla");
    let dir = parent.join(release.jar.sha1);
    if dir.join(COMPLETE).is_file() {
        return Ok(dir);
    }
    fs::create_dir_all(&parent).map_err(io(&parent))?;
    let lock = parent.join(LOCK);
    let lock = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock)
        .and_then(|file: File| file.lock().map(|()| file))
        .map_err(io(&lock))?;
    if dir.join(COMPLETE).is_file() {
        return Ok(dir);
    }
    sweep(&parent, &dir)?;
    let bytes = jar()?;
    if let Ok(mut status) = progress.status.lock() {
        *status = format!("Unpacking the data pack into {}", dir.display());
    }
    let staging = parent.join(format!(".{}", release.jar.sha1));
    let published =
        write_data(&bytes, &staging).and_then(|()| fs::rename(&staging, &dir).map_err(io(&dir)));
    if published.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    drop(lock);
    published.map(|()| dir)
}

/// Removes what a process killed while it held the lock left behind: staging trees, and the
/// unmarked tree at `dir`, which is first moved aside so the live path is never deleted.
fn sweep(parent: &Path, dir: &Path) -> Result<(), VanillaDataError> {
    if dir.exists() {
        let aside = parent.join(format!(".stale-{}", std::process::id()));
        fs::rename(dir, &aside).map_err(io(dir))?;
    }
    for entry in fs::read_dir(parent).map_err(io(parent))? {
        let path = entry.map_err(io(parent))?.path();
        let name = path.file_name().and_then(OsStr::to_str).unwrap_or_default();
        if name.starts_with('.') && name != LOCK {
            fs::remove_dir_all(&path).map_err(io(&path))?;
        }
    }
    Ok(())
}

fn write_data(jar: &[u8], staging: &Path) -> Result<(), VanillaDataError> {
    let directory = Directory::of(jar).map_err(VanillaDataError::MalformedJar)?;
    let mut version = false;
    for file in directory.inflated(jar, |name| name == VERSION || name.starts_with(DATA)) {
        let (name, bytes) = file.map_err(VanillaDataError::MalformedJar)?;
        version |= name == VERSION;
        let relative = name.strip_prefix(DATA).unwrap_or(name);
        if !is_plain(relative) {
            return Err(VanillaDataError::UnsafeEntry {
                name: name.to_owned(),
            });
        }
        write(&staging.join(relative), &bytes)?;
    }
    if !version {
        return Err(VanillaDataError::NoVersionFile);
    }
    write(&staging.join(COMPLETE), &[])
}

fn is_plain(relative: &str) -> bool {
    !relative.is_empty()
        && relative.split('/').all(|part| {
            let mut components = Path::new(part).components();
            !part.contains(['\\', '\0'])
                && matches!(
                    (components.next(), components.next()),
                    (Some(Component::Normal(name)), None) if name == OsStr::new(part)
                )
        })
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), VanillaDataError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io(parent))?;
    }
    fs::write(path, bytes).map_err(io(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::sample_jar;
    use crate::{Artifact, sha1_hex};

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "mcrs_client_jar_data-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn release(sha1: &'static str) -> Release<'static> {
        let artifact = Artifact {
            url: "",
            sha1,
            size: 0,
        };
        Release {
            id: "sample",
            jar: artifact,
            directory: artifact,
            json: artifact,
            font_hint: "",
        }
    }

    fn listing(dir: &Path) -> Vec<String> {
        let mut found = Vec::new();
        let mut directories = vec![dir.to_owned()];
        while let Some(directory) = directories.pop() {
            for entry in fs::read_dir(&directory).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    directories.push(path);
                } else {
                    let relative = path.strip_prefix(dir).unwrap().to_str().unwrap();
                    found.push(relative.replace('\\', "/"));
                }
            }
        }
        found.sort();
        found
    }

    #[test]
    fn the_data_pack_and_version_are_unpacked_once_and_reused_without_the_jar() {
        let cache = scratch("once");
        let jar = sample_jar();
        let sha1: &'static str = sha1_hex(&jar).leak();
        let release = release(sha1);
        let progress = Progress::default();

        let dir = unpacked(&cache, &release, &progress, || Ok(jar.clone())).unwrap();

        assert_eq!(
            listing(&dir),
            [
                COMPLETE,
                "tags/block/fake.json",
                "textures/fake.png",
                "version.json"
            ]
        );
        assert_eq!(
            fs::read(dir.join("version.json")).unwrap(),
            br#"{"id":"sample"}"#
        );
        let again = unpacked(&cache, &release, &progress, || {
            Err(VanillaDataError::MalformedJar(
                "read the jar again".to_owned(),
            ))
        });
        assert_eq!(again.unwrap(), dir);
        let mut held: Vec<_> = fs::read_dir(cache.join("vanilla"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        held.sort();
        assert_eq!(held, [LOCK, sha1]);
    }

    #[test]
    fn what_a_killed_unpack_left_is_swept_and_the_tree_unpacked_again() {
        let cache = scratch("incomplete");
        let jar = sample_jar();
        let release = release(sha1_hex(&jar).leak());
        let parent = cache.join("vanilla");
        let dir = parent.join(release.jar.sha1);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("half.json"), b"{").unwrap();
        fs::create_dir_all(parent.join(".staging-of-a-killed-process")).unwrap();

        let unpacked = unpacked(&cache, &release, &Progress::default(), || Ok(jar)).unwrap();

        assert!(unpacked.join(COMPLETE).is_file());
        assert!(!unpacked.join("half.json").exists());
        assert!(!parent.join(".staging-of-a-killed-process").exists());
    }

    #[test]
    fn only_plain_relative_entry_paths_are_written() {
        for (path, plain) in [
            ("tags/block/logs.json", true),
            ("version.json", true),
            ("", false),
            ("tags//logs.json", false),
            ("tags/./logs.json", false),
            ("tags/../../outside.json", false),
            ("/absolute.json", false),
            ("tags\\logs.json", false),
            ("nul\0.json", false),
        ] {
            assert_eq!(is_plain(path), plain, "{path:?}");
        }
    }
}
