use std::io;
use std::path::{Path, PathBuf};

use mcrs_minecraft_core::RegionPos;
use mcrs_voxel::ColumnPos;

/// Copies a region and the external chunks it references beside the world, so a damaged
/// source is never overwritten before a person sees it.
pub(crate) fn quarantine_region(
    root: &Path,
    region_dir: &Path,
    at: RegionPos,
    externals: &[ColumnPos],
    reason: &str,
) -> io::Result<PathBuf> {
    let region_root = root.join(format!("r.{}.{}", at.x, at.z));
    std::fs::create_dir_all(&region_root)?;
    let attempt = next_attempt(&region_root)?;
    let result = fill_attempt(&attempt, region_dir, at, externals, reason);
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&attempt);
    }
    result.map(|()| attempt)
}

fn next_attempt(root: &Path) -> io::Result<PathBuf> {
    let mut number = 1u32;
    loop {
        let attempt = root.join(format!("attempt-{number}"));
        match std::fs::create_dir(&attempt) {
            Ok(()) => return Ok(attempt),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                number = number.saturating_add(1);
            }
            Err(error) => return Err(error),
        }
    }
}

fn fill_attempt(
    attempt: &Path,
    region_dir: &Path,
    at: RegionPos,
    externals: &[ColumnPos],
    reason: &str,
) -> io::Result<()> {
    let region_name = format!("r.{}.{}.mca", at.x, at.z);
    copy_durably(&region_dir.join(&region_name), &attempt.join(&region_name))?;
    for pos in externals {
        let name = format!("c.{}.{}.mcc", pos.x, pos.z);
        copy_durably(&region_dir.join(&name), &attempt.join(&name))?;
    }
    std::fs::write(attempt.join("report.txt"), format!("{reason}\n"))
}

fn copy_durably(from: &Path, to: &Path) -> io::Result<()> {
    std::fs::copy(from, to)?;
    std::fs::File::open(to)?.sync_all()
}

/// The external chunk files in `region_dir` that lie in the range of `at`, for a region whose
/// header cannot be read and so cannot name the records that use them.
pub(crate) fn existing_externals(region_dir: &Path, at: RegionPos) -> Vec<ColumnPos> {
    (0..32)
        .flat_map(|z| (0..32).map(move |x| at.column_at(x, z)))
        .filter(|pos| {
            region_dir
                .join(format!("c.{}.{}.mcc", pos.x, pos.z))
                .is_file()
        })
        .collect()
}
