use crate::checks::Outcome;
use crate::metadata::{Kind, Workspace};
use crate::scope::{self, Found, Head};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

pub(crate) const LAYERS_FILE: &str = "quality/layers.toml";

const FIRST_LAYER: u8 = 1;
const LAST_LAYER: u8 = 7;

const FIX_UNMAPPED: &str = "give every workspace crate one entry in [crates] of quality/layers.toml and remove the entries of crates that do not exist";
const FIX_VIOLATION: &str = "depend only on crates in the same or a lower layer; a crate's layer and kind are set in quality/layers.toml, and an exception can only be removed";
const FIX_STALE: &str = "remove the stale exception from quality/layers.toml";
const FIX_ADDED: &str = "remove the exception; fix the dependency instead of listing it";

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
enum CrateKind {
    Data,
    Rules,
    Engine,
    Behaviour,
    Plugin,
    Render,
    App,
    Tool,
    Library,
}

impl fmt::Display for CrateKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            CrateKind::Data => "data",
            CrateKind::Rules => "rules",
            CrateKind::Engine => "engine",
            CrateKind::Behaviour => "behaviour",
            CrateKind::Plugin => "plugin",
            CrateKind::Render => "render",
            CrateKind::App => "app",
            CrateKind::Tool => "tool",
            CrateKind::Library => "library",
        })
    }
}

#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
struct Entry {
    layer: u8,
    kind: CrateKind,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(rename_all = "lowercase")]
enum Reach {
    Bevy,
    Tokio,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Exception {
    #[serde(rename = "crate")]
    krate: String,
    dependency: Option<String>,
    reaches: Option<Reach>,
    issue: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LayerFile {
    #[serde(default)]
    crates: BTreeMap<String, Entry>,
    #[serde(default)]
    exception: Vec<Exception>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Target {
    Dependency(String),
    Reach(Reach),
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Target::Dependency(name) => f.write_str(name),
            Target::Reach(Reach::Bevy) => f.write_str("bevy"),
            Target::Reach(Reach::Tokio) => f.write_str("tokio"),
        }
    }
}

type Key = (String, Target);

struct Parsed {
    crates: BTreeMap<String, Entry>,
    exceptions: BTreeSet<Key>,
}

fn is_issue(text: &str) -> bool {
    text.strip_prefix('#').is_some_and(|number| {
        !number.starts_with('0') && !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit())
    })
}

fn exception_key(exception: Exception) -> Result<Key, String> {
    let target = match (exception.dependency, exception.reaches) {
        (Some(dependency), None) => Target::Dependency(dependency),
        (None, Some(reach)) => Target::Reach(reach),
        _ => {
            return Err(format!(
                "the exception for {} names both or neither of `dependency` and `reaches`; give exactly one",
                exception.krate
            ));
        }
    };
    if !is_issue(&exception.issue) {
        return Err(format!(
            "the exception for {} `{target}` names issue `{}`; write the issue that removes it as #N",
            exception.krate, exception.issue
        ));
    }
    Ok((exception.krate, target))
}

fn parse(text: &str, revision: &str) -> Result<Parsed, String> {
    let prefix = format!("{LAYERS_FILE} {revision}");
    let file: LayerFile =
        toml::from_str(text).map_err(|e| format!("{prefix} is malformed: {}", e.message()))?;
    let outside = file
        .crates
        .iter()
        .find(|(_, entry)| !(FIRST_LAYER..=LAST_LAYER).contains(&entry.layer));
    if let Some((name, entry)) = outside {
        return Err(format!(
            "{prefix}: {name} is in layer {}; use a layer from {FIRST_LAYER} to {LAST_LAYER}",
            entry.layer
        ));
    }
    let mut exceptions = BTreeSet::new();
    for exception in file.exception {
        let key = exception_key(exception).map_err(|problem| format!("{prefix}: {problem}"))?;
        if exceptions.contains(&key) {
            return Err(format!(
                "{prefix}: the exception for {} `{}` is listed twice; remove one entry",
                key.0, key.1
            ));
        }
        exceptions.insert(key);
    }
    Ok(Parsed {
        crates: file.crates,
        exceptions,
    })
}

