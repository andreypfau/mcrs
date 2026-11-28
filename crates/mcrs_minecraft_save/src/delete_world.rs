use std::fs;
use std::io;
use std::path::Path;
use std::thread;
use std::time::Duration;

use crate::SaveError;
use crate::SaveLock;

const ATTEMPTS: u32 = 5;
const GAP: Duration = Duration::from_millis(500);
const LOCK_FILE: &str = "session.lock";

pub fn delete_world(dir: &Path) -> Result<(), SaveError> {
    if fs::symlink_metadata(dir).is_ok_and(|metadata| metadata.is_symlink()) {
        return retry(dir, || remove_entry(dir));
    }
    let lock = SaveLock::take(dir)?;
    retry(dir, || remove_contents(dir))?;
    retry(dir, || {
        fs::remove_file(dir.join(LOCK_FILE)).or_else(missing)
    })?;
    drop(lock);
    retry(dir, || fs::remove_dir(dir))
}

fn retry(dir: &Path, mut removal: impl FnMut() -> io::Result<()>) -> Result<(), SaveError> {
    let mut attempt = 1;
    loop {
        match removal() {
            Ok(()) => return Ok(()),
            Err(source) if attempt >= ATTEMPTS => {
                return Err(SaveError::Io {
                    path: dir.to_owned(),
                    source,
                });
            }
            Err(_) => {
                attempt += 1;
                thread::sleep(GAP);
            }
        }
    }
}

fn remove_contents(dir: &Path) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_name() != LOCK_FILE {
            remove_entry(&entry.path())?;
        }
    }
    Ok(())
}

fn remove_entry(path: &Path) -> io::Result<()> {
    let file_type = fs::symlink_metadata(path)?.file_type();
    if file_type.is_dir() {
        fs::remove_dir_all(path)
    } else if is_directory_link(file_type) {
        fs::remove_dir(path)
    } else {
        fs::remove_file(path)
    }
}

#[cfg(windows)]
fn is_directory_link(file_type: fs::FileType) -> bool {
    use std::os::windows::fs::FileTypeExt;
    file_type.is_symlink_dir()
}

#[cfg(not(windows))]
fn is_directory_link(_: fs::FileType) -> bool {
    false
}

fn missing(error: io::Error) -> io::Result<()> {
    match error.kind() {
        io::ErrorKind::NotFound => Ok(()),
        _ => Err(error),
    }
}
