use std::path::{Path, PathBuf};

/// A fresh working directory under the system temporary directory, removed on drop.
pub struct ScratchDir {
    path: PathBuf,
}

impl ScratchDir {
    pub fn new(name: &str) -> std::io::Result<Self> {
        let path = std::env::temp_dir().join(format!("perf-kit-{}-{name}", std::process::id()));
        if path.exists() {
            std::fs::remove_dir_all(&path)?;
        }
        std::fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
