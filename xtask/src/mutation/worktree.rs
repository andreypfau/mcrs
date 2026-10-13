use crate::git;
use std::io::Write;
use std::path::{Path, PathBuf};

const BASE: &str = "target/mutation";

pub(crate) struct Tree {
    pub path: PathBuf,
    pub target_dir: PathBuf,
    pub out_dir: PathBuf,
    lock: Option<PathBuf>,
}

impl Drop for Tree {
    fn drop(&mut self) {
        if let Some(lock) = &self.lock {
            let _ = std::fs::remove_file(lock);
        }
    }
}

pub(crate) fn checkout(repo: &Path) -> Tree {
    Tree {
        path: repo.to_path_buf(),
        target_dir: repo.join("target"),
        out_dir: repo.join(BASE).join("shards"),
        lock: None,
    }
}

pub(crate) fn prepare(repo: &Path, sha: &str) -> Result<Tree, String> {
    let base = repo.join(BASE);
    std::fs::create_dir_all(&base)
        .map_err(|e| format!("could not create {}: {e}", base.display()))?;
    let lock = base.join("tree.lock");
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock)
    {
        Ok(mut file) => {
            let _ = writeln!(file, "{}", std::process::id());
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(format!(
                "{} exists, so another mutation run holds the dedicated tree; wait for it to finish, or delete the file if no run is active",
                lock.display()
            ));
        }
        Err(e) => return Err(format!("could not create {}: {e}", lock.display())),
    }
    let tree = Tree {
        path: base.join("tree"),
        target_dir: base.join("target"),
        out_dir: base.join("out"),
        lock: Some(lock),
    };
    force_clean(repo, &tree.path, sha)?;
    Ok(tree)
}

// chisle: one lane directory stays per distinct lane key, never pruned; the keys are workspace
// package names, so the count is bounded by the workspace.
pub(crate) fn prepare_lane(
    repo: &Path,
    sha: &str,
    key: &str,
    shared: &Tree,
) -> Result<Tree, String> {
    let lane = repo.join(BASE).join(format!("lane-{key}"));
    let tree = Tree {
        path: lane.join("tree"),
        target_dir: lane.join("target"),
        out_dir: shared.out_dir.clone(),
        lock: None,
    };
    force_clean(repo, &tree.path, sha)?;
    Ok(tree)
}

fn force_clean(repo: &Path, path: &Path, sha: &str) -> Result<(), String> {
    if !path.join(".git").is_file() {
        if path.exists() {
            return Err(format!(
                "{} exists but is not a worktree; remove it so that it can be created again",
                path.display()
            ));
        }
        git::run(repo, &["worktree", "prune"]).map_err(|e| e.to_string())?;
        let target = path.to_string_lossy();
        git::run(
            repo,
            &["worktree", "add", "--detach", "--quiet", &target, sha],
        )
        .map_err(|e| e.to_string())?;
        return Ok(());
    }
    let top = git::text(path, &["rev-parse", "--show-toplevel"]).map_err(|e| e.to_string())?;
    let own = path
        .canonicalize()
        .map_err(|e| format!("could not resolve {}: {e}", path.display()))?;
    if Path::new(top.trim()) != own {
        return Err(format!(
            "{} is not the root of its own worktree, so it is not reset or cleaned",
            path.display()
        ));
    }
    git::run(path, &["reset", "--hard", "--quiet", sha]).map_err(|e| e.to_string())?;
    git::run(path, &["clean", "-fdx", "--quiet"]).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scope::tests::Repo;
    use std::path::Path;

    fn head_of(tree: &Path) -> String {
        let output = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(tree)
            .output()
            .unwrap();
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    #[test]
    fn the_dedicated_worktree_is_forced_clean_before_a_run() {
        let repo = Repo::new("dedicated-tree");
        repo.init();
        repo.write(".gitignore", "*.log\n");
        repo.write("tracked.txt", "one\n");
        repo.commit("first");
        let first = repo.sha();
        repo.write("tracked.txt", "two\n");
        repo.commit("second");
        let second = repo.sha();

        {
            let tree = prepare(&repo.0, &first).unwrap();
            assert_eq!(
                std::fs::read_to_string(tree.path.join("tracked.txt")).unwrap(),
                "one\n"
            );
            assert!(tree.path.starts_with(repo.0.join("target/mutation")));
            assert!(
                !tree.target_dir.starts_with(&tree.path),
                "{:?}",
                tree.target_dir
            );

            let second_run = prepare(&repo.0, &second)
                .err()
                .expect("a second run must fail");
            assert!(
                second_run.contains("tree.lock"),
                "the failure names the lock: {second_run}"
            );

            std::fs::write(tree.path.join("tracked.txt"), "edited by a mutant\n").unwrap();
            std::fs::write(tree.path.join("untracked.txt"), "left behind\n").unwrap();
            std::fs::write(tree.path.join("ignored.log"), "left behind\n").unwrap();
        }

        let tree = prepare(&repo.0, &second).unwrap();
        assert_eq!(
            std::fs::read_to_string(tree.path.join("tracked.txt")).unwrap(),
            "two\n"
        );
        assert!(!tree.path.join("untracked.txt").exists());
        assert!(!tree.path.join("ignored.log").exists());
        assert_eq!(head_of(&tree.path), second);
    }
}
