use crate::git;
use crate::scope::{self, Head};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const CRATE: &str = "mutants";
const SKIP: &str = "skip";
const EXCLUDE: &str = "exclude_re";
const IN_PLACE_MARKER: [&str; 3] = ["/* ~ changed by ", "cargo-mutants", " ~ */"];

fn listing(repo: &Path, args: &[&str], strip: Option<&str>) -> Result<Vec<String>, String> {
    let found = git::output(repo, args).map_err(|e| e.to_string())?;
    if !matches!(found.status, Some(0 | 1)) || !found.stderr.is_empty() {
        return Err(format!(
            "git grep could not search every file (status {:?}): {}",
            found.status, found.stderr
        ));
    }
    found
        .stdout
        .split(|&byte| byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| {
            let path = std::str::from_utf8(path)
                .map_err(|_| "a path that is not valid UTF-8 was found; rename it".to_owned())?;
            match strip {
                Some(prefix) => path
                    .strip_prefix(prefix)
                    .map(str::to_owned)
                    .ok_or_else(|| format!("git grep printed an unexpected path {path}")),
                None => Ok(path.to_owned()),
            }
        })
        .collect()
}

fn pending_whitespace(rest: &[u8]) -> usize {
    match rest {
        [0x09..=0x0D | 0x20, ..] => 1,
        [0xC2, 0x85, ..] => 2,
        [0xE2, 0x80, 0x8E | 0x8F | 0xA8 | 0xA9, ..] => 3,
        _ => 0,
    }
}

fn skip_trivia(bytes: &[u8], mut at: usize) -> usize {
    loop {
        let rest = &bytes[at..];
        let space = pending_whitespace(rest);
        if space > 0 {
            at += space;
        } else if rest.starts_with(b"//") {
            at += rest
                .iter()
                .position(|&byte| byte == b'\n')
                .unwrap_or(rest.len());
        } else if rest.starts_with(b"/*") {
            let mut depth = 0usize;
            let mut cursor = 0;
            while cursor < rest.len() {
                if rest[cursor..].starts_with(b"/*") {
                    depth += 1;
                    cursor += 2;
                } else if rest[cursor..].starts_with(b"*/") {
                    depth -= 1;
                    cursor += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    cursor += 1;
                }
            }
            at += cursor;
        } else {
            return at;
        }
    }
}

fn attribute_lines(bytes: &[u8]) -> Vec<usize> {
    let mut lines = Vec::new();
    let mut from = 0;
    while let Some(offset) = bytes[from..]
        .windows(CRATE.len())
        .position(|window| window == CRATE.as_bytes())
    {
        let start = from + offset;
        from = start + 1;
        let mut at = skip_trivia(bytes, start + CRATE.len());
        if !bytes[at..].starts_with(b"::") {
            continue;
        }
        at = skip_trivia(bytes, at + 2);
        if bytes[at..].starts_with(b"r#") {
            at += 2;
        }
        let named = [SKIP, EXCLUDE].into_iter().any(|name| {
            bytes[at..].starts_with(name.as_bytes())
                && !bytes
                    .get(at + name.len())
                    .is_some_and(|next| next.is_ascii_alphanumeric() || *next == b'_')
        });
        if named {
            lines.push(1 + bytes[..start].iter().filter(|&&byte| byte == b'\n').count());
        }
    }
    lines
}

fn from_working_tree(repo: &Path, paths: &[String]) -> Result<Vec<(String, Vec<u8>)>, String> {
    paths
        .iter()
        .map(|path| {
            std::fs::read(repo.join(path))
                .map(|bytes| (path.clone(), bytes))
                .map_err(|e| format!("could not read {path}: {e}; the scan cannot vouch for a file it cannot read"))
        })
        .collect()
}

fn from_git(repo: &Path, prefix: &str, paths: &[String]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let specs: Vec<String> = paths.iter().map(|path| format!("{prefix}{path}")).collect();
    scope::blobs(repo, &specs)?
        .into_iter()
        .zip(paths)
        .map(|(blob, path)| {
            blob.map(|bytes| (path.clone(), bytes)).ok_or_else(|| {
                format!("could not read {path}; the scan cannot vouch for a file it cannot read")
            })
        })
        .collect()
}

fn grep_args<'a>(pattern: &'a str, extra: &[&'a str], rev: Option<&'a str>) -> Vec<&'a str> {
    let mut args = vec!["grep"];
    args.extend(extra);
    args.extend(["-a", "-l", "-z", "-F", "-e", pattern]);
    args.extend(rev);
    args
}

