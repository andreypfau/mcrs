use crate::git::{self, Failure};
use crate::metadata::Workspace;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

const ROOT_MANIFESTS: [&str; 2] = ["Cargo.toml", "Cargo.lock"];

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Head {
    Worktree,
    Commit(String),
}

impl Head {
    pub fn rev(&self) -> &str {
        match self {
            Head::Worktree => "HEAD",
            Head::Commit(sha) => sha,
        }
    }

    pub fn source(&self) -> Option<&str> {
        match self {
            Head::Worktree => None,
            Head::Commit(sha) => Some(sha),
        }
    }
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
                "the head and {base_ref} share no commit in this shallow history; fetch the full history with git fetch --unshallow origin"
            ),
            BaseError::NoMergeBase {
                base_ref,
                shallow: false,
            } => write!(
                f,
                "the head and {base_ref} share no commit because their histories are unrelated; set CHECKS_BASE_REF to a ref this branch descends from"
            ),
            BaseError::Git(message) => write!(
                f,
                "{message}; check that git is installed and this is a repository"
            ),
        }
    }
}

pub fn resolve_base(repo: &Path, base_ref: &str, head: &Head) -> Result<String, BaseError> {
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
    match git::text(repo, &["merge-base", head.rev(), base_ref]) {
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

fn diff_args<'a>(base: &'a str, head: &'a Head) -> Vec<&'a str> {
    match head {
        Head::Worktree => vec![base],
        Head::Commit(sha) => vec![base, sha],
    }
}

pub fn changed_paths(repo: &Path, base: &str, head: &Head) -> Result<Vec<String>, String> {
    let mut args = vec!["diff", "--name-only", "-z", "--no-renames"];
    args.extend(diff_args(base, head));
    let mut paths = null_separated(&git::run(repo, &args).map_err(|e| e.to_string())?);
    if matches!(head, Head::Worktree) {
        let untracked = git::run(repo, &["ls-files", "--others", "--exclude-standard", "-z"])
            .map_err(|e| e.to_string())?;
        paths.extend(null_separated(&untracked));
        paths.sort();
        paths.dedup();
    }
    let rust_differs = if paths.is_empty() {
        let mut quiet = diff_args(base, head);
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

pub fn manifest_texts(repo: &Path, base: &str, head: &Head) -> Option<ManifestTexts> {
    let at_base = |file: &str| git::text(repo, &["show", &format!("{base}:{file}")]).ok();
    let at_head = |file: &str| match head {
        Head::Worktree => std::fs::read_to_string(repo.join(file)).ok(),
        Head::Commit(sha) => git::text(repo, &["show", &format!("{sha}:{file}")]).ok(),
    };
    Some(ManifestTexts {
        base_toml: at_base("Cargo.toml")?,
        head_toml: at_head("Cargo.toml")?,
        base_lock: at_base("Cargo.lock")?,
        head_lock: at_head("Cargo.lock")?,
    })
}

pub fn manifest_version_only(base_toml: &str, head_toml: &str) -> Option<VersionBump> {
    let from = workspace_version(base_toml)?;
    let to = workspace_version(head_toml)?;
    (retarget_manifest(head_toml, &to, &from) == base_toml).then_some(VersionBump { from, to })
}

pub fn version_bump(workspace: &Workspace, texts: &ManifestTexts) -> Option<VersionBump> {
    let bump = manifest_version_only(&texts.base_toml, &texts.head_toml)?;
    let members: Vec<&str> = workspace.packages.iter().map(|p| p.name.as_str()).collect();
    let lock_matches =
        retarget_lock(&texts.head_lock, &bump.to, &bump.from, &members) == texts.base_lock;
    lock_matches.then_some(bump)
}

pub fn diff_names(repo: &Path, base: &str, head: &Head) -> Result<Vec<String>, String> {
    let mut args = vec!["diff", "--name-only", "-z", "--no-renames"];
    args.extend(diff_args(base, head));
    paths_of(&git::run(repo, &args).map_err(|e| e.to_string())?)
}

fn paths_of(listing: &[u8]) -> Result<Vec<String>, String> {
    listing
        .split(|&byte| byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| {
            String::from_utf8(path.to_vec()).map_err(|_| {
                format!(
                    "a path that is not valid UTF-8 was found ({}); rename it",
                    String::from_utf8_lossy(path)
                )
            })
        })
        .collect()
}

#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub path: String,
    pub from: Option<String>,
    pub added: Option<u64>,
    pub deleted: Option<u64>,
}

pub fn numstat(repo: &Path, base: &str, head: &Head) -> Result<Vec<Entry>, String> {
    let mut args = vec![
        "diff",
        "--numstat",
        "-z",
        "-M",
        "--no-ext-diff",
        "--no-textconv",
        "--no-color",
    ];
    args.extend(diff_args(base, head));
    let listing = git::run(repo, &args).map_err(|e| e.to_string())?;
    let mut fields = listing.split(|&byte| byte == 0);
    let mut entries = Vec::new();
    let utf8 = |bytes: &[u8]| {
        String::from_utf8(bytes.to_vec())
            .map_err(|_| "a path that is not valid UTF-8 was found; rename it".to_owned())
    };
    while let Some(field) = fields.next() {
        if field.is_empty() {
            continue;
        }
        let record = utf8(field)?;
        let mut parts = record.splitn(3, '\t');
        let (Some(added), Some(deleted), Some(path)) = (parts.next(), parts.next(), parts.next())
        else {
            return Err(format!(
                "git diff --numstat printed an unreadable record: {record}"
            ));
        };
        let count = |text: &str| match text {
            "-" => Ok(None),
            digits => digits
                .parse::<u64>()
                .map(Some)
                .map_err(|_| format!("git diff --numstat printed a bad count {text:?}")),
        };
        let (added, deleted) = (count(added)?, count(deleted)?);
        let (from, path) = if path.is_empty() {
            let (Some(old), Some(new)) = (fields.next(), fields.next()) else {
                return Err("git diff --numstat ended in the middle of a rename".to_owned());
            };
            (Some(utf8(old)?), utf8(new)?)
        } else {
            (None, path.to_owned())
        };
        entries.push(Entry {
            path,
            from,
            added,
            deleted,
        });
    }
    Ok(entries)
}

pub fn attributes(
    repo: &Path,
    source: Option<&str>,
    paths: &[String],
) -> Result<BTreeMap<String, String>, String> {
    if paths.is_empty() {
        return Ok(BTreeMap::new());
    }
    let source = source.map(|rev| format!("--source={rev}"));
    let mut args = vec![
        "-c",
        "core.attributesFile=/dev/null",
        "check-attr",
        "-z",
        "--stdin",
    ];
    args.extend(source.as_deref());
    args.push("linguist-generated");
    let mut input = Vec::new();
    for path in paths {
        input.extend_from_slice(path.as_bytes());
        input.push(0);
    }
    let reply = git::pipe(repo, &args, &input).map_err(|e| e.to_string())?;
    let fields = paths_of(&reply)?;
    if fields.len() != paths.len() * 3 {
        return Err("git check-attr answered for a different number of paths".to_owned());
    }
    Ok(fields
        .chunks(3)
        .map(|triple| (triple[0].clone(), triple[2].clone()))
        .collect())
}

pub fn is_set(value: &str) -> bool {
    value == "set"
}

pub fn is_generated_for_review(value: &str) -> bool {
    !matches!(value, "unspecified" | "unset" | "false")
}

pub fn tracked(repo: &Path, head: &Head) -> Result<Vec<String>, String> {
    let listing = match head {
        Head::Worktree => git::run(repo, &["ls-files", "-z"]),
        Head::Commit(sha) => git::run(repo, &["ls-tree", "-r", "-z", "--name-only", sha]),
    };
    paths_of(&listing.map_err(|e| e.to_string())?)
}

pub fn blobs(repo: &Path, specs: &[String]) -> Result<Vec<Option<Vec<u8>>>, String> {
    if specs.is_empty() {
        return Ok(Vec::new());
    }
    if let Some(spec) = specs.iter().find(|spec| spec.contains('\n')) {
        return Err(format!(
            "the path in {spec:?} contains a line break; rename it"
        ));
    }
    let mut input = specs.join("\n").into_bytes();
    input.push(b'\n');
    let reply = git::pipe(repo, &["cat-file", "--batch"], &input).map_err(|e| e.to_string())?;
    let mut rest = reply.as_slice();
    let mut found = Vec::with_capacity(specs.len());
    for spec in specs {
        let end = rest
            .iter()
            .position(|&byte| byte == b'\n')
            .ok_or_else(|| format!("git cat-file stopped answering at {spec}"))?;
        let header = String::from_utf8_lossy(&rest[..end]).into_owned();
        rest = &rest[end + 1..];
        if header.ends_with(" missing") {
            found.push(None);
            continue;
        }
        let mut words = header.rsplitn(3, ' ');
        let size = words.next().and_then(|size| size.parse::<usize>().ok());
        let kind = words.next();
        let (Some(size), Some("blob")) = (size, kind) else {
            return Err(format!("{spec} is not a file in git"));
        };
        if rest.len() < size + 1 {
            return Err(format!("git cat-file cut {spec} short"));
        }
        found.push(Some(rest[..size].to_vec()));
        rest = &rest[size + 1..];
    }
    Ok(found)
}

pub fn read_at(repo: &Path, rev: &str, path: &str) -> Result<Option<String>, String> {
    let mut found = blobs(repo, &[format!("{rev}:{path}")])?;
    Ok(found
        .pop()
        .flatten()
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned()))
}

pub fn head_text(repo: &Path, head: &Head, path: &str) -> Result<Option<String>, String> {
    let Head::Worktree = head else {
        return read_at(repo, head.rev(), path);
    };
    let literal = format!(":(literal){path}");
    let listed = git::run(repo, &["ls-files", "-z", "--", &literal]).map_err(|e| e.to_string())?;
    if listed.is_empty() {
        return Ok(None);
    }
    match std::fs::read(repo.join(path)) {
        Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("could not read {path}: {e}")),
    }
}

pub fn line_count(bytes: &[u8]) -> u64 {
    let breaks = bytes.iter().filter(|&&byte| byte == b'\n').count() as u64;
    breaks + u64::from(!bytes.is_empty() && !bytes.ends_with(b"\n"))
}

pub fn line_counts(repo: &Path, head: &Head, paths: &[String]) -> Result<Vec<u64>, String> {
    match head {
        Head::Worktree => paths
            .iter()
            .map(|path| {
                std::fs::read(repo.join(path))
                    .map(|bytes| line_count(&bytes))
                    .map_err(|e| format!("could not read {path}: {e}"))
            })
            .collect(),
        Head::Commit(sha) => {
            let specs: Vec<String> = paths.iter().map(|path| format!("{sha}:{path}")).collect();
            blobs(repo, &specs)?
                .into_iter()
                .zip(paths)
                .map(|(blob, path)| {
                    blob.map(|bytes| line_count(&bytes))
                        .ok_or_else(|| format!("{path} is missing from the head"))
                })
                .collect()
        }
    }
}

pub enum Found {
    AtBase(String),
    Restored { restored: String, last: String },
}

pub fn configuration_at(
    repo: &Path,
    base: &str,
    head: &Head,
    file: &str,
) -> Result<Option<Found>, String> {
    if let Some(text) = read_at(repo, base, file)? {
        return Ok(Some(Found::AtBase(text)));
    }
    if is_shallow(repo)? {
        return Err(format!(
            "cannot tell from a shallow history whether {file} existed; fetch the full history"
        ));
    }
    let removing =
        git::text(repo, &["rev-list", "-1", base, "--", file]).map_err(|e| e.to_string())?;
    let removing = removing.trim();
    if removing.is_empty() {
        return Ok(None);
    }
    let restored = head_text(repo, head, file)?
        .ok_or_else(|| format!("{file} was removed from the base; restore it"))?;
    let last = read_at(repo, &format!("{removing}~1"), file)?
        .ok_or_else(|| format!("cannot read the last version of {file} before it was removed"))?;
    Ok(Some(Found::Restored { restored, last }))
}

pub fn is_shallow(repo: &Path) -> Result<bool, String> {
    git::text(repo, &["rev-parse", "--is-shallow-repository"])
        .map(|answer| answer.trim() == "true")
        .map_err(|e| e.to_string())
}

pub fn short(sha: &str) -> &str {
    sha.get(..9).unwrap_or(sha)
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
pub(crate) mod tests {
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

    pub(crate) struct Repo(pub(crate) PathBuf);

    impl Repo {
        pub(crate) fn new(label: &str) -> Repo {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "xtask-{label}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Repo(dir)
        }

        fn command(&self, args: &[&str]) -> Command {
            let mut command = Command::new("git");
            command
                .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
                .args([
                    "-c",
                    "commit.gpgsign=false",
                    "-c",
                    "protocol.file.allow=always",
                ])
                .args(args)
                .current_dir(&self.0);
            for (name, _) in std::env::vars_os() {
                if name.to_string_lossy().starts_with("GIT_") {
                    command.env_remove(name);
                }
            }
            command
        }

        pub(crate) fn git(&self, args: &[&str]) {
            let status = self.command(args).status().unwrap();
            assert!(status.success(), "git {args:?}");
        }

        pub(crate) fn out(&self, args: &[&str]) -> String {
            let output = self.command(args).output().unwrap();
            assert!(output.status.success(), "git {args:?}");
            String::from_utf8(output.stdout).unwrap().trim().to_owned()
        }

        pub(crate) fn init(&self) {
            self.git(&["init", "--quiet", "-b", "main"]);
        }

        pub(crate) fn write(&self, file: &str, text: &str) {
            self.write_bytes(file, text.as_bytes());
        }

        pub(crate) fn write_bytes(&self, file: &str, bytes: &[u8]) {
            let path = self.0.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }

        pub(crate) fn remove(&self, file: &str) {
            std::fs::remove_file(self.0.join(file)).unwrap();
        }

        pub(crate) fn rename(&self, from: &str, to: &str) {
            let target = self.0.join(to);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::rename(self.0.join(from), target).unwrap();
        }

        pub(crate) fn commit(&self, message: &str) {
            self.git(&["add", "--all"]);
            self.git(&["commit", "--quiet", "--allow-empty", "-m", message]);
        }

        pub(crate) fn sha(&self) -> String {
            self.out(&["rev-parse", "HEAD"])
        }

        pub(crate) fn status(&self) -> String {
            self.out(&["status", "--porcelain"])
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
        let error = resolve_base(&repo.0, "origin/main", &Head::Worktree).unwrap_err();
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
        let error = resolve_base(&repo.0, "main", &Head::Worktree).unwrap_err();
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
        let error = resolve_base(&clone.0, "origin/main", &Head::Worktree).unwrap_err();
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
        let base = resolve_base(&repo.0, "main", &Head::Worktree).unwrap();
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

        let tip = Head::Commit(repo.sha());
        let committed = changed_paths(&repo.0, "base", &tip).unwrap();
        assert_eq!(committed, ["crates/x/new.rs", "crates/x/old name.rs"]);
        let working = changed_paths(&repo.0, "base", &Head::Worktree).unwrap();
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

        let tip = Head::Commit(repo.sha());
        assert_eq!(
            changed_paths(&repo.0, "main", &tip).unwrap(),
            Vec::<String>::new()
        );
        assert_eq!(
            changed_paths(&repo.0, "main", &Head::Worktree).unwrap(),
            ["crates/x/src/new.rs"]
        );
    }

    #[test]
    fn a_run_on_the_base_itself_lists_nothing() {
        let repo = Repo::new("empty");
        repo.git(&["init", "--quiet", "-b", "main"]);
        repo.write("a.rs", "fn a() {}\n");
        repo.commit("base");
        let tip = Head::Commit(repo.sha());
        assert_eq!(
            changed_paths(&repo.0, "main", &tip).unwrap(),
            Vec::<String>::new()
        );
        assert_eq!(
            changed_paths(&repo.0, "main", &Head::Worktree).unwrap(),
            Vec::<String>::new()
        );
    }
}
