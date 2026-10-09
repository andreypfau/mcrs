use crate::checks::capture;
use crate::metadata::{Package, Workspace};
use crate::scope::{self, Found, Head};
use serde::Deserialize;
use serde::de::IgnoredAny;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub(crate) const VERSION: &str = "1.14.0";
pub(crate) const EXEMPTIONS_FILE: &str = "quality/unused-deps.toml";

const DEPENDENCY_CODES: [&str; 5] = [
    "shear/unused_dependency",
    "shear/misplaced_dependency",
    "shear/unused_feature_dependency",
    "shear/unused_optional_dependency",
    "shear/misplaced_optional_dependency",
];

const FIX_FINDING: &str = "remove the dependency or move it to [dev-dependencies]; for a dependency that only a feature, a target or the crate's own tests need, first merge a pull request that adds only its entry, with its reason, to quality/unused-deps.toml, then add the dependency once that entry is on the base branch";
const FIX_STALE: &str = "remove the stale entry from quality/unused-deps.toml";

#[derive(Deserialize)]
struct Report {
    findings: Vec<Finding>,
}

#[derive(Deserialize)]
struct Finding {
    code: String,
    message: String,
    file: Option<String>,
}

type Pair = (String, String);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExemptionFile {
    #[serde(default)]
    exemption: Vec<Exemption>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Exemption {
    package: String,
    dependency: String,
    reason: String,
}

pub(crate) fn exemption_pairs(text: &str, revision: &str) -> Result<BTreeSet<Pair>, String> {
    let file: ExemptionFile = toml::from_str(text)
        .map_err(|e| format!("{EXEMPTIONS_FILE} {revision} is malformed: {}", e.message()))?;
    let mut pairs = BTreeSet::new();
    for entry in file.exemption {
        if entry.reason.trim().is_empty() {
            return Err(format!(
                "{EXEMPTIONS_FILE} {revision}: the entry for `{}` in {} has no reason; state why the dependency is needed",
                entry.dependency, entry.package
            ));
        }
        if !pairs.insert((entry.package.clone(), entry.dependency.clone())) {
            return Err(format!(
                "{EXEMPTIONS_FILE} {revision}: `{}` in {} is listed twice; remove one entry",
                entry.dependency, entry.package
            ));
        }
    }
    Ok(pairs)
}

#[derive(Debug, PartialEq, Eq)]
enum Subject {
    Dependency { package: String, name: String },
    WorkspaceEntry { name: String },
    Files { package: String, paths: Vec<String> },
    Unknown { code: String, message: String },
}

impl Subject {
    fn exemption_pair(&self) -> Option<Pair> {
        match self {
            Subject::Dependency { package, name } => Some((package.clone(), name.clone())),
            _ => None,
        }
    }
}

fn backticked(text: &str) -> Option<&str> {
    text.split('`').nth(1).filter(|name| !name.is_empty())
}

fn subject(finding: &Finding, owner: &dyn Fn(&str) -> Option<String>) -> Subject {
    let unknown = || Subject::Unknown {
        code: finding.code.clone(),
        message: finding.message.clone(),
    };
    let code = finding.code.as_str();
    if DEPENDENCY_CODES.contains(&code) {
        let package = finding.file.as_deref().and_then(owner);
        return match (package, backticked(&finding.message)) {
            (Some(package), Some(name)) => Subject::Dependency {
                package,
                name: name.to_owned(),
            },
            _ => unknown(),
        };
    }
    match code {
        "shear/unused_workspace_dependency" => backticked(&finding.message)
            .map(|name| Subject::WorkspaceEntry {
                name: name.to_owned(),
            })
            .unwrap_or_else(unknown),
        "shear/empty_files" | "shear/unlinked_files" => {
            let mut lines = finding.message.lines();
            let package = lines.next().and_then(backticked);
            match package {
                Some(package) => Subject::Files {
                    package: package.to_owned(),
                    paths: lines
                        .filter(|line| !line.is_empty())
                        .map(str::to_owned)
                        .collect(),
                },
                None => unknown(),
            }
        }
        _ => unknown(),
    }
}

fn first_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .to_owned()
}