fn candidates(
    repo: &Path,
    head: &Head,
    pattern: &str,
) -> Result<(Vec<String>, Vec<String>), String> {
    match head {
        Head::Worktree => Ok((
            listing(repo, &grep_args(pattern, &[], None), None)?,
            listing(repo, &grep_args(pattern, &["--cached"], None), None)?,
        )),
        Head::Commit(sha) => {
            let found = listing(
                repo,
                &grep_args(pattern, &[], Some(sha)),
                Some(&format!("{sha}:")),
            )?;
            Ok((found, Vec::new()))
        }
    }
}

fn attribute_problem(repo: &Path, head: &Head) -> Result<Option<String>, String> {
    let (working, staged) = candidates(repo, head, CRATE)?;
    let files = match head {
        Head::Worktree => {
            let mut files = from_working_tree(repo, &working)?;
            files.extend(from_git(repo, ":", &staged)?);
            files
        }
        Head::Commit(sha) => from_git(repo, &format!("{sha}:"), &working)?,
    };
    let hits: BTreeSet<(&str, usize)> = files
        .iter()
        .flat_map(|(path, bytes)| {
            attribute_lines(bytes)
                .into_iter()
                .map(move |line| (path.as_str(), line))
        })
        .collect();
    if hits.is_empty() {
        return Ok(None);
    }
    let places: Vec<String> = hits
        .into_iter()
        .map(|(path, line)| format!("{path}:{line}"))
        .collect();
    Ok(Some(format!(
        "the mutation-testing skip attribute appears at {}; remove it, because mutation exclusions belong in the mutants configuration",
        places.join(", ")
    )))
}

fn in_place_problem(repo: &Path, head: &Head) -> Result<Option<String>, String> {
    let (working, staged) = candidates(repo, head, &IN_PLACE_MARKER.concat())?;
    let paths: BTreeSet<String> = working.into_iter().chain(staged).collect();
    if paths.is_empty() {
        return Ok(None);
    }
    Ok(Some(format!(
        "cargo-mutants' in-place marker comment appears in {}; a mutation run was interrupted while a mutant was applied, so restore the files with git",
        paths.into_iter().collect::<Vec<_>>().join(", ")
    )))
}

type Dependencies = BTreeMap<String, Dependency>;

#[derive(Deserialize)]
#[serde(untagged)]
enum Dependency {
    Version(String),
    Detailed(Detail),
}

#[derive(Deserialize)]
struct Detail {
    package: Option<String>,
}

#[derive(Default, Deserialize)]
struct Tables {
    #[serde(default)]
    dependencies: Dependencies,
    #[serde(default, rename = "dev-dependencies", alias = "dev_dependencies")]
    dev: Dependencies,
    #[serde(default, rename = "build-dependencies", alias = "build_dependencies")]
    build: Dependencies,
}

#[derive(Deserialize)]
struct Manifest {
    #[serde(flatten)]
    own: Tables,
    #[serde(default)]
    workspace: WorkspaceTable,
    #[serde(default)]
    target: BTreeMap<String, Tables>,
}

#[derive(Default, Deserialize)]
struct WorkspaceTable {
    #[serde(default)]
    dependencies: Dependencies,
}

impl Tables {
    fn names_the_crate(&self) -> bool {
        [&self.dependencies, &self.dev, &self.build]
            .into_iter()
            .any(names_the_crate)
    }
}

fn renamed_from(dependency: &Dependency) -> Option<&str> {
    match dependency {
        Dependency::Detailed(detail) => detail.package.as_deref(),
        Dependency::Version(_version) => None,
    }
}

fn names_the_crate(dependencies: &Dependencies) -> bool {
    dependencies
        .iter()
        .any(|(name, dependency)| name == CRATE || renamed_from(dependency) == Some(CRATE))
}

