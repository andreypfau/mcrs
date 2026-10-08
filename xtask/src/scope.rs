use crate::git::{self, Failure};
use crate::metadata::Workspace;
use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;

const ROOT_MANIFESTS: [&str; 2] = ["Cargo.toml", "Cargo.lock"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Working,
    Committed,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BaseError {
    MissingRef { base_ref: String },
    NoMergeBase { base_ref: String, shallow: bool },
    Git(String),
}

impl fmt::Display for BaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BaseError::MissingRef { base_ref } => write!(
                f,
                "the base ref {base_ref} does not exist; fetch it with git fetch, or set CHECKS_BASE_REF to a ref that exists"
            ),
            BaseError::NoMergeBase {
                base_ref,
                shallow: true,
            } => write!(
                f,
                "HEAD and {base_ref} share no commit in this shallow history; fetch the full history with git fetch --unshallow origin"
            ),
            BaseError::NoMergeBase {
                base_ref,
                shallow: false,
            } => write!(
                f,
                "HEAD and {base_ref} share no commit because their histories are unrelated; set CHECKS_BASE_REF to a ref this branch descends from"
            ),
            BaseError::Git(message) => write!(
                f,
                "{message}; check that git is installed and this is a repository"
            ),
        }
    }
}

pub fn resolve_base(repo: &Path, base_ref: &str) -> Result<String, BaseError> {
    let commit = format!("{base_ref}^{{commit}}");
    match git::run(repo, &["rev-parse", "--verify", "--quiet", &commit]) {
        Ok(_) => {}
        Err(Failure::Exit { .. }) => {
            return Err(BaseError::MissingRef {
                base_ref: base_ref.to_owned(),
            });
        }
        Err(other) => return Err(BaseError::Git(other.to_string())),
    }
    match git::text(repo, &["merge-base", "HEAD", base_ref]) {
        Ok(sha) => Ok(sha.trim().to_owned()),
        Err(Failure::Exit { .. }) => {
            let shallow = git::text(repo, &["rev-parse", "--is-shallow-repository"])
                .is_ok_and(|answer| answer.trim() == "true");
            Err(BaseError::NoMergeBase {
                base_ref: base_ref.to_owned(),
                shallow,
            })
        }
        Err(other) => Err(BaseError::Git(other.to_string())),
    }
}

fn diff_args(base: &str, mode: Mode) -> Vec<&str> {
    match mode {
        Mode::Working => vec![base],
        Mode::Committed => vec![base, "HEAD"],
    }
}

pub fn changed_paths(repo: &Path, base: &str, mode: Mode) -> Result<Vec<String>, String> {
    let mut args = vec!["diff", "--name-only", "-z", "--no-renames"];
    args.extend(diff_args(base, mode));
    let mut paths = null_separated(&git::run(repo, &args).map_err(|e| e.to_string())?);
    if matches!(mode, Mode::Working) {
        let untracked = git::run(repo, &["ls-files", "--others", "--exclude-standard", "-z"])
            .map_err(|e| e.to_string())?;
        paths.extend(null_separated(&untracked));
        paths.sort();
        paths.dedup();
    }
    let rust_differs = if paths.is_empty() {
        let mut quiet = diff_args(base, mode);
        quiet.extend(["--", "*.rs"]);
        git::differs(repo, &quiet).map_err(|e| e.to_string())?
    } else {
        false
    };
    ensure_consistent(&paths, rust_differs, base)?;
    Ok(paths)
}

fn null_separated(listing: &[u8]) -> Vec<String> {
    listing
        .split(|&byte| byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8_lossy(path).into_owned())
        .collect()
}