fn judge(status: Option<i32>, stdout: &[u8], stderr: &[u8]) -> Result<Vec<Finding>, String> {
    let Some(status) = status else {
        return Err("cargo shear was stopped by a signal; run it again".to_owned());
    };
    let said = match first_line(stderr) {
        line if line.is_empty() => first_line(stdout),
        line => line,
    };
    match serde_json::from_slice::<Report>(stdout) {
        Ok(report) if status == 0 || status == 1 => {
            if status == 1 && report.findings.is_empty() {
                Err("cargo shear exited with status 1 but reported no finding".to_owned())
            } else {
                Ok(report.findings)
            }
        }
        Ok(_) => Err(format!("cargo shear exited with status {status}: {said}")),
        Err(_) => Err(format!(
            "cargo shear did not print a report (status {status}): {said}"
        )),
    }
}

fn version_matches(output: &str) -> Result<(), String> {
    if output.trim().strip_prefix("Version: ") == Some(VERSION) {
        return Ok(());
    }
    let found = first_line(output.as_bytes());
    let found = if found.is_empty() {
        "no answer"
    } else {
        &found
    };
    Err(format!(
        "cargo-shear {VERSION} is required, found {found}; run `cargo install --locked cargo-shear@{VERSION}` inside the repository"
    ))
}

fn require_version(repo: &Path, env: &[(&str, &str)]) -> Result<(), String> {
    let out = capture(repo, "cargo", &["shear", "--version"], env)?;
    let text = if out.status.success() {
        String::from_utf8_lossy(&out.stdout).into_owned()
    } else {
        String::from_utf8_lossy(&out.stderr).into_owned()
    };
    version_matches(&text)
}

fn run_shear(repo: &Path, env: &[(&str, &str)]) -> Result<Vec<Finding>, String> {
    let args = ["shear", "--locked", "--deny-warnings", "--format", "json"];
    let out = capture(repo, "cargo", &args, env)?;
    judge(out.status.code(), &out.stdout, &out.stderr)
}

fn manifest_of(package: &Package) -> String {
    if package.dir.is_empty() {
        "Cargo.toml".to_owned()
    } else {
        format!("{}/Cargo.toml", package.dir)
    }
}

#[derive(Deserialize, Default)]
struct Tables {
    #[serde(default)]
    dependencies: BTreeMap<String, IgnoredAny>,
    #[serde(default, rename = "dev-dependencies")]
    dev_dependencies: BTreeMap<String, IgnoredAny>,
    #[serde(default, rename = "build-dependencies")]
    build_dependencies: BTreeMap<String, IgnoredAny>,
}

impl Tables {
    fn names(&self, dependency: &str) -> bool {
        [
            &self.dependencies,
            &self.dev_dependencies,
            &self.build_dependencies,
        ]
        .iter()
        .any(|table| table.contains_key(dependency))
    }
}

#[derive(Deserialize, Default)]
struct Declaring {
    #[serde(flatten)]
    tables: Tables,
    #[serde(default)]
    target: BTreeMap<String, Tables>,
}

#[derive(Deserialize, Default)]
struct Ignores {
    package: Option<Section>,
    workspace: Option<Section>,
}

#[derive(Deserialize, Default)]
struct Section {
    metadata: Option<Tools>,
}

#[derive(Deserialize, Default)]
struct Tools {
    #[serde(rename = "cargo-shear")]
    cargo_shear: Option<IgnoredAny>,
}

fn forbid_ignore_tables(repo: &Path, workspace: &Workspace) -> Result<(), String> {
    let mut offending = BTreeSet::new();
    let manifests: BTreeSet<String> = workspace
        .packages
        .iter()
        .map(manifest_of)
        .chain(std::iter::once("Cargo.toml".to_owned()))
        .collect();
    for path in manifests {
        let text = std::fs::read_to_string(repo.join(&path))
            .map_err(|e| format!("could not read {path}: {e}"))?;
        let ignores: Ignores =
            toml::from_str(&text).map_err(|e| format!("{path} is malformed: {}", e.message()))?;
        let has = |section: &Option<Section>| {
            section
                .as_ref()
                .and_then(|s| s.metadata.as_ref())
                .is_some_and(|tools| tools.cargo_shear.is_some())
        };
        if has(&ignores.package) || has(&ignores.workspace) {
            offending.insert(path);
        }
    }
    if offending.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{} holds a cargo-shear metadata table, which would be read from the pull request itself; remove it and list the exemption in {EXEMPTIONS_FILE} on the base branch",
        offending.into_iter().collect::<Vec<_>>().join(", ")
    ))
}