fn dependency_problem(repo: &Path, head: &Head) -> Result<Option<String>, String> {
    let is_manifest = |path: &String| path == "Cargo.toml" || path.ends_with("/Cargo.toml");
    let manifests: Vec<String> = scope::tracked(repo, head)?
        .into_iter()
        .filter(is_manifest)
        .collect();
    let files = match head {
        Head::Worktree => {
            let mut files = from_git(repo, ":", &manifests)?;
            let on_disk: Vec<String> = manifests
                .iter()
                .filter(|path| repo.join(path).exists())
                .cloned()
                .collect();
            files.extend(from_working_tree(repo, &on_disk)?);
            files
        }
        Head::Commit(sha) => from_git(repo, &format!("{sha}:"), &manifests)?,
    };
    let mut named = BTreeSet::new();
    for (path, bytes) in &files {
        let text = String::from_utf8_lossy(bytes);
        let manifest: Manifest = toml::from_str(&text)
            .map_err(|e| format!("{path} is not a manifest cargo accepts: {}", e.message()))?;
        if manifest.own.names_the_crate()
            || names_the_crate(&manifest.workspace.dependencies)
            || manifest.target.values().any(Tables::names_the_crate)
        {
            named.insert(path.as_str());
        }
    }
    if named.is_empty() {
        return Ok(None);
    }
    Ok(Some(format!(
        "a dependency on the {CRATE} crate, under its own name or renamed, appears in {}; remove it, because mutation exclusions belong in the mutants configuration",
        named.into_iter().collect::<Vec<_>>().join(", ")
    )))
}

