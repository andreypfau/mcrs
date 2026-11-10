use std::path::{Path, PathBuf};

use serde::Serializer;

/// `path` as the report shows it: relative to `out` when inside it, else without the home directory.
#[must_use]
pub fn under_out(path: &Path, out: &Path) -> PathBuf {
    match path.strip_prefix(out) {
        Ok(relative) => relative.to_path_buf(),
        Err(_) => without_home(path),
    }
}

/// `path` with the tester's home directory written as `~`, so a shared report names no account.
#[must_use]
pub fn without_home(path: &Path) -> PathBuf {
    let Some(home) = home_dir() else {
        return path.to_path_buf();
    };
    match path.strip_prefix(&home) {
        Ok(rest) => Path::new("~").join(rest),
        Err(_) => path.to_path_buf(),
    }
}

pub fn serialize_without_home<S: Serializer>(
    path: &impl AsRef<Path>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_str(&without_home(path.as_ref()).display())
}

#[expect(
    clippy::disallowed_methods,
    reason = "the kit reads the home directory only to keep the account name out of the report"
)]
fn home_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    if home.is_empty() {
        return None;
    }
    Some(PathBuf::from(home))
}