enum At<'a> {
    Rev(&'a str),
    Head(&'a Head),
}

fn declares(repo: &Path, workspace: &Workspace, pair: &Pair, at: At) -> Result<bool, String> {
    let Some(package) = workspace.packages.iter().find(|p| p.name == pair.0) else {
        return Ok(false);
    };
    let path = manifest_of(package);
    let text = match at {
        At::Rev(rev) => scope::read_at(repo, rev, &path)?,
        At::Head(head) => scope::head_text(repo, head, &path)?,
    };
    let Some(text) = text else {
        return Ok(false);
    };
    let manifest: Declaring =
        toml::from_str(&text).map_err(|e| format!("{path} is malformed: {}", e.message()))?;
    Ok(manifest.tables.names(&pair.1)
        || manifest.target.values().any(|tables| tables.names(&pair.1)))
}

pub(crate) fn check(
    repo: &Path,
    base: &str,
    head: &Head,
    workspace: &Workspace,
) -> Result<String, String> {
    check_with(repo, base, head, workspace, &[])
}

fn check_with(
    repo: &Path,
    base: &str,
    head: &Head,
    workspace: &Workspace,
    env: &[(&str, &str)],
) -> Result<String, String> {
    require_version(repo, env)?;
    forbid_ignore_tables(repo, workspace)?;
    let findings = run_shear(repo, env)?;
    let tracked: BTreeSet<String> = scope::tracked(repo, head)?.into_iter().collect();
    let owner = |file: &str| {
        workspace
            .packages
            .iter()
            .find(|p| manifest_of(p) == file)
            .map(|p| p.name.clone())
    };
    let mut dependencies: BTreeMap<Pair, BTreeSet<String>> = BTreeMap::new();
    let mut others: BTreeMap<String, String> = BTreeMap::new();
    for finding in &findings {
        let on_disk = |file: Option<&str>| file.is_some_and(|file| tracked.contains(file));
        let subject = subject(finding, &owner);
        if let Some(pair) = subject.exemption_pair() {
            if on_disk(finding.file.as_deref()) {
                let line = format!("{}: {} ({})", pair.0, finding.message, finding.code);
                dependencies.entry(pair).or_default().insert(line);
            }
            continue;
        }
        match subject {
            Subject::WorkspaceEntry { .. } if on_disk(finding.file.as_deref()) => {
                let line = format!("workspace: {} ({})", finding.message, finding.code);
                others.insert(line.clone(), line);
            }
            Subject::Files { package, paths } => {
                let dir = workspace
                    .packages
                    .iter()
                    .find(|p| p.name == package)
                    .map(|p| p.dir.as_str())
                    .unwrap_or_default();
                let kind = if finding.code == "shear/empty_files" {
                    "empty file"
                } else {
                    "unlinked file"
                };
                for path in paths {
                    let full = if dir.is_empty() {
                        path
                    } else {
                        format!("{dir}/{path}")
                    };
                    if tracked.contains(&full) {
                        let label = workspace
                            .owner(&full)
                            .map_or(package.as_str(), |p| p.name.as_str());
                        others.insert(
                            full.clone(),
                            format!("{label}: {kind} {full} ({})", finding.code),
                        );
                    }
                }
            }
            Subject::Unknown { code, message } => {
                let line = format!("cargo shear: {} ({code})", message.replace('\n', " "));
                others.insert(line.clone(), line);
            }
            Subject::Dependency { .. } | Subject::WorkspaceEntry { .. } => {}
        }
    }
    let short = scope::short(base);
    let count = dependencies.values().map(BTreeSet::len).sum::<usize>() + others.len();
    let Some(found) = scope::configuration_at(repo, base, head, EXEMPTIONS_FILE)? else {
        return Ok(format!("not in force yet at {short}; {count} findings"));
    };
    let base_pairs = match &found {
        Found::AtBase(text) => exemption_pairs(text, &format!("at the base {short}"))?,
        Found::Restored { last, .. } => {
            exemption_pairs(last, &format!("before its removal from the base {short}"))?
        }
    };
    let head_pairs = match scope::head_text(repo, head, EXEMPTIONS_FILE)? {
        Some(text) => exemption_pairs(&text, "at the head")?,
        None => BTreeSet::new(),
    };
    let applied: BTreeSet<&Pair> = base_pairs.intersection(&head_pairs).collect();
    let mut lines: BTreeSet<String> = others.into_values().collect();
    let mut used = 0;
    for (pair, found_lines) in &dependencies {
        if applied.contains(pair) {
            used += 1;
        } else {
            lines.extend(found_lines.iter().cloned());
        }
    }
    let mut stale = BTreeSet::new();
    for pair in &head_pairs {
        if dependencies.contains_key(pair) {
            continue;
        }
        if declares(repo, workspace, pair, At::Rev(base))?
            || declares(repo, workspace, pair, At::Head(head))?
        {
            stale.insert(format!("{}: stale exemption for `{}`", pair.0, pair.1));
        }
    }
    if lines.is_empty() && stale.is_empty() {
        return Ok(format!("no unused dependency; {used} exemptions applied"));
    }
    let mut message = String::new();
    for line in lines.iter().chain(stale.iter()) {
        message.push_str(line);
        message.push('\n');
    }
    if !lines.is_empty() {
        message.push_str(&format!("fix: {FIX_FINDING}\n"));
    }
    if !stale.is_empty() {
        message.push_str(&format!("fix: {FIX_STALE}\n"));
    }
    Err(message.trim_end().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(json: &str) -> Finding {
        serde_json::from_str(json).unwrap()
    }

    fn owner(file: &str) -> Option<String> {
        match file {
            "Cargo.toml" => Some("mcrs".to_owned()),
            other => other
                .strip_prefix("crates/")?
                .strip_suffix("/Cargo.toml")
                .map(str::to_owned),
        }
    }

    #[test]
    fn cargo_shear_findings_map_to_their_package_and_dependency() {
        let dependency = |package: &str, name: &str| Subject::Dependency {
            package: package.to_owned(),
            name: name.to_owned(),
        };
        let files = |package: &str, path: &str| Subject::Files {
            package: package.to_owned(),
            paths: vec![path.to_owned()],
        };
        let rows: [(&str, Subject, bool); 7] = [
            (
                r#"{"code":"shear/unused_dependency","severity":"error","message":"unused dependency `mcrs_minecraft_core`","file":"crates/mcrs_minecraft_assets/Cargo.toml","location":{"offset":384,"length":19},"help":"remove this dependency","fixable":true}"#,
                dependency("mcrs_minecraft_assets", "mcrs_minecraft_core"),
                true,
            ),
            (
                r#"{"code":"shear/unused_workspace_dependency","severity":"error","message":"unused workspace dependency `rocksdb`","file":"Cargo.toml","location":{"offset":917,"length":7},"help":"remove this dependency","fixable":true}"#,
                Subject::WorkspaceEntry {
                    name: "rocksdb".to_owned(),
                },
                false,
            ),
            (
                r#"{"code":"shear/unused_feature_dependency","severity":"warning","message":"dependency `mcrs_minecraft_worldgen_carver` only used in features","file":"crates/mcrs_minecraft_worldgen/Cargo.toml","location":{"offset":939,"length":30},"fixable":false}"#,
                dependency("mcrs_minecraft_worldgen", "mcrs_minecraft_worldgen_carver"),
                true,
            ),
            (
                r#"{"code":"shear/misplaced_dependency","severity":"error","message":"misplaced dependency `tokio`","file":"crates/mcrs_minecraft_server/Cargo.toml","help":"move this dependency to `[dev-dependencies]`","fixable":true}"#,
                dependency("mcrs_minecraft_server", "tokio"),
                true,
            ),
            (
                r#"{"code":"shear/empty_files","severity":"warning","message":"1 empty file in `mcrs`\ncrates/mcrs_minecraft_protocol/src/entity/sniffer.rs","help":"delete this file","fixable":false}"#,
                files(
                    "mcrs",
                    "crates/mcrs_minecraft_protocol/src/entity/sniffer.rs",
                ),
                false,
            ),
            (
                r#"{"code":"shear/unlinked_files","severity":"warning","message":"1 unlinked file in `mcrs_minecraft_client`\nsrc/eb.rs","help":"delete this file","fixable":false}"#,
                files("mcrs_minecraft_client", "src/eb.rs"),
                false,
            ),
            (
                r#"{"code":"shear/brand_new","severity":"error","message":"something new about `x`","fixable":false}"#,
                Subject::Unknown {
                    code: "shear/brand_new".to_owned(),
                    message: "something new about `x`".to_owned(),
                },
                false,
            ),
        ];
        for (json, expected, exemptable) in rows {
            let found = subject(&finding(json), &owner);
            assert_eq!(found, expected, "{json}");
            assert_eq!(found.exemption_pair().is_some(), exemptable, "{json}");
        }
    }

    #[test]
    fn a_cargo_shear_run_is_trusted_only_as_a_parsed_report() {
        const CLEAN: &str = r#"{"summary":{"errors":0,"warnings":0,"fixed":0},"findings":[]}"#;
        const ONE: &str = r#"{"summary":{"errors":1,"warnings":0,"fixed":0},"findings":[{"code":"shear/unused_dependency","severity":"error","message":"unused dependency `b`","file":"c/Cargo.toml","fixable":true}]}"#;
        type Row = (
            &'static str,
            Option<i32>,
            &'static str,
            &'static str,
            Result<usize, &'static str>,
        );
        let rows: [Row; 6] = [
            ("a clean report", Some(0), CLEAN, "", Ok(0)),
            ("findings", Some(1), ONE, "", Ok(1)),
            (
                "an unknown flag",
                Some(1),
                "Error: `--bogus` is not expected in this context\n",
                "",
                Err("`--bogus` is not expected in this context"),
            ),
            (
                "a fatal error",
                Some(2),
                "",
                "error: could not read the manifest\n",
                Err("could not read the manifest"),
            ),
            (
                "exit 1 with no finding",
                Some(1),
                CLEAN,
                "",
                Err("reported no finding"),
            ),
            ("a signal", None, "", "", Err("signal")),
        ];
        for (name, status, stdout, stderr, expected) in rows {
            let found = judge(status, stdout.as_bytes(), stderr.as_bytes());
            match (expected, found) {
                (Ok(count), Ok(findings)) => assert_eq!(findings.len(), count, "{name}"),
                (Err(part), Err(message)) => assert!(message.contains(part), "{name}: {message}"),
                (expected, found) => panic!(
                    "{name}: expected {expected:?}, found {:?}",
                    found.map(|f| f.len())
                ),
            }
        }
    }

    #[test]
    fn the_version_check_accepts_only_the_pinned_release() {
        let install = "cargo install --locked cargo-shear@1.14.0";
        let rows: [(&str, Result<(), &str>); 4] = [
            ("Version: 1.14.0\n\n", Ok(())),
            ("Version: 1.14.1\n", Err(install)),
            ("", Err(install)),
            ("error: no such command: `shear`", Err("no such command")),
        ];
        for (output, expected) in rows {
            match (expected, version_matches(output)) {
                (Ok(()), Ok(())) => {}
                (Err(part), Err(message)) => {
                    assert!(message.contains(part), "{output:?}: {message}")
                }
                (expected, found) => panic!("{output:?}: expected {expected:?}, found {found:?}"),
            }
        }
    }

    mod cargo_shear {
        use super::super::*;
        use crate::checks::capture;
        use crate::scope::tests::Repo;
        use std::path::PathBuf;
        use std::sync::LazyLock;

        type Edit = (String, Option<String>);

        enum Expect {
            Pass(&'static str),
            Fail(&'static [&'static str]),
        }

        struct Case {
            name: &'static str,
            carried_then_removed: bool,
            base: Vec<Edit>,
            head: Vec<Edit>,
            untracked: Vec<Edit>,
            expect: Expect,
        }

        static CARGO_HOME: LazyLock<PathBuf> = LazyLock::new(|| {
            let dir = std::env::temp_dir().join(format!("xtask-shear-home-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            dir
        });

        fn env() -> [(&'static str, &'static str); 2] {
            let home: &'static str = CARGO_HOME.to_str().unwrap();
            [("CARGO_HOME", home), ("CARGO_NET_OFFLINE", "true")]
        }

        fn put(path: &str, text: &str) -> Edit {
            (path.to_owned(), Some(text.to_owned()))
        }

        fn root(entries: &str, tail: &str) -> String {
            format!(
                "[workspace]\nmembers = [\"a\", \"b\", \"c\"]\nresolver = \"3\"\n\n[workspace.package]\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace.dependencies]\nb = {{ path = \"b\" }}\n{entries}{tail}"
            )
        }

        fn member(name: &str, dependencies: &str, tail: &str) -> String {
            format!(
                "[package]\nname = \"{name}\"\nversion.workspace = true\nedition.workspace = true\n\n[dependencies]\n{dependencies}{tail}"
            )
        }

        const USES_B: &str = "b.workspace = true\n";

        fn c_depends() -> Edit {
            put("c/Cargo.toml", &member("c", USES_B, ""))
        }

        fn c_independent() -> Edit {
            put("c/Cargo.toml", &member("c", "", ""))
        }

        fn exemptions(pairs: &[(&str, &str)]) -> Edit {
            let text: String = pairs
                .iter()
                .map(|(package, dependency)| {
                    format!(
                        "[[exemption]]\npackage = \"{package}\"\ndependency = \"{dependency}\"\nreason = \"needed\"\n\n"
                    )
                })
                .collect();
            put(EXEMPTIONS_FILE, &text)
        }

        fn standard(repo: &Repo) {
            repo.write("Cargo.toml", &root("", ""));
            repo.write("a/Cargo.toml", &member("a", USES_B, ""));
            repo.write("a/src/lib.rs", "pub fn f() {\n    b::g();\n}\n");
            repo.write("b/Cargo.toml", &member("b", "", ""));
            repo.write("b/src/lib.rs", "pub fn g() {}\n");
            repo.write("c/Cargo.toml", &member("c", "", ""));
            repo.write("c/src/lib.rs", "pub fn h() {}\n");
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
            let out = capture(
                &repo.0,
                "cargo",
                &["generate-lockfile", "--offline"],
                &env(),
            )
            .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
        }

        fn workspace(repo: &Repo) -> Workspace {
            let args = ["metadata", "--format-version", "1", "--no-deps", "--locked"];
            let out = capture(&repo.0, "cargo", &args, &env()).unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            Workspace::parse(&String::from_utf8_lossy(&out.stdout)).unwrap()
        }

        fn case(name: &'static str, base: Vec<Edit>, head: Vec<Edit>, expect: Expect) -> Case {
            Case {
                name,
                carried_then_removed: false,
                base,
                head,
                untracked: Vec::new(),
                expect,
            }
        }

        fn cases() -> Vec<Case> {
            let none = || exemptions(&[]);
            let c_b = || exemptions(&[("c", "b")]);
            let a_b = || exemptions(&[("a", "b")]);
            let stray = || put("b/src/stray.rs", "pub fn s() {}\n");
            let entry = || put("Cargo.toml", &root("zzz-not-a-real-crate = \"1\"\n", ""));
            vec![
                case(
                    "an unused dependency fails",
                    vec![none()],
                    vec![c_depends()],
                    Expect::Fail(&["c: unused dependency `b`"]),
                ),
                case(
                    "an unused workspace entry fails",
                    vec![none()],
                    vec![entry()],
                    Expect::Fail(&["unused workspace dependency `zzz-not-a-real-crate`"]),
                ),
                case(
                    "a used dependency alone passes",
                    vec![none()],
                    vec![],
                    Expect::Pass("no unused dependency"),
                ),
                case(
                    "an exemption in the base and the head file is applied",
                    vec![c_b()],
                    vec![c_depends()],
                    Expect::Pass("1 exemptions applied"),
                ),
                case(
                    "an exemption only in the head file does not apply",
                    vec![none()],
                    vec![c_b(), c_depends()],
                    Expect::Fail(&["c: unused dependency `b`"]),
                ),
                case(
                    "a head-only entry with no dependency on either side is pending",
                    vec![none()],
                    vec![c_b()],
                    Expect::Pass("0 exemptions applied"),
                ),
                case(
                    "an entry in both files with no dependency on either side is pending",
                    vec![c_b()],
                    vec![],
                    Expect::Pass("0 exemptions applied"),
                ),
                case(
                    "an entry for a dependency that is used is stale",
                    vec![a_b()],
                    vec![],
                    Expect::Fail(&["a: stale exemption for `b`"]),
                ),
                case(
                    "a head-only entry for a dependency that is used is stale",
                    vec![none()],
                    vec![a_b()],
                    Expect::Fail(&["a: stale exemption for `b`"]),
                ),
                case(
                    "an entry kept after its dependency was removed at the head is stale",
                    vec![c_b(), c_depends()],
                    vec![c_independent()],
                    Expect::Fail(&["c: stale exemption for `b`"]),
                ),
                case(
                    "a cargo-shear table in a member manifest fails",
                    vec![none()],
                    vec![put(
                        "c/Cargo.toml",
                        &member(
                            "c",
                            "",
                            "\n[package.metadata.cargo-shear]\nignored = [\"b\"]\n",
                        ),
                    )],
                    Expect::Fail(&["c/Cargo.toml"]),
                ),
                case(
                    "a cargo-shear table in the root manifest fails",
                    vec![none()],
                    vec![put(
                        "Cargo.toml",
                        &root(
                            "",
                            "\n[workspace.metadata.cargo-shear]\nignored = [\"b\"]\n",
                        ),
                    )],
                    Expect::Fail(&["Cargo.toml"]),
                ),
                case(
                    "an entry removed at the head while the dependency stays fails",
                    vec![c_b(), c_depends()],
                    vec![none()],
                    Expect::Fail(&["c: unused dependency `b`"]),
                ),
                case(
                    "an entry removed together with its dependency passes",
                    vec![c_b(), c_depends()],
                    vec![none(), c_independent()],
                    Expect::Pass("no unused dependency"),
                ),
                Case {
                    untracked: vec![stray()],
                    ..case(
                        "an untracked unlinked file passes",
                        vec![none()],
                        vec![],
                        Expect::Pass("no unused dependency"),
                    )
                },
                case(
                    "the same unlinked file tracked fails",
                    vec![none()],
                    vec![stray()],
                    Expect::Fail(&["b/src/stray.rs"]),
                ),
                case(
                    "a base that never carried the file is not in force",
                    vec![],
                    vec![c_depends(), none()],
                    Expect::Pass("not in force yet"),
                ),
                Case {
                    carried_then_removed: true,
                    ..case(
                        "a base whose history removed the file needs it restored",
                        vec![],
                        vec![c_depends()],
                        Expect::Fail(&["was removed from the base; restore it"]),
                    )
                },
                case(
                    "an unknown key in the base file fails",
                    vec![put(
                        EXEMPTIONS_FILE,
                        "[[exemption]]\npackage = \"c\"\ndependency = \"b\"\nreason = \"x\"\nextra = 1\n",
                    )],
                    vec![],
                    Expect::Fail(&[EXEMPTIONS_FILE, "extra"]),
                ),
                case(
                    "an empty reason in the base file fails",
                    vec![put(
                        EXEMPTIONS_FILE,
                        "[[exemption]]\npackage = \"c\"\ndependency = \"b\"\nreason = \" \"\n",
                    )],
                    vec![],
                    Expect::Fail(&["no reason", "`b`"]),
                ),
            ]
        }

        #[test]
        fn the_check_judges_a_workspace_against_the_exemptions_at_its_base() {
            for case in cases() {
                let repo = Repo::new("shear");
                repo.init();
                standard(&repo);
                if case.carried_then_removed {
                    repo.write(EXEMPTIONS_FILE, "");
                    lock(&repo);
                    repo.commit("carry the exemptions");
                    repo.remove(EXEMPTIONS_FILE);
                }
                apply(&repo, &case.base);
                lock(&repo);
                repo.commit("base");
                let base = repo.sha();
                apply(&repo, &case.head);
                lock(&repo);
                repo.commit("head");
                let head = if case.untracked.is_empty() {
                    Head::Commit(repo.sha())
                } else {
                    apply(&repo, &case.untracked);
                    Head::Worktree
                };
                let found = check_with(&repo.0, &base, &head, &workspace(&repo), &env());
                match (&case.expect, found) {
                    (Expect::Pass(part), Ok(detail)) => {
                        assert!(detail.contains(part), "{}: {detail}", case.name)
                    }
                    (Expect::Fail(parts), Err(message)) => {
                        for part in *parts {
                            assert!(
                                message.contains(part),
                                "{}: missing {part:?} in {message}",
                                case.name
                            )
                        }
                    }
                    (_, other) => panic!("{}: {other:?}", case.name),
                }
            }
        }
    }
}