pub fn check(repo: &Path, head: &Head) -> Result<String, String> {
    let problems: Vec<String> = [
        attribute_problem(repo, head)?,
        in_place_problem(repo, head)?,
        dependency_problem(repo, head)?,
    ]
    .into_iter()
    .flatten()
    .collect();
    if problems.is_empty() {
        Ok(String::new())
    } else {
        Err(problems.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scope::tests::{Repo, judge_rows_in_parallel};
    use std::os::unix::fs::PermissionsExt;

    const SEP: &str = "::";

    fn form(template: &str) -> Vec<u8> {
        template
            .replace("{M}", CRATE)
            .replace("{S}", SKIP)
            .replace("{X}", EXCLUDE)
            .replace("{::}", SEP)
            .into_bytes()
    }

    const PREFIX: &str = "fn f() {}\n";

    fn forms() -> Vec<(&'static str, &'static str, String)> {
        let at_line_two = |body: &str| format!("{PREFIX}{body}");
        vec![
            (
                "the plain attribute",
                "src/a.rs",
                at_line_two("#[{M}{::}{S}]\nfn g() {}\n"),
            ),
            (
                "the spaced form",
                "src/a.rs",
                at_line_two("#[ {M} {::} {S} ]\n"),
            ),
            (
                "the cfg_attr form",
                "src/a.rs",
                at_line_two("#[cfg_attr(test, {M}{::}{S})]\n"),
            ),
            (
                "the inner attribute",
                "src/a.rs",
                at_line_two("#![{M}{::}{S}]\n"),
            ),
            (
                "the exclude_re form",
                "src/a.rs",
                at_line_two("#[{M}{::}{X}(\"x\")]\n"),
            ),
            (
                "a form split across lines",
                "src/a.rs",
                at_line_two("#[{M}\n{::}\n{S}]\n"),
            ),
            (
                "a block comment between the tokens",
                "src/a.rs",
                at_line_two("#[{M}/* c */{::}/* c */{S}]\n"),
            ),
            (
                "a nested block comment between the tokens",
                "src/a.rs",
                at_line_two("#[{M}/* a /* b */ c */{::}{S}]\n"),
            ),
            (
                "a line comment between the tokens",
                "src/a.rs",
                at_line_two("#[{M} // c\n{::} // d\n{S}]\n"),
            ),
            (
                "a leading path separator",
                "src/a.rs",
                at_line_two("#[{::}{M}{::}{S}]\n"),
            ),
            (
                "carriage return line endings",
                "src/a.rs",
                "fn f() {}\r\n#[{M}\r\n{::}\r\n{S}]\r\n".to_owned(),
            ),
            (
                "a NUL byte in a comment before the attribute",
                "src/a.rs",
                "// a\0b\n#[{M}{::}{S}]\n".to_owned(),
            ),
            (
                "a left-to-right mark around the separator",
                "src/a.rs",
                at_line_two("#[{M}\u{200E}{::}\u{200E}{S}]\n"),
            ),
            (
                "a next-line character around the separator",
                "src/a.rs",
                at_line_two("#[{M}\u{0085}{::}\u{0085}{S}]\n"),
            ),
            (
                "a vertical tab around the separator",
                "src/a.rs",
                at_line_two("#[{M}\u{000B}{::}\u{000B}{S}]\n"),
            ),
            (
                "a raw identifier",
                "src/a.rs",
                at_line_two("#[{M}{::}r#{S}]\n"),
            ),
            (
                "a file name with a space and a letter outside ASCII",
                "src/é file.rs",
                at_line_two("#[{M}{::}{S}]\n"),
            ),
        ]
    }

    #[derive(Clone, Copy, Debug)]
    enum Place {
        Commit,
        Staged,
        WorkingTreeOnly,
        IndexOnly,
    }

    fn setup(label: &str, place: Place, files: &[(&str, Vec<u8>)]) -> (Repo, Head) {
        let repo = Repo::new(label);
        repo.init();
        repo.write("README.md", "readme\n");
        let clean: Vec<(&str, Vec<u8>)> = files
            .iter()
            .map(|(path, _)| (*path, b"fn clean() {}\n".to_vec()))
            .collect();
        match place {
            Place::Commit => {
                repo.commit("base");
                repo.git(&["checkout", "--quiet", "-b", "topic"]);
                for (path, bytes) in files {
                    repo.write_bytes(path, bytes);
                }
                repo.commit("topic");
                let head = Head::Commit(repo.sha());
                repo.git(&["checkout", "--quiet", "main"]);
                (repo, head)
            }
            Place::Staged => {
                repo.commit("base");
                for (path, bytes) in files {
                    repo.write_bytes(path, bytes);
                }
                repo.git(&["add", "--all"]);
                (repo, Head::Worktree)
            }
            Place::WorkingTreeOnly => {
                for (path, bytes) in &clean {
                    repo.write_bytes(path, bytes);
                }
                repo.commit("base");
                for (path, bytes) in files {
                    repo.write_bytes(path, bytes);
                }
                (repo, Head::Worktree)
            }
            Place::IndexOnly => {
                repo.commit("base");
                for (path, bytes) in files {
                    repo.write_bytes(path, bytes);
                }
                repo.git(&["add", "--all"]);
                for (path, _) in files {
                    repo.remove(path);
                }
                (repo, Head::Worktree)
            }
        }
    }

    struct Case {
        name: &'static str,
        path: &'static str,
        bytes: Vec<u8>,
        place: Place,
    }

    fn cases(rows: Vec<(&'static str, &'static str, Vec<u8>)>, places: &[Place]) -> Vec<Case> {
        rows.into_iter()
            .flat_map(|(name, path, bytes)| {
                places.iter().map(move |&place| Case {
                    name,
                    path,
                    bytes: bytes.clone(),
                    place,
                })
            })
            .collect()
    }

    fn fail_naming(label: &str, cases: &[Case], needs: impl Fn(&Case) -> Vec<String> + Sync) {
        judge_rows_in_parallel(cases, |case| {
            let (repo, head) = setup(label, case.place, &[(case.path, case.bytes.clone())]);
            let (name, place) = (case.name, case.place);
            match check(&repo.0, &head) {
                Ok(found) => Some(format!("{name} ({place:?}): passed with {found:?}")),
                Err(message) => needs(case)
                    .iter()
                    .any(|part| !message.contains(part.as_str()))
                    .then(|| format!("{name} ({place:?}): {message}")),
            }
        });
    }

    #[test]
    fn the_mutation_marker_fails_in_every_form_and_place() {
        let forms = forms()
            .into_iter()
            .map(|(name, path, template)| (name, path, form(&template)))
            .collect();
        let mut all = cases(forms, &[Place::Commit, Place::Staged]);
        let bare = vec![(
            "the bare attribute",
            "src/a.rs",
            form("fn f() {}\n#[{M}{::}{S}]\n"),
        )];
        all.extend(cases(bare, &[Place::WorkingTreeOnly, Place::IndexOnly]));
        fail_naming("marker", &all, |case| vec![format!("{}:2", case.path)]);
    }

    #[test]
    fn a_candidate_that_cannot_be_read_fails_the_check_and_is_named() {
        let marker = form("fn f() {}\n#[{M}{::}{S}]\n");
        let (repo, head) = setup(
            "unreadable",
            Place::WorkingTreeOnly,
            &[("src/a.rs", marker)],
        );
        let file = repo.0.join("src/a.rs");
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000)).unwrap();
        let found = check(&repo.0, &head);
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        let message = found.unwrap_err();
        assert!(message.contains("src/a.rs"), "{message}");
    }

    #[test]
    fn a_tree_without_the_attribute_passes_even_beside_its_parts() {
        let harmless = form(
            "use {M};\nlet {M} = 1; // {S} {::}\nlet a = {M}{::}other();\nlet b = {M}{::}{S}ped();\n",
        );
        for place in [Place::Commit, Place::Staged] {
            let (repo, head) = setup("clean", place, &[("src/ok.rs", harmless.clone())]);
            assert_eq!(check(&repo.0, &head), Ok(String::new()), "{place:?}");
        }
    }

    #[test]
    fn a_failure_lists_each_match_as_path_and_line_sorted_and_without_repeats() {
        let first = form("x\ny\n#[{M}{::}{S}]\n");
        let second = form("#[{M}{::}{S}]\n#[{M}{::}{X}(\"x\")] #[{M}{::}{S}]\n");
        let (repo, head) = setup(
            "sorted",
            Place::Commit,
            &[("src/b.rs", first), ("src/a.rs", second)],
        );
        let message = check(&repo.0, &head).unwrap_err();
        assert!(
            message.contains("src/a.rs:1, src/a.rs:2, src/b.rs:3;"),
            "{message}"
        );
    }

    #[test]
    fn a_head_that_is_not_in_the_repository_fails_closed() {
        let repo = Repo::new("missing");
        repo.init();
        repo.write("a.txt", "a\n");
        repo.commit("one");
        let message = check(&repo.0, &Head::Commit("0".repeat(40))).unwrap_err();
        assert!(message.contains("git grep"), "{message}");
    }

    const MARKER_PARTS: [&str; 3] = ["/* ~ changed by ", "cargo-mutants", " ~ */"];

    fn marked(template: &str) -> Vec<u8> {
        template
            .replace("{MARK}", &MARKER_PARTS.concat())
            .into_bytes()
    }

    #[test]
    fn the_in_place_marker_comment_fails_in_every_file_kind() {
        let rows: [(&str, &str, &str); 4] = [
            ("a Rust file", "src/a.rs", "fn f() -> u8 { 0 {MARK} }\n"),
            (
                "a tracked file of another kind",
                "notes/log.txt",
                "{MARK}\n",
            ),
            ("a file with a NUL byte", "src/b.rs", "// a\0b\n{MARK}\n"),
            ("a manifest", "crates/x/Cargo.toml", "# {MARK}\n"),
        ];
        let rows = rows
            .into_iter()
            .map(|(name, path, template)| (name, path, marked(template)))
            .collect();
        fail_naming(
            "in-place",
            &cases(rows, &[Place::Commit, Place::Staged]),
            |case| vec![case.path.to_owned(), "in-place".to_owned()],
        );
    }

    #[test]
    fn a_dependency_on_the_attribute_crate_fails_under_any_name_or_table() {
        let rows: [(&str, &str, &str); 5] = [
            (
                "a dev dependency",
                "crates/x/Cargo.toml",
                "[dev-dependencies]\nmutants = \"25\"\n",
            ),
            (
                "a workspace dependency",
                "Cargo.toml",
                "[workspace.dependencies]\nmutants = \"25\"\n",
            ),
            (
                "a target dependency",
                "crates/y/Cargo.toml",
                "[target.'cfg(unix)'.dependencies]\nmutants = \"25\"\n",
            ),
            (
                "a renamed dependency",
                "crates/z/Cargo.toml",
                "[dependencies]\nskipper = { package = \"mutants\", version = \"25\" }\n",
            ),
            (
                "a dependency written as a table",
                "crates/w/Cargo.toml",
                "[build-dependencies.mutants]\nversion = \"25\"\n",
            ),
        ];
        let rows = rows
            .into_iter()
            .map(|(name, path, text)| (name, path, text.as_bytes().to_vec()))
            .collect();
        fail_naming(
            "manifest",
            &cases(rows, &[Place::Commit, Place::Staged]),
            |case| vec![case.path.to_owned()],
        );
    }

    #[test]
    fn a_dependency_with_a_longer_name_and_a_clean_manifest_pass() {
        let manifest =
            b"[dependencies]\nmutants_helper = \"1\"\nserde = { workspace = true }\n".to_vec();
        for place in [Place::Commit, Place::Staged] {
            let (repo, head) = setup(
                "manifest-ok",
                place,
                &[("crates/x/Cargo.toml", manifest.clone())],
            );
            assert_eq!(check(&repo.0, &head), Ok(String::new()), "{place:?}");
        }
    }
}