fn ensure_consistent(paths: &[String], rust_differs: bool, base: &str) -> Result<(), String> {
    if paths.is_empty() && rust_differs {
        return Err(format!(
            "git diff --name-only listed no changed file against {base}, but git diff --quiet reports a difference in *.rs files; \
             the changed-file list cannot be trusted, so fix the repository state and rerun"
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    Workspace,
    Packages(Vec<String>),
}

impl Scope {
    pub fn with_dependents(self, workspace: &Workspace) -> Scope {
        let Scope::Packages(changed) = self else {
            return self;
        };
        let mut all: BTreeSet<String> = changed.iter().cloned().collect();
        for package in &workspace.packages {
            if package
                .dependencies
                .iter()
                .any(|dep| dep.local && changed.contains(&dep.name))
            {
                all.insert(package.name.clone());
            }
        }
        Scope::Packages(all.into_iter().collect())
    }
}

enum Effect<'a> {
    Inert,
    Package(&'a str),
    Widen,
}

fn classify<'a>(workspace: &'a Workspace, path: &str, version_only: bool) -> Effect<'a> {
    if ROOT_MANIFESTS.contains(&path) {
        return if version_only {
            Effect::Inert
        } else {
            Effect::Widen
        };
    }
    if let Some(owner) = workspace.owner(path) {
        return Effect::Package(&owner.name);
    }
    let licence = path.starts_with("LICENSE-") && !path.contains('/');
    if path.ends_with(".md") || path.starts_with("docs/") || licence {
        return Effect::Inert;
    }
    Effect::Widen
}

pub fn scope(workspace: &Workspace, paths: &[String], version_only: bool) -> Scope {
    if paths.is_empty() {
        return Scope::Workspace;
    }
    let mut changed = BTreeSet::new();
    for path in paths {
        match classify(workspace, path, version_only) {
            Effect::Inert => {}
            Effect::Package(name) => {
                changed.insert(name.to_owned());
            }
            Effect::Widen => return Scope::Workspace,
        }
    }
    Scope::Packages(changed.into_iter().collect())
}

pub fn touches_root_manifests(paths: &[String]) -> bool {
    paths
        .iter()
        .any(|path| ROOT_MANIFESTS.contains(&path.as_str()))
}

pub struct ManifestTexts {
    pub base_toml: String,
    pub head_toml: String,
    pub base_lock: String,
    pub head_lock: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct VersionBump {
    pub from: String,
    pub to: String,
}

pub fn manifest_texts(repo: &Path, base: &str, mode: Mode) -> Option<ManifestTexts> {
    let at_base = |file: &str| git::text(repo, &["show", &format!("{base}:{file}")]).ok();
    let at_head = |file: &str| match mode {
        Mode::Working => std::fs::read_to_string(repo.join(file)).ok(),
        Mode::Committed => git::text(repo, &["show", &format!("HEAD:{file}")]).ok(),
    };
    Some(ManifestTexts {
        base_toml: at_base("Cargo.toml")?,
        head_toml: at_head("Cargo.toml")?,
        base_lock: at_base("Cargo.lock")?,
        head_lock: at_head("Cargo.lock")?,
    })
}

pub fn version_bump(workspace: &Workspace, texts: &ManifestTexts) -> Option<VersionBump> {
    let from = workspace_version(&texts.base_toml)?;
    let to = workspace_version(&texts.head_toml)?;
    let members: Vec<&str> = workspace.packages.iter().map(|p| p.name.as_str()).collect();
    let manifest_matches = retarget_manifest(&texts.head_toml, &to, &from) == texts.base_toml;
    let lock_matches = retarget_lock(&texts.head_lock, &to, &from, &members) == texts.base_lock;
    (manifest_matches && lock_matches).then_some(VersionBump { from, to })
}

fn string_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let rest = line
        .trim()
        .strip_prefix(key)?
        .trim_start()
        .strip_prefix('=')?
        .trim();
    rest.strip_prefix('"')?.strip_suffix('"')
}

fn workspace_version(manifest: &str) -> Option<String> {
    let mut in_workspace_package = false;
    for line in manifest.lines() {
        if line.trim_start().starts_with('[') {
            in_workspace_package = line.trim() == "[workspace.package]";
        } else if in_workspace_package && let Some(version) = string_value(line, "version") {
            return Some(version.to_owned());
        }
    }
    None
}

fn retarget_line(line: &str, from_text: &str, to_text: &str) -> String {
    if string_value(line, "version") != Some(from_text) {
        return line.to_owned();
    }
    let ending = &line[line.trim_end().len()..];
    format!("version = \"{to_text}\"{ending}")
}

fn retarget_manifest(manifest: &str, head: &str, base: &str) -> String {
    let mut in_versioned_section = false;
    manifest
        .split_inclusive('\n')
        .map(|line| {
            if line.trim_start().starts_with('[') {
                in_versioned_section = matches!(line.trim(), "[package]" | "[workspace.package]");
                line.to_owned()
            } else if in_versioned_section {
                retarget_line(line, head, base)
            } else {
                line.to_owned()
            }
        })
        .collect()
}

fn retarget_lock(lock: &str, head: &str, base: &str, members: &[&str]) -> String {
    let mut blocks: Vec<Vec<&str>> = vec![Vec::new()];
    for line in lock.split_inclusive('\n') {
        if line.trim() == "[[package]]" {
            blocks.push(Vec::new());
        }
        blocks
            .last_mut()
            .expect("blocks starts non-empty")
            .push(line);
    }
    let mut out = String::with_capacity(lock.len());
    for block in blocks {
        let member = block
            .iter()
            .any(|line| string_value(line, "name").is_some_and(|name| members.contains(&name)));
        for line in block {
            if member {
                out.push_str(&retarget_line(line, head, base));
            } else {
                out.push_str(line);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn fixture() -> Workspace {
        let package = |name: &str, dir: &str, deps: &[(&str, Option<&str>)]| {
            let dependencies: Vec<_> = deps
                .iter()
                .map(|(dep, kind)| json!({"name": dep, "kind": kind, "path": format!("/w/{dep}")}))
                .collect();
            json!({
                "name": name,
                "edition": "2024",
                "manifest_path": format!("/w/{dir}/Cargo.toml").replace("//", "/"),
                "dependencies": dependencies,
            })
        };
        let metadata = json!({
            "workspace_root": "/w",
            "packages": [
                package("mcrs", "", &[("a", None)]),
                package("x", "crates/x", &[]),
                package("a", "crates/a", &[("x", None)]),
                package("b", "crates/b", &[("x", Some("build"))]),
                package("c", "crates/c", &[("x", Some("dev"))]),
                package("mcrs_minecraft_light", "crates/mcrs_minecraft_light", &[]),
                package(
                    "mcrs_minecraft_light_color",
                    "crates/mcrs_minecraft_light_color",
                    &[("mcrs_minecraft_light", None)],
                ),
                package("xtask", "xtask", &[]),
            ],
        });
        Workspace::parse(&metadata.to_string()).unwrap()
    }

    fn manifest(version: &str, members: &str) -> String {
        format!(
            "[package]\nname = \"mcrs\"\nversion = \"{version}\"\n\n[workspace]\nmembers = [{members}]\n\n\
             [workspace.package]\nversion = \"{version}\"\nedition = \"2024\"\n"
        )
    }

    fn lock(entries: &[(&str, &str)]) -> String {
        let mut text = String::from("version = 4\n");
        for (name, version) in entries {
            text.push_str(&format!(
                "\n[[package]]\nname = \"{name}\"\nversion = \"{version}\"\n"
            ));
        }
        text
    }

    fn texts(head_manifest: String, head_lock: String) -> ManifestTexts {
        ManifestTexts {
            base_toml: manifest("0.5.1", "\"crates/*\""),
            head_toml: head_manifest,
            base_lock: lock(&[("mcrs", "0.5.1"), ("x", "0.5.1"), ("serde", "1.0.0")]),
            head_lock,
        }
    }

    fn bumped_lock(serde: &str, x: &str) -> String {
        lock(&[("mcrs", "0.5.2"), ("x", x), ("serde", serde)])
    }

    type Row = (
        &'static str,
        Vec<&'static str>,
        Option<ManifestTexts>,
        Option<&'static [&'static str]>,
    );

    #[test]
    fn changed_paths_map_to_packages_and_their_direct_dependents() {
        const X_AND_DEPENDENTS: &[&str] = &["a", "b", "c", "x"];
        let workspace = fixture();
        let bump_manifest = manifest("0.5.2", "\"crates/*\"");
        let cases: Vec<Row> = vec![
            (
                "a crate file",
                vec!["crates/x/src/lib.rs"],
                None,
                Some(X_AND_DEPENDENTS),
            ),
            (
                "a longer sibling name is a different crate",
                vec!["crates/mcrs_minecraft_light_color/src/lib.rs"],
                None,
                Some(&["mcrs_minecraft_light_color"]),
            ),
            (
                "the shorter sibling brings its dependent",
                vec!["crates/mcrs_minecraft_light/src/lib.rs"],
                None,
                Some(&["mcrs_minecraft_light", "mcrs_minecraft_light_color"]),
            ),
            (
                "root sources",
                vec!["src/main.rs", "tests/it/main.rs"],
                None,
                Some(&["mcrs"]),
            ),
            (
                "the runner maps to itself",
                vec!["xtask/src/main.rs"],
                None,
                Some(&["xtask"]),
            ),
            (
                "a crate's Markdown",
                vec!["crates/x/README.md"],
                None,
                Some(X_AND_DEPENDENTS),
            ),
            (
                "inert paths",
                vec!["README.md", "docs/a.md", "LICENSE-MIT"],
                None,
                Some(&[]),
            ),
            ("the root manifest", vec!["Cargo.toml"], None, None),
            ("the lock file", vec!["Cargo.lock"], None, None),
            ("the toolchain", vec!["rust-toolchain.toml"], None, None),
            (
                "cargo configuration",
                vec![".cargo/config.toml"],
                None,
                None,
            ),
            (
                "nextest configuration",
                vec![".config/nextest.toml"],
                None,
                None,
            ),
            ("assets", vec!["assets/x.json"], None, None),
            ("tools", vec!["tools/x"], None, None),
            ("scripts", vec!["scripts/x.sh"], None, None),
            ("workflows", vec![".github/workflows/x.yml"], None, None),
            (
                "a deleted crate",
                vec!["crates/gone/src/lib.rs"],
                None,
                None,
            ),
            (
                "an unknown crate sharing a name prefix",
                vec!["crates/x_extra/src/lib.rs"],
                None,
                None,
            ),
            ("an empty list", vec![], None, None),
            (
                "a widening path among crates",
                vec!["crates/x/a.rs", "scripts/x.sh"],
                None,
                None,
            ),
            (
                "inert next to a crate",
                vec!["README.md", "crates/a/src/lib.rs"],
                None,
                Some(&["a", "mcrs"]),
            ),
            (
                "sorted and deduplicated",
                vec!["crates/b/src/lib.rs", "crates/a/x.rs", "crates/a/y.rs"],
                None,
                Some(&["a", "b", "mcrs"]),
            ),
            (
                "a version-only bump adds nothing",
                vec!["Cargo.toml", "Cargo.lock", "crates/x/src/lib.rs"],
                Some(texts(bump_manifest.clone(), bumped_lock("1.0.0", "0.5.2"))),
                Some(X_AND_DEPENDENTS),
            ),
            (
                "a member at a version that is not the workspace version widens",
                vec!["Cargo.toml", "Cargo.lock", "crates/x/src/lib.rs"],
                Some(texts(bump_manifest.clone(), bumped_lock("1.0.0", "0.5.9"))),
                None,
            ),
            (
                "a bump plus another package's lock change widens",
                vec!["Cargo.toml", "Cargo.lock", "crates/x/src/lib.rs"],
                Some(texts(bump_manifest, bumped_lock("1.0.1", "0.5.2"))),
                None,
            ),
            (
                "a manifest change that keeps the versions widens",
                vec!["Cargo.toml", "Cargo.lock", "crates/x/src/lib.rs"],
                Some(texts(
                    manifest("0.5.2", "\"crates/*\", \"xtask\""),
                    bumped_lock("1.0.0", "0.5.2"),
                )),
                None,
            ),
        ];
        for (name, paths, texts, expected) in cases {
            let paths: Vec<String> = paths.into_iter().map(str::to_owned).collect();
            let version_only = texts.is_some_and(|t| version_bump(&workspace, &t).is_some());
            let found = scope(&workspace, &paths, version_only).with_dependents(&workspace);
            let expected = expected.map_or(Scope::Workspace, |names| {
                Scope::Packages(names.iter().map(|n| (*n).to_owned()).collect())
            });
            assert_eq!(found, expected, "{name}");
        }
    }

    #[test]
    fn a_version_bump_reports_both_versions() {
        let workspace = fixture();
        let t = texts(
            manifest("0.5.2", "\"crates/*\""),
            bumped_lock("1.0.0", "0.5.2"),
        );
        assert_eq!(
            version_bump(&workspace, &t),
            Some(VersionBump {
                from: "0.5.1".to_owned(),
                to: "0.5.2".to_owned()
            })
        );
    }

    #[test]
    fn an_empty_list_with_rust_differences_is_an_error_naming_both_results() {
        let message = ensure_consistent(&[], true, "origin/main").unwrap_err();
        assert!(
            message.contains("listed no changed file") && message.contains("reports a difference"),
            "{message}"
        );
        assert_eq!(ensure_consistent(&[], false, "origin/main"), Ok(()));
        assert_eq!(
            ensure_consistent(&["a.rs".to_owned()], true, "origin/main"),
            Ok(())
        );
    }

    struct Repo(PathBuf);

    impl Repo {
        fn new(label: &str) -> Repo {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "xtask-{label}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Repo(dir)
        }

        fn git(&self, args: &[&str]) {
            let status = Command::new("git")
                .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
                .args([
                    "-c",
                    "commit.gpgsign=false",
                    "-c",
                    "protocol.file.allow=always",
                ])
                .args(args)
                .current_dir(&self.0)
                .env_remove("GIT_DIR")
                .env_remove("GIT_INDEX_FILE")
                .env_remove("GIT_WORK_TREE")
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?}");
        }

        fn write(&self, file: &str, text: &str) {
            let path = self.0.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }

        fn commit(&self, message: &str) {
            self.git(&["add", "--all"]);
            self.git(&["commit", "--quiet", "--allow-empty", "-m", message]);
        }
    }

    impl Drop for Repo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_missing_base_ref_is_named() {
        let repo = Repo::new("missing");
        repo.git(&["init", "--quiet", "-b", "main"]);
        repo.commit("first");
        let error = resolve_base(&repo.0, "origin/main").unwrap_err();
        assert_eq!(
            error,
            BaseError::MissingRef {
                base_ref: "origin/main".to_owned()
            }
        );
        assert!(error.to_string().contains("origin/main"));
    }

    #[test]
    fn unrelated_histories_have_no_merge_base_and_are_not_called_shallow() {
        let repo = Repo::new("unrelated");
        repo.git(&["init", "--quiet", "-b", "main"]);
        repo.commit("first");
        repo.git(&["checkout", "--quiet", "--orphan", "other"]);
        repo.commit("second");
        let error = resolve_base(&repo.0, "main").unwrap_err();
        assert_eq!(
            error,
            BaseError::NoMergeBase {
                base_ref: "main".to_owned(),
                shallow: false
            }
        );
        assert!(error.to_string().contains("unrelated"));
    }

    #[test]
    fn a_shallow_clone_without_the_common_commit_is_called_shallow() {
        let origin = Repo::new("origin");
        origin.git(&["init", "--quiet", "-b", "main"]);
        origin.commit("one");
        origin.git(&["branch", "feature"]);
        origin.commit("two");
        origin.git(&["checkout", "--quiet", "feature"]);
        origin.commit("feature work");

        let clone = Repo::new("clone");
        let url = format!("file://{}", origin.0.display());
        clone.git(&[
            "clone", "--quiet", "--depth", "1", "--branch", "feature", &url, ".",
        ]);
        clone.git(&[
            "fetch",
            "--quiet",
            "--depth",
            "1",
            "origin",
            "main:refs/remotes/origin/main",
        ]);
        let error = resolve_base(&clone.0, "origin/main").unwrap_err();
        assert_eq!(
            error,
            BaseError::NoMergeBase {
                base_ref: "origin/main".to_owned(),
                shallow: true
            }
        );
        assert!(error.to_string().contains("--unshallow"));
    }

    #[test]
    fn the_merge_base_of_a_branch_is_where_it_left_the_base() {
        let repo = Repo::new("merge-base");
        repo.git(&["init", "--quiet", "-b", "main"]);
        repo.commit("one");
        repo.git(&["checkout", "--quiet", "-b", "topic"]);
        repo.commit("topic");
        let base = resolve_base(&repo.0, "main").unwrap();
        let first = git::text(&repo.0, &["rev-parse", "main"]).unwrap();
        assert_eq!(base, first.trim());
    }

    #[test]
    fn each_mode_reads_its_own_side_of_the_diff_and_renames_list_both_paths() {
        let repo = Repo::new("paths");
        repo.git(&["init", "--quiet", "-b", "main"]);
        repo.write("crates/x/old name.rs", "fn a() {}\n");
        repo.write("kept.txt", "kept\n");
        repo.commit("base");
        repo.git(&["branch", "base"]);
        repo.git(&["mv", "crates/x/old name.rs", "crates/x/new.rs"]);
        repo.commit("rename");
        repo.write("kept.txt", "edited\n");

        let committed = changed_paths(&repo.0, "base", Mode::Committed).unwrap();
        assert_eq!(committed, ["crates/x/new.rs", "crates/x/old name.rs"]);
        let working = changed_paths(&repo.0, "base", Mode::Working).unwrap();
        assert_eq!(
            working,
            ["crates/x/new.rs", "crates/x/old name.rs", "kept.txt"]
        );
    }

    #[test]
    fn only_the_working_mode_lists_untracked_files_and_never_ignored_ones() {
        let repo = Repo::new("untracked");
        repo.git(&["init", "--quiet", "-b", "main"]);
        repo.write(".gitignore", "target/\n");
        repo.write("a.rs", "fn a() {}\n");
        repo.commit("base");
        repo.write("crates/x/src/new.rs", "fn n() {}\n");
        repo.write("target/built.rs", "fn b() {}\n");

        assert_eq!(
            changed_paths(&repo.0, "main", Mode::Committed).unwrap(),
            Vec::<String>::new()
        );
        assert_eq!(
            changed_paths(&repo.0, "main", Mode::Working).unwrap(),
            ["crates/x/src/new.rs"]
        );
    }

    #[test]
    fn a_run_on_the_base_itself_lists_nothing() {
        let repo = Repo::new("empty");
        repo.git(&["init", "--quiet", "-b", "main"]);
        repo.write("a.rs", "fn a() {}\n");
        repo.commit("base");
        assert_eq!(
            changed_paths(&repo.0, "main", Mode::Committed).unwrap(),
            Vec::<String>::new()
        );
        assert_eq!(
            changed_paths(&repo.0, "main", Mode::Working).unwrap(),
            Vec::<String>::new()
        );
    }
}
