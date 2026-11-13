// std: std::fs
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

pub(crate) fn temp_path(path: &Path) -> io::Result<PathBuf> {
    let mut name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?
        .to_owned();
    name.push(format!(
        ".{}.{}.tmp",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    Ok(path.with_file_name(name))
}

/// Replaces `path` with `bytes` so that a crash at any point leaves the previous file whole.
///
/// The bytes go to a temporary file next to `path` and reach the disk before the rename. The
/// directory is synced after the rename, so the new name survives a power loss too. A failed
/// write removes the temporary file and leaves `path` as it was.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    if let Some(parent) = parent {
        fs::create_dir_all(parent)?;
    }
    let tmp = temp_path(path)?;
    let written = File::create(&tmp).and_then(|mut file| {
        file.write_all(bytes)?;
        file.sync_all()
    });
    if let Err(err) = written.and_then(|()| fs::rename(&tmp, path)) {
        let _ = fs::remove_file(&tmp);
        return Err(err);
    }
    // The new file is already in place; a failed directory sync only weakens durability of the
    // name, and reporting it would make the caller treat a finished save as failed.
    #[cfg(unix)]
    let _ = File::open(parent.unwrap_or(Path::new("."))).and_then(|dir| dir.sync_all());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("mcrs-write-atomic-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_write_replaces_the_file_and_leaves_no_temporary_file() {
        let dir = temp_dir("replace");
        let path = dir.join("nested/level.dat");
        write_atomic(&path, b"old").unwrap();
        write_atomic(&path, b"new").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new");
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn two_temporary_names_for_one_target_differ() {
        let path = Path::new("r.0.0.mca");
        assert_ne!(temp_path(path).unwrap(), temp_path(path).unwrap());
    }

    #[test]
    fn a_failed_write_leaves_the_old_file() {
        let dir = temp_dir("failed");
        let path = dir.join("level.dat");
        fs::create_dir_all(path.join("occupied")).unwrap();
        assert!(write_atomic(&path, b"new").is_err());
        assert!(path.join("occupied").is_dir());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        let _ = fs::remove_dir_all(&dir);
    }
}
