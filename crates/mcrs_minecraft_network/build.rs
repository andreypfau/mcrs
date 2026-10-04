use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

const PLACEHOLDER: &str = "unknown";

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

fn git_path(dir: &Path, name: &str) -> Option<PathBuf> {
    git(
        dir,
        &["rev-parse", "--path-format=absolute", "--git-path", name],
    )
    .map(PathBuf::from)
}

fn is_workspace_repository(dir: &Path, root: &Path) -> bool {
    let Some(top) = git(dir, &["rev-parse", "--show-toplevel"]) else {
        return false;
    };
    matches!(
        (Path::new(&top).canonicalize(), root.canonicalize()),
        (Ok(top), Ok(root)) if top == root
    )
}

fn commit_hash(dir: &Path, root: &Path) -> Option<String> {
    if !is_workspace_repository(dir, root) {
        return None;
    }
    git(dir, &["rev-parse", "--verify", "HEAD"])
        .filter(|hash| hash.len() == 40 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
}

// A packed branch has no loose ref file until the next commit writes one, so the nearest
// existing ancestor directory (inside refs/heads) is watched in its place.
fn nearest_existing(path: PathBuf) -> Option<PathBuf> {
    path.ancestors().find(|p| p.exists()).map(Path::to_path_buf)
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("crate is two levels below workspace root");

    println!("cargo:rerun-if-changed=build.rs");

    let Some(hash) = commit_hash(&manifest_dir, root) else {
        println!("cargo:rustc-env=MCRS_COMMIT_HASH={PLACEHOLDER}");
        return;
    };
    println!("cargo:rustc-env=MCRS_COMMIT_HASH={hash}");

    let mut paths: Vec<PathBuf> = ["HEAD", "packed-refs"]
        .into_iter()
        .filter_map(|name| git_path(&manifest_dir, name))
        .filter(|path| path.exists())
        .collect();
    if let Some(reference) = git(&manifest_dir, &["symbolic-ref", "-q", "HEAD"])
        && let Some(path) = git_path(&manifest_dir, &reference).and_then(nearest_existing)
    {
        paths.push(path);
    }
    for path in paths {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}