fn unmapped(crates: &BTreeMap<String, Entry>, workspace: &Workspace) -> BTreeSet<String> {
    let members: BTreeSet<&str> = workspace.packages.iter().map(|p| p.name.as_str()).collect();
    let missing = members
        .iter()
        .filter(|name| !crates.contains_key(**name))
        .map(|name| format!("{name} has no entry in {LAYERS_FILE}"));
    let unknown = crates
        .keys()
        .filter(|name| !members.contains(name.as_str()))
        .map(|name| format!("{LAYERS_FILE} names {name}, which is not a workspace crate"));
    missing.chain(unknown).collect()
}

fn describe(name: &str, entry: &Entry) -> String {
    format!("{name} (layer {}, {})", entry.layer, entry.kind)
}

fn violations(crates: &BTreeMap<String, Entry>, workspace: &Workspace) -> BTreeMap<Key, String> {
    let members: BTreeSet<&str> = workspace.packages.iter().map(|p| p.name.as_str()).collect();
    let mut found = BTreeMap::new();
    for package in &workspace.packages {
        let Some(from) = crates.get(&package.name) else {
            continue;
        };
        let normal = package.dependencies.iter().filter(|dep| {
            dep.kind == Kind::Normal
                && dep.name != package.name
                && members.contains(dep.name.as_str())
        });
        for dep in normal {
            let Some(to) = crates.get(&dep.name) else {
                continue;
            };
            if from.layer < to.layer {
                let line = format!(
                    "{} depends on {}",
                    describe(&package.name, from),
                    describe(&dep.name, to)
                );
                found.insert(
                    (package.name.clone(), Target::Dependency(dep.name.clone())),
                    line,
                );
            }
        }
    }
    found
}

fn base_exceptions(repo: &Path, base: &str, head: &Head) -> Result<Option<BTreeSet<Key>>, String> {
    let short = scope::short(base);
    let keys = match scope::configuration_at(repo, base, head, LAYERS_FILE)? {
        None => return Ok(None),
        Some(Found::AtBase(text)) => parse(&text, &format!("at the base {short}"))?,
        Some(Found::Restored { last, .. }) => {
            parse(&last, &format!("before its removal from the base {short}"))?
        }
    };
    Ok(Some(keys.exceptions))
}

#[derive(Default)]
struct Report {
    unmapped: BTreeSet<String>,
    violations: BTreeSet<String>,
    stale: BTreeSet<String>,
    added: BTreeSet<String>,
}

impl Report {
    fn failure(&self) -> Option<String> {
        let sets = [
            (&self.unmapped, FIX_UNMAPPED),
            (&self.violations, FIX_VIOLATION),
            (&self.stale, FIX_STALE),
            (&self.added, FIX_ADDED),
        ];
        let lines: BTreeSet<&String> = sets.iter().flat_map(|(set, _)| set.iter()).collect();
        if lines.is_empty() {
            return None;
        }
        let mut message = String::new();
        for line in lines {
            message.push_str(line);
            message.push('\n');
        }
        for (set, fix) in sets {
            if !set.is_empty() {
                message.push_str(&format!("fix: {fix}\n"));
            }
        }
        Some(message.trim_end().to_owned())
    }
}

