use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use crate::SaveError;

const FILE_NAME: &str = "session.lock";
const CONTENT: &[u8] = "\u{2603}".as_bytes();

#[derive(Debug)]
pub struct SaveLock {
    dir: PathBuf,
    _file: File,
}

impl SaveLock {
    pub fn take(dir: &Path) -> Result<Self, SaveError> {
        let path = dir.join(FILE_NAME);
        let io = |source| SaveError::Io {
            path: path.clone(),
            source,
        };
        fs::create_dir_all(dir).map_err(io)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(io)?;
        match file.write_all(CONTENT).and_then(|()| file.sync_all()) {
            Err(error) if is_held_by_other(&error) => {
                return Err(SaveError::Locked {
                    dir: dir.to_owned(),
                });
            }
            written => written.map_err(io)?,
        }
        match try_lock(&file) {
            Ok(()) => Ok(Self {
                dir: dir.to_owned(),
                _file: file,
            }),
            Err(TryLockError::WouldBlock) => Err(SaveError::Locked {
                dir: dir.to_owned(),
            }),
            Err(TryLockError::Error(source)) => Err(io(source)),
        }
    }

    pub fn is_held(dir: &Path) -> Result<bool, SaveError> {
        let path = dir.join(FILE_NAME);
        let file = match OpenOptions::new().write(true).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
            Err(error) if error.kind() == ErrorKind::PermissionDenied => return Ok(true),
            Err(source) => return Err(SaveError::Io { path, source }),
        };
        match try_lock(&file) {
            Ok(()) => Ok(false),
            Err(TryLockError::WouldBlock) => Ok(true),
            Err(TryLockError::Error(source)) => Err(SaveError::Io { path, source }),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

#[cfg(target_os = "linux")]
fn try_lock(file: &File) -> Result<(), TryLockError> {
    use std::os::fd::AsRawFd;
    let whole_file = libc::flock {
        l_type: libc::F_WRLCK as libc::c_short,
        l_whence: libc::SEEK_SET as libc::c_short,
        l_start: 0,
        l_len: 0,
        l_pid: 0,
    };
    // SAFETY: the descriptor is open for the call, and `whole_file` is a valid `flock` that
    // outlives it.
    let result = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_OFD_SETLK, &raw const whole_file) };
    if result == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    match error.raw_os_error() {
        Some(libc::EAGAIN | libc::EACCES) => Err(TryLockError::WouldBlock),
        _ => Err(TryLockError::Error(error)),
    }
}

#[cfg(not(target_os = "linux"))]
fn try_lock(file: &File) -> Result<(), TryLockError> {
    file.try_lock()
}

fn is_held_by_other(error: &std::io::Error) -> bool {
    cfg!(windows) && error.raw_os_error() == Some(33)
}
