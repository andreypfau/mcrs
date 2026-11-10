use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::bail;

/// The prebuilt release binary `name` of the repository, or an error that names its build.
pub fn binary(repo: &Path, name: &str, package: &str) -> anyhow::Result<PathBuf> {
    let path = repo
        .join("target")
        .join("release")
        .join(name)
        .with_extension(std::env::consts::EXE_EXTENSION);
    if !path.is_file() {
        bail!(
            "{} is missing; build it with `cargo build --release -p {package}`",
            path.display()
        );
    }
    Ok(path)
}

#[must_use]
pub fn git_commit(repo: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// A loopback port nothing listens on at the time of the call.
pub fn free_port() -> std::io::Result<u16> {
    Ok(TcpListener::bind("127.0.0.1:0")?.local_addr()?.port())
}