pub(crate) fn check(repo: &Path, base: &str, head: &Head, workspace: &Workspace) -> Outcome {
    let text = scope::head_text(repo, head, LAYERS_FILE)?.ok_or_else(|| {
        format!(
            "{LAYERS_FILE} is missing at the head; map every workspace crate to a layer and a kind"
        )
    })?;
    let map = parse(&text, "at the head")?;
    let found = violations(&map.crates, workspace);
    let from_base = base_exceptions(repo, base, head)?;
    let in_force = from_base.is_none();
    let base_keys = from_base.unwrap_or_else(|| map.exceptions.clone());
    let applied: BTreeSet<&Key> = map.exceptions.intersection(&base_keys).collect();

    let mut report = Report {
        unmapped: unmapped(&map.crates, workspace),
        ..Report::default()
    };
    for (key, line) in &found {
        if !applied.contains(key) {
            report.violations.insert(line.clone());
        }
    }
    for key in &map.exceptions {
        if !found.contains_key(key) {
            report
                .stale
                .insert(format!("{}: stale exception for `{}`", key.0, key.1));
        }
        if !base_keys.contains(key) {
            report.added.insert(format!(
                "{}: the exception for `{}` is not on the base branch",
                key.0, key.1
            ));
        }
    }
    if let Some(message) = report.failure() {
        return Err(message);
    }
    let suffix = if in_force {
        format!("; the base has no {LAYERS_FILE}, so the head's exceptions are in force")
    } else {
        String::new()
    };
    Ok(format!(
        "{} crates; {} exceptions applied{suffix}",
        map.crates.len(),
        applied.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::capture;
    use crate::scope::tests::{Repo, judge_rows_in_parallel};

    type Edit = (String, Option<String>);

    enum Expect {
        Pass(&'static str),
        Fail(&'static [&'static str]),
    }

    struct Case {
        name: &'static str,
        base: Vec<Edit>,
        head: Vec<Edit>,
        expect: Expect,
    }

    const MEMBERS: [(&str, u8, &str, &str); 7] = [
        ("wire", 1, "data", ""),
        ("link", 2, "library", "wire"),
        ("values", 3, "data", "wire"),
        ("kernels", 3, "rules", "values"),
        ("engine", 4, "engine", "kernels"),
        ("behaviour", 5, "behaviour", "values"),
        ("plugin", 5, "plugin", "behaviour engine"),
    ];

    fn put(path: &str, text: &str) -> Edit {
        (path.to_owned(), Some(text.to_owned()))
    }

    fn paths(names: &str) -> String {
        names
            .split_whitespace()
            .map(|name| format!("{name} = {{ path = \"../{name}\" }}\n"))
            .collect()
    }

    fn member(name: &str, dependencies: &str, tail: &str) -> Edit {
        put(
            &format!("{name}/Cargo.toml"),
            &format!(
                "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n{dependencies}{tail}"
            ),
        )
    }

    fn root(extra: &[&str]) -> Edit {
        let names: Vec<String> = MEMBERS
            .iter()
            .map(|member| member.0)
            .chain(extra.iter().copied())
            .map(|name| format!("\"{name}\""))
            .collect();
        put(
            "Cargo.toml",
            &format!(
                "[workspace]\nmembers = [{}]\nresolver = \"3\"\n",
                names.join(", ")
            ),
        )
    }

    fn layer_file(extra: &str, exceptions: &[(&str, &str, &str)]) -> Edit {
        let mut text = String::from("[crates]\n");
        for (name, layer, kind, _) in MEMBERS {
            text.push_str(&format!(
                "{name} = {{ layer = {layer}, kind = \"{kind}\" }}\n"
            ));
        }
        text.push_str(extra);
        for (krate, dependency, issue) in exceptions {
            text.push_str(&format!(
                "\n[[exception]]\ncrate = \"{krate}\"\ndependency = \"{dependency}\"\nissue = \"{issue}\"\n"
            ));
        }
        put(LAYERS_FILE, &text)
    }

    fn standard() -> Vec<Edit> {
        let mut files = vec![root(&[]), layer_file("", &[])];
        for (name, _, _, dependencies) in MEMBERS {
            files.push(member(name, &paths(dependencies), ""));
            files.push(put(&format!("{name}/src/lib.rs"), ""));
        }
        files
    }

    fn apply(repo: &Repo, edits: &[Edit]) {
        for (path, text) in edits {
            match text {
                Some(text) => repo.write(path, text),
                None => repo.remove(path),
            }
        }
    }

    fn lock(repo: &Repo) {
        let out = capture(&repo.0, "cargo", &["generate-lockfile", "--offline"], &[]).unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn judge(case: &Case) -> Option<String> {
        let repo = Repo::new("layers");
        repo.init();
        apply(&repo, &standard());
        apply(&repo, &case.base);
        lock(&repo);
        repo.commit("base");
        let base = repo.sha();
        apply(&repo, &case.head);
        lock(&repo);
        repo.commit("head");
        let workspace = Workspace::load(&repo.0).unwrap();
        let found = check(&repo.0, &base, &Head::Commit(repo.sha()), &workspace);
        match (&case.expect, found) {
            (Expect::Pass(part), Ok(detail)) if detail.contains(part) => None,
            (Expect::Fail(parts), Err(message)) if parts.iter().all(|p| message.contains(p)) => {
                None
            }
            (_, other) => Some(format!("{}: {other:?}", case.name)),
        }
    }

    const LINK_ENGINE: (&str, &str, &str) = ("link", "engine", "#59");

    fn link_reaches_engine() -> Edit {
        member("link", &paths("wire engine"), "")
    }

    fn order_cases() -> Vec<Case> {
        vec![
            Case {
                name: "same-layer, downward and dev-dependency edges pass",
                base: vec![],
                head: vec![],
                expect: Expect::Pass("exceptions applied"),
            },
            Case {
                name: "an optional dependency up a layer fails",
                base: vec![],
                head: vec![member(
                    "link",
                    &format!(
                        "{}engine = {{ path = \"../engine\", optional = true }}\n",
                        paths("wire")
                    ),
                    "",
                )],
                expect: Expect::Fail(&[
                    "link (layer 2, library) depends on engine (layer 4, engine)",
                ]),
            },
            Case {
                name: "a crate with no entry and an entry with no crate fail",
                base: vec![],
                head: vec![
                    root(&["extra"]),
                    member("extra", "", ""),
                    put("extra/src/lib.rs", ""),
                    layer_file("gone = { layer = 1, kind = \"data\" }\n", &[]),
                ],
                expect: Expect::Fail(&["extra has no entry", "names gone"]),
            },
        ]
    }

    fn exception_cases() -> Vec<Case> {
        vec![
            Case {
                name: "an exception on the base and the head is applied",
                base: vec![layer_file("", &[LINK_ENGINE])],
                head: vec![layer_file("", &[LINK_ENGINE]), link_reaches_engine()],
                expect: Expect::Pass("1 exceptions applied"),
            },
            Case {
                name: "an exception added at the head fails",
                base: vec![],
                head: vec![layer_file("", &[LINK_ENGINE]), link_reaches_engine()],
                expect: Expect::Fail(&["is not on the base branch"]),
            },
            Case {
                name: "a stale exception fails",
                base: vec![layer_file("", &[LINK_ENGINE])],
                head: vec![layer_file("", &[LINK_ENGINE])],
                expect: Expect::Fail(&["stale exception for `engine`"]),
            },
            Case {
                name: "a base that never carried the file puts the head's exceptions in force",
                base: vec![(LAYERS_FILE.to_owned(), None)],
                head: vec![layer_file("", &[LINK_ENGINE]), link_reaches_engine()],
                expect: Expect::Pass("in force"),
            },
            Case {
                name: "an issue not written as #N fails",
                base: vec![],
                head: vec![layer_file("", &[("link", "engine", "59")])],
                expect: Expect::Fail(&["#N"]),
            },
        ]
    }

    #[test]
    fn the_check_judges_a_workspace_against_the_map_and_the_exceptions_at_its_base() {
        let mut cases = order_cases();
        cases.extend(exception_cases());
        judge_rows_in_parallel(&cases, judge);
    }
}
