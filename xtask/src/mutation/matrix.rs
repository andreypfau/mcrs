use crate::git;
use crate::metadata::Workspace;
use crate::mutation::attribution::{self, TestId};
use crate::mutation::classify::{self, Protection};
use crate::mutation::config;
use crate::mutation::outcomes::{Class, Measured};
use crate::mutation::run::{self, Run};
use crate::mutation::tool;
use crate::mutation::worktree::{self, Tree};
use crate::scope::{self, Head, Scope};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

const MATRIX_DIR: &str = "target/mutation/matrix";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Attributed {
    pub name: String,
    pub outcome: Class,
    pub killed_by: BTreeSet<TestId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Listed {
    pub id: TestId,
    pub package: String,
    pub measured: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Pass {
    pub package: String,
    pub test_packages: Vec<String>,
    pub shard: Option<(u32, u32)>,
    pub commit: String,
    pub mutants: Vec<Attributed>,
    pub tests: Vec<Listed>,
}

pub(crate) fn attribute(
    measured: &[Measured],
    read_log: &dyn Fn(&str) -> Result<String, String>,
    listed: &BTreeSet<TestId>,
) -> Result<Vec<Attributed>, String> {
    let mut attributed = Vec::with_capacity(measured.len());
    for mutant in measured {
        let mut killed_by = BTreeSet::new();
        if mutant.class == Class::Caught {
            let log = read_log(&mutant.log_path)?;
            killed_by = attribution::failing_tests(&log)
                .map_err(|e| format!("{}: {e} (log {})", mutant.name, mutant.log_path))?;
            if killed_by.is_empty() {
                return Err(format!(
                    "{}: cargo-mutants counted it caught, but its log {} names no failing test; the log format of nextest may have changed",
                    mutant.name, mutant.log_path
                ));
            }
            if let Some(stray) = killed_by.iter().find(|test| !listed.contains(test)) {
                return Err(format!(
                    "{}: its log {} names the failing test {} {}, which the test listing of this pass does not contain",
                    mutant.name, mutant.log_path, stray.binary, stray.name
                ));
            }
        }
        attributed.push(Attributed {
            name: mutant.name.clone(),
            outcome: mutant.class,
            killed_by,
        });
    }
    Ok(attributed)
}

#[derive(Deserialize)]
struct List {
    #[serde(rename = "rust-suites")]
    suites: BTreeMap<String, Suite>,
}

#[derive(Deserialize)]
struct Suite {
    #[serde(rename = "package-name")]
    package: String,
    testcases: BTreeMap<String, Case>,
}

#[derive(Deserialize)]
struct Case {
    #[serde(rename = "filter-match")]
    filter: FilterMatch,
}

#[derive(Deserialize)]
struct FilterMatch {
    status: String,
}

fn parse_list(json: &[u8]) -> Result<Vec<Listed>, String> {
    let list: List = serde_json::from_slice(json).map_err(|e| e.to_string())?;
    let mut tests = Vec::new();
    for (binary, suite) in list.suites {
        for (name, case) in suite.testcases {
            tests.push(Listed {
                id: TestId {
                    binary: binary.clone(),
                    name,
                },
                package: suite.package.clone(),
                measured: case.filter.status == "matches",
            });
        }
    }
    Ok(tests)
}

fn list_tests(tree: &Tree, packages: &[String]) -> Result<Vec<Listed>, String> {
    let mut command = tool::cargo(&tree.path);
    command
        .args([
            "nextest",
            "list",
            "--cargo-profile",
            "mutants",
            "--profile",
            "mutants-matrix",
            "--message-format",
            "json",
        ])
        .env("CARGO_TARGET_DIR", &tree.target_dir);
    for package in packages {
        command.args(["-p", package]);
    }
    let output = command
        .output()
        .map_err(|e| format!("could not run cargo nextest: {e}; install cargo-nextest"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo nextest could not list the tests of {}: {}",
            packages.join(", "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    parse_list(&output.stdout).map_err(|e| {
        format!(
            "the test listing of {} cannot be read: {e}",
            packages.join(", ")
        )
    })
}

struct Measuring<'a> {
    tree: &'a Tree,
    config: &'a Path,
    commit: &'a str,
    shard: Option<(u32, u32)>,
}

impl Measuring<'_> {
    fn pass(&self, name: &str, package: &str, test_packages: &[String]) -> Result<Pass, String> {
        let started = Instant::now();
        let tests = list_tests(self.tree, test_packages)?;
        let listed: BTreeSet<TestId> = tests
            .iter()
            .filter(|test| test.measured)
            .map(|test| test.id.clone())
            .collect();
        let out_dir = run::execute(
            self.tree,
            self.config,
            &Run {
                name,
                package,
                test_packages,
                test_timeout_seconds: tool::MATRIX_TEST_TIMEOUT_SECONDS,
                skip_baseline: false,
                shard: self.shard,
                in_diff: None,
                names: &[],
            },
        )?;
        let measured = run::measured(&out_dir)?;
        let mutants = attribute(
            &measured,
            &|log| {
                std::fs::read_to_string(out_dir.join(log))
                    .map_err(|e| format!("could not read the log {log}: {e}"))
            },
            &listed,
        )?;
        let caught = mutants
            .iter()
            .filter(|m| m.outcome == Class::Caught)
            .count();
        println!(
            "{name}: {} mutants of {package} against {} test packages, {caught} caught, {:.0}s",
            mutants.len(),
            test_packages.len(),
            started.elapsed().as_secs_f64()
        );
        Ok(Pass {
            package: package.to_owned(),
            test_packages: test_packages.to_vec(),
            shard: self.shard,
            commit: self.commit.to_owned(),
            mutants,
            tests,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Status {
    ZeroKill,
    Protected,
    NotMeasured,
    Killing,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Row {
    pub id: TestId,
    pub status: Status,
    pub rule: Option<String>,
    pub kills_a: u64,
    pub kills_b: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct DependentKills {
    pub id: TestId,
    pub package: String,
    pub kills: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Matrix {
    pub package: String,
    pub commit: String,
    pub tests: Vec<Row>,
    pub dependents: Vec<DependentKills>,
    pub unclassified: Vec<TestId>,
}

pub(crate) struct Shard {
    pub label: String,
    pub passes: BTreeMap<String, Pass>,
}

pub(crate) fn judge(
    package: &str,
    passes: &[Pass],
    protected: &BTreeMap<TestId, String>,
) -> Matrix {
    let mut owner: BTreeMap<&TestId, &str> = BTreeMap::new();
    let mut measured: BTreeMap<&TestId, bool> = BTreeMap::new();
    for listed in passes.iter().flat_map(|pass| &pass.tests) {
        owner.insert(&listed.id, &listed.package);
        let seen = measured.entry(&listed.id).or_insert(listed.measured);
        *seen = *seen && listed.measured;
    }
    let mut own_kills: BTreeMap<&TestId, (u64, u64)> = BTreeMap::new();
    let mut dependent_kills: BTreeMap<&TestId, u64> = BTreeMap::new();
    for pass in passes {
        let in_floor_scope = pass.package == package;
        for test in pass.mutants.iter().flat_map(|mutant| &mutant.killed_by) {
            if owner.get(test) == Some(&package) {
                let counts = own_kills.entry(test).or_default();
                if in_floor_scope {
                    counts.0 += 1;
                } else {
                    counts.1 += 1;
                }
            } else if in_floor_scope {
                *dependent_kills.entry(test).or_default() += 1;
            }
        }
    }
    let tests = owner
        .iter()
        .filter(|(_, owned_by)| **owned_by == package)
        .map(|(id, _)| {
            let (kills_a, kills_b) = own_kills.get(id).copied().unwrap_or_default();
            let rule = protected.get(*id).cloned();
            let status = if !measured[id] {
                Status::NotMeasured
            } else if rule.is_some() {
                Status::Protected
            } else if kills_a + kills_b > 0 {
                Status::Killing
            } else {
                Status::ZeroKill
            };
            Row {
                id: (*id).clone(),
                status,
                rule: rule.filter(|_| status == Status::Protected),
                kills_a,
                kills_b,
            }
        })
        .collect();
    let dependents = dependent_kills
        .into_iter()
        .map(|(id, kills)| DependentKills {
            id: id.clone(),
            package: owner[id].to_owned(),
            kills,
        })
        .collect();
    Matrix {
        package: package.to_owned(),
        commit: passes.first().map(|p| p.commit.clone()).unwrap_or_default(),
        tests,
        dependents,
        unclassified: Vec::new(),
    }
}

pub(crate) fn merge_passes(mut shards: Vec<Shard>) -> Result<BTreeMap<String, Pass>, String> {
    if shards.is_empty() {
        return Err("no shard directory was given".to_owned());
    }
    let mut slices = Vec::with_capacity(shards.len());
    for shard in &shards {
        let declared: BTreeSet<Option<(u32, u32)>> =
            shard.passes.values().map(|pass| pass.shard).collect();
        let mut declared = declared.into_iter();
        match (declared.next(), declared.next()) {
            (Some(slice), None) => slices.push(slice),
            (None, _) => return Err(format!("{} holds no pass result", shard.label)),
            _ => {
                return Err(format!(
                    "the passes of {} come from different shards",
                    shard.label
                ));
            }
        }
    }
    let counts: BTreeSet<Option<u32>> = slices.iter().map(|s| s.map(|(_, n)| n)).collect();
    if counts.len() > 1 {
        return Err(
            "the shard directories mix sharded and unsharded results, or shard counts".to_owned(),
        );
    }
    if let Some(Some(count)) = counts.first() {
        let mut seen = BTreeSet::new();
        for (index, _) in slices.iter().flatten() {
            if !seen.insert(*index) {
                return Err(format!("shard {index}/{count} was given twice"));
            }
        }
        if let Some(index) = (0..*count).find(|index| !seen.contains(index)) {
            return Err(format!(
                "shard {index}/{count} is missing; run it and merge again"
            ));
        }
    }
    let mut order: Vec<usize> = (0..shards.len()).collect();
    order.sort_by_key(|&i| slices[i]);
    let keys: BTreeSet<String> = shards[order[0]].passes.keys().cloned().collect();
    for shard in &shards {
        let own: BTreeSet<String> = shard.passes.keys().cloned().collect();
        if own != keys {
            return Err(format!(
                "{} holds the passes {own:?}, but the first shard holds {keys:?}",
                shard.label
            ));
        }
    }
    let mut merged = BTreeMap::new();
    for key in keys {
        let mut found: Option<Pass> = None;
        let mut first_in: BTreeMap<String, String> = BTreeMap::new();
        let mut known_tests: BTreeSet<TestId> = BTreeSet::new();
        for &i in &order {
            let shard = &mut shards[i];
            let pass = shard
                .passes
                .remove(&key)
                .expect("every shard holds every pass");
            let into = found.get_or_insert_with(|| Pass {
                shard: None,
                mutants: Vec::new(),
                tests: Vec::new(),
                ..pass.clone()
            });
            if into.commit != pass.commit {
                return Err(format!(
                    "{} measured commit {}, but the other shards measured {}",
                    shard.label, pass.commit, into.commit
                ));
            }
            for mutant in pass.mutants {
                if let Some(other) = first_in.insert(mutant.name.clone(), shard.label.clone()) {
                    return Err(format!(
                        "the mutant {} appears in {other} and in {} for {key}",
                        mutant.name, shard.label
                    ));
                }
                into.mutants.push(mutant);
            }
            for test in pass.tests {
                if known_tests.insert(test.id.clone()) {
                    into.tests.push(test);
                }
            }
        }
        merged.insert(key, found.expect("at least one shard"));
    }
    Ok(merged)
}

fn read_shard(dir: &Path) -> Result<Shard, String> {
    let label = dir.file_name().map_or_else(
        || dir.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("could not read {}: {e}", dir.display()))?;
    let mut passes = BTreeMap::new();
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        let key = path.file_stem().map(|s| s.to_string_lossy().into_owned());
        let Some(key) =
            key.filter(|k| k.starts_with("pass-") && path.extension().is_some_and(|e| e == "json"))
        else {
            continue;
        };
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("could not read {}: {e}", path.display()))?;
        let pass: Pass = serde_json::from_str(&text)
            .map_err(|e| format!("{} is not a pass result: {e}", path.display()))?;
        passes.insert(key, pass);
    }
    Ok(Shard { label, passes })
}

fn read_protection(dirs: &[PathBuf]) -> Result<Protection, String> {
    let mut found = Protection::default();
    for dir in dirs {
        let path = dir.join("protected.json");
        if !path.exists() {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("could not read {}: {e}", path.display()))?;
        let shard: Protection = serde_json::from_str(&text)
            .map_err(|e| format!("{} is not a protection list: {e}", path.display()))?;
        found.protected.extend(shard.protected);
        found.unclassified.extend(shard.unclassified);
    }
    found.protected.sort();
    found.protected.dedup();
    found.unclassified.sort();
    found.unclassified.dedup();
    Ok(found)
}

fn render_text(matrix: &Matrix) -> String {
    let mut text = format!(
        "matrix of {} at commit {}\n{}\n\n",
        matrix.package,
        matrix.commit,
        summary(matrix)
    );
    let mut rows: Vec<&Row> = matrix.tests.iter().collect();
    rows.sort_by(|a, b| (a.status, &a.id).cmp(&(b.status, &b.id)));
    for row in rows {
        let status = serde_json::to_value(row.status)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        text.push_str(&format!(
            "{status:<13} a={:<4} b={:<4} {} {}",
            row.kills_a, row.kills_b, row.id.binary, row.id.name
        ));
        if let Some(rule) = &row.rule {
            text.push_str(&format!("  [{rule}]"));
        }
        text.push('\n');
    }
    if !matrix.unclassified.is_empty() {
        text.push_str("\nconformance tests that match no protected rule:\n");
        for test in &matrix.unclassified {
            text.push_str(&format!("{} {}\n", test.binary, test.name));
        }
    }
    if !matrix.dependents.is_empty() {
        text.push_str(
            "\nkills by tests of other packages in pass (a), judged in their own matrix:\n",
        );
        for kill in &matrix.dependents {
            text.push_str(&format!(
                "{:<6} {} {}\n",
                kill.kills, kill.id.binary, kill.id.name
            ));
        }
    }
    text
}

pub(crate) fn summary(matrix: &Matrix) -> String {
    let count = |status| {
        matrix
            .tests
            .iter()
            .filter(|row| row.status == status)
            .count()
    };
    format!(
        "{}: {} tests: {} zero-kill, {} protected, {} not measured, {} killing; see target/{}/{}/matrix.txt",
        matrix.package,
        matrix.tests.len(),
        count(Status::ZeroKill),
        count(Status::Protected),
        count(Status::NotMeasured),
        count(Status::Killing),
        MATRIX_DIR.trim_start_matches("target/"),
        matrix.package
    )
}

pub(crate) fn merge_dirs(repo: &Path, package: &str, dirs: &[PathBuf]) -> Result<Matrix, String> {
    let shards = dirs
        .iter()
        .map(|dir| read_shard(dir))
        .collect::<Result<Vec<_>, _>>()?;
    let merged = merge_passes(shards)?;
    match merged.get("pass-a") {
        Some(pass) if pass.package == package => {}
        _ => return Err(format!("the shards hold no pass (a) of {package}")),
    }
    let passes: Vec<Pass> = merged.into_values().collect();
    let protection = read_protection(dirs)?;
    let rules: BTreeMap<TestId, String> = protection.protected.into_iter().collect();
    let mut matrix = judge(package, &passes, &rules);
    matrix.unclassified = protection.unclassified;
    let out = repo.join(MATRIX_DIR).join(package);
    std::fs::create_dir_all(&out)
        .map_err(|e| format!("could not create {}: {e}", out.display()))?;
    write_json(&out.join("matrix.json"), &matrix)?;
    let text_path = out.join("matrix.txt");
    std::fs::write(&text_path, render_text(&matrix))
        .map_err(|e| format!("could not write {}: {e}", text_path.display()))?;
    Ok(matrix)
}

pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| format!("could not write {}: {e}", path.display()))
}

pub(crate) fn shard_name(shard: Option<(u32, u32)>) -> String {
    shard.map_or_else(
        || "all".to_owned(),
        |(index, count)| format!("{index}-of-{count}"),
    )
}

pub(crate) fn direct_dependencies(workspace: &Workspace, package: &str) -> Vec<String> {
    let Some(found) = workspace.packages.iter().find(|p| p.name == package) else {
        return Vec::new();
    };
    let names: BTreeSet<&str> = found
        .dependencies
        .iter()
        .filter(|dep| dep.local && dep.name != package)
        .map(|dep| dep.name.as_str())
        .collect();
    names.into_iter().map(str::to_owned).collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PassSpec {
    pub name: String,
    pub package: String,
    pub test_packages: Vec<String>,
}

pub(crate) fn passes(workspace: &Workspace, package: &str) -> Result<Vec<PassSpec>, String> {
    let Scope::Packages(floor) =
        Scope::Packages(vec![package.to_owned()]).with_dependents(workspace)
    else {
        return Err("the floor scope of a package is a list of packages".to_owned());
    };
    let mut found = vec![PassSpec {
        name: "pass-a".to_owned(),
        package: package.to_owned(),
        test_packages: floor,
    }];
    for dependency in direct_dependencies(workspace, package) {
        found.push(PassSpec {
            name: format!("pass-b-{dependency}"),
            package: dependency,
            test_packages: vec![package.to_owned()],
        });
    }
    Ok(found)
}

pub(crate) fn shard_dirs(shards: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir(shards).map_err(|e| {
        format!(
            "could not read {}: {e}; download the shard artifacts first",
            shards.display()
        )
    })?;
    let mut dirs = Vec::new();
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            dirs.push(path);
        }
    }
    dirs.sort();
    Ok(dirs)
}

pub(crate) fn measure(
    repo: &Path,
    package: &str,
    shard: Option<(u32, u32)>,
    base_ref: &str,
) -> Result<PathBuf, String> {
    let commit = git::text(repo, &["rev-parse", "HEAD"])
        .map_err(|e| e.to_string())?
        .trim()
        .to_owned();
    tool::require_version(repo)?;
    let tree = worktree::prepare(repo, &commit)?;
    println!("measured commit {commit}");
    let workspace = Workspace::load(&tree.path)?;
    if !workspace.packages.iter().any(|p| p.name == package) {
        return Err(format!("{package} is not a package of this workspace"));
    }
    let head = Head::Commit(commit.clone());
    let base = scope::resolve_base(repo, base_ref, &head).map_err(|e| e.to_string())?;
    let base_config = config::base_config(repo, &base, &head)?;
    let config = config::write_matrix(repo, &base_config)?;

    let shard_dir = repo.join(MATRIX_DIR).join(package).join(shard_name(shard));
    if shard_dir.exists() {
        std::fs::remove_dir_all(&shard_dir)
            .map_err(|e| format!("could not clear {}: {e}", shard_dir.display()))?;
    }
    std::fs::create_dir_all(&shard_dir)
        .map_err(|e| format!("could not create {}: {e}", shard_dir.display()))?;

    let report = classify::workspace_report(&tree.path, &workspace)?;
    write_json(
        &shard_dir.join("protected.json"),
        &classify::protection(&report, package),
    )?;

    let measuring = Measuring {
        tree: &tree,
        config: &config,
        commit: &commit,
        shard,
    };
    for spec in passes(&workspace, package)? {
        let pass = measuring.pass(&spec.name, &spec.package, &spec.test_packages)?;
        write_json(&shard_dir.join(format!("{}.json", spec.name)), &pass)?;
    }
    Ok(shard_dir)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn id(binary: &str, name: &str) -> TestId {
        TestId {
            binary: binary.to_owned(),
            name: name.to_owned(),
        }
    }

    fn measured(name: &str, class: Class) -> Measured {
        Measured {
            name: name.to_owned(),
            class,
            log_path: format!("log/{name}.log"),
        }
    }

    #[test]
    fn a_caught_mutant_without_a_named_test_fails_the_matrix() {
        const NAMED: &str = "*** cargo nextest run --cargo-profile=mutants\n        FAIL [   0.1s] (  1/2) pkg a::b\n";
        const SILENT: &str = "*** cargo nextest run --cargo-profile=mutants\n     Summary [   0.1s] 2 tests run: 2 passed\n";
        const UNLISTED: &str = "*** cargo nextest run --cargo-profile=mutants\n        FAIL [   0.1s] (  1/2) pkg a::unlisted\n";
        let logs = |path: &str| match path {
            "log/named.log" | "log/timed_out_and_named.log" => Ok(NAMED.to_owned()),
            "log/silent.log" | "log/timed_out.log" => Ok(SILENT.to_owned()),
            "log/unlisted.log" => Ok(UNLISTED.to_owned()),
            other => Err(format!("{other} must not be read")),
        };
        let listed = BTreeSet::from([id("pkg", "a::b")]);

        let found = attribute(
            &[
                measured("named", Class::Caught),
                measured("timed_out", Class::Timeout),
                measured("timed_out_and_named", Class::Timeout),
                measured("missed", Class::Missed),
                measured("unviable", Class::Unviable),
            ],
            &logs,
            &listed,
        )
        .unwrap();
        let rows: Vec<(&str, Class, Vec<TestId>)> = found
            .iter()
            .map(|m| {
                (
                    m.name.as_str(),
                    m.outcome,
                    m.killed_by.iter().cloned().collect(),
                )
            })
            .collect();
        assert_eq!(
            rows,
            vec![
                ("named", Class::Caught, vec![id("pkg", "a::b")]),
                ("timed_out", Class::Timeout, vec![]),
                ("timed_out_and_named", Class::Timeout, vec![]),
                ("missed", Class::Missed, vec![]),
                ("unviable", Class::Unviable, vec![]),
            ]
        );

        let silent = attribute(&[measured("silent", Class::Caught)], &logs, &listed)
            .expect_err("a caught mutant with no failing test must fail the run");
        assert!(
            silent.contains("silent") && silent.contains("log/silent.log"),
            "{silent}"
        );

        let unlisted = attribute(&[measured("unlisted", Class::Caught)], &logs, &listed)
            .expect_err("a failing test outside the listed set is format drift");
        assert!(
            unlisted.contains("a::unlisted") && unlisted.contains("log/unlisted.log"),
            "{unlisted}"
        );
    }

    fn mutant(name: &str, outcome: Class, killed_by: &[(&str, &str)]) -> Attributed {
        Attributed {
            name: name.to_owned(),
            outcome,
            killed_by: killed_by.iter().map(|(b, t)| id(b, t)).collect(),
        }
    }

    fn listed(package: &str, binary: &str, name: &str, measured: bool) -> Listed {
        Listed {
            id: id(binary, name),
            package: package.to_owned(),
            measured,
        }
    }

    fn pass(
        package: &str,
        shard: Option<(u32, u32)>,
        mutants: Vec<Attributed>,
        tests: Vec<Listed>,
    ) -> Pass {
        Pass {
            package: package.to_owned(),
            test_packages: Vec::new(),
            shard,
            commit: "c0ffee".to_owned(),
            mutants,
            tests,
        }
    }

    #[test]
    fn a_test_is_zero_kill_only_when_both_passes_record_no_kill() {
        let own_tests = || {
            vec![
                listed("x", "x", "kills_in_a", true),
                listed("x", "x", "kills_in_b", true),
                listed("x", "x", "kills_nothing", true),
                listed("x", "x", "guards_layout", true),
                listed("x", "x", "an_exhaustive::sweep", false),
            ]
        };
        let mut floor = own_tests();
        floor.push(listed("y", "y", "dependent_kills", true));
        floor.push(listed("y", "y", "dependent_kills_nothing", true));
        let pass_a = pass(
            "x",
            None,
            vec![
                mutant(
                    "x.rs:1:1: a",
                    Class::Caught,
                    &[("x", "kills_in_a"), ("y", "dependent_kills")],
                ),
                mutant("x.rs:2:1: b", Class::Caught, &[("x", "kills_in_a")]),
                mutant("x.rs:3:1: c", Class::Missed, &[]),
            ],
            floor,
        );
        let pass_b_core = pass(
            "core",
            None,
            vec![
                mutant("core.rs:1:1: a", Class::Caught, &[("x", "kills_in_b")]),
                mutant("core.rs:2:1: b", Class::Timeout, &[]),
            ],
            own_tests(),
        );
        let pass_b_keys = pass(
            "keys",
            None,
            vec![mutant(
                "keys.rs:1:1: a",
                Class::Caught,
                &[("x", "kills_in_b")],
            )],
            own_tests(),
        );
        let protected = BTreeMap::from([(id("x", "guards_layout"), "size-pin".to_owned())]);

        let matrix = judge("x", &[pass_a, pass_b_core, pass_b_keys], &protected);

        let rows: Vec<(&str, Status, Option<&str>, u64, u64)> = matrix
            .tests
            .iter()
            .map(|r| {
                (
                    r.id.name.as_str(),
                    r.status,
                    r.rule.as_deref(),
                    r.kills_a,
                    r.kills_b,
                )
            })
            .collect();
        assert_eq!(
            rows,
            vec![
                ("an_exhaustive::sweep", Status::NotMeasured, None, 0, 0),
                ("guards_layout", Status::Protected, Some("size-pin"), 0, 0),
                ("kills_in_a", Status::Killing, None, 2, 0),
                ("kills_in_b", Status::Killing, None, 0, 2),
                ("kills_nothing", Status::ZeroKill, None, 0, 0),
            ]
        );
        assert_eq!(
            matrix.dependents,
            vec![DependentKills {
                id: id("y", "dependent_kills"),
                package: "y".to_owned(),
                kills: 1
            }]
        );
    }

    #[test]
    fn a_matrix_merge_needs_every_shard_once() {
        let shard = |k: u32, names: &[&str]| Shard {
            label: format!("{k}-of-3"),
            passes: BTreeMap::from([(
                "pass-a".to_owned(),
                pass(
                    "x",
                    Some((k, 3)),
                    names
                        .iter()
                        .map(|n| mutant(n, Class::Caught, &[("x", "t")]))
                        .collect(),
                    vec![listed("x", "x", "t", true)],
                ),
            )]),
        };
        let missing = merge_passes(vec![shard(0, &["m0"]), shard(2, &["m2"])])
            .expect_err("shard 1/3 is missing");
        assert!(missing.contains("1/3"), "{missing}");

        let twice = merge_passes(vec![
            shard(0, &["m0", "shared"]),
            shard(1, &["m1"]),
            shard(2, &["shared"]),
        ])
        .expect_err("a mutant in two shards");
        assert!(twice.contains("shared"), "{twice}");

        let whole = merge_passes(vec![
            shard(2, &["m2"]),
            shard(0, &["m0"]),
            shard(1, &["m1"]),
        ])
        .unwrap();
        let names: Vec<&str> = whole["pass-a"]
            .mutants
            .iter()
            .map(|m| m.name.as_str())
            .collect();
        assert_eq!(names, ["m0", "m1", "m2"]);
        assert_eq!(whole["pass-a"].shard, None);
        assert_eq!(whole["pass-a"].tests.len(), 1);
    }

    pub(crate) mod cargo_mutants {
        use super::*;
        use crate::scope::tests::Repo;

        const LOW: &str = "pub fn double(x: i32) -> i32 {\n    x * 2\n}\n\npub fn inc(x: i32) -> i32 {\n    x + 1\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn inc_adds_one() {\n        assert_eq!(inc(1), 2);\n    }\n}\n";
        const HIGH: &str = "pub fn marker() -> bool {\n    true\n}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn doubling_twice_quadruples() {\n        assert_eq!(low::double(low::double(1)), 4);\n    }\n\n    #[test]\n    fn always_true() {\n        assert!(true);\n    }\n\n    mod exhaustive {\n        #[test]\n        fn a_sweep() {\n            assert_eq!(low::inc(1), 2);\n        }\n    }\n}\n";
        const NEXTEST: &str = "[profile.default]\ndefault-filter = \"not test(/(^|::)(exhaustive|gpu|online|vanilla_server)::/)\"\n\n[profile.mutants]\nfail-fast = true\nretries = 0\n\n[profile.mutants-matrix]\nfail-fast = false\nretries = 0\nstatus-level = \"fail\"\nfinal-status-level = \"fail\"\nfailure-output = \"never\"\n";

        pub(crate) fn floor_workspace(label: &str) -> Repo {
            let repo = Repo::new(label);
            repo.init();
            repo.write(".gitignore", "/target\n");
            repo.write(
                "Cargo.toml",
                "[workspace]\nmembers = [\"low\", \"high\"]\nresolver = \"2\"\n\n[profile.mutants]\ninherits = \"test\"\ndebug = \"none\"\n",
            );
            repo.write(
                "low/Cargo.toml",
                "[package]\nname = \"low\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
            );
            repo.write("low/src/lib.rs", LOW);
            repo.write(
                "high/Cargo.toml",
                "[package]\nname = \"high\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nlow = { path = \"../low\" }\n",
            );
            repo.write("high/src/lib.rs", HIGH);
            repo.write(".config/nextest.toml", NEXTEST);
            let status = std::process::Command::new("cargo")
                .args(["generate-lockfile", "--offline"])
                .current_dir(&repo.0)
                .status()
                .unwrap();
            assert!(status.success());
            repo.commit("floor workspace");
            repo
        }

        #[test]
        fn the_matrix_attributes_kills_across_the_floor_scope() {
            let repo = floor_workspace("floor-scope");
            let shard_dir = measure(&repo.0, "low", None, "HEAD").unwrap();
            let json = std::fs::read_to_string(shard_dir.join("pass-a.json")).unwrap();
            let pass: Pass = serde_json::from_str(&json).unwrap();
            assert_eq!(pass.package, "low");
            assert_eq!(pass.test_packages, ["high", "low"]);
            let caught: Vec<&Attributed> = pass
                .mutants
                .iter()
                .filter(|m| m.outcome == Class::Caught)
                .collect();
            assert!(caught.iter().any(|m| m.name.contains("inc")), "{pass:?}");
            assert!(caught.iter().any(|m| m.name.contains("double")), "{pass:?}");
            for mutant in caught {
                let wanted = if mutant.name.contains(" inc") || mutant.name.contains("in inc") {
                    id("low", "tests::inc_adds_one")
                } else {
                    id("high", "tests::doubling_twice_quadruples")
                };
                assert_eq!(
                    mutant.killed_by,
                    BTreeSet::from([wanted]),
                    "{}",
                    mutant.name
                );
            }

            let high = measure(&repo.0, "high", None, "HEAD").unwrap();
            assert!(high.join("pass-b-low.json").exists());
            let matrix = merge_dirs(&repo.0, "high", &[high]).unwrap();
            let row = |name: &str| {
                matrix
                    .tests
                    .iter()
                    .find(|r| r.id.name == name)
                    .unwrap_or_else(|| panic!("{name} not in {matrix:?}"))
            };
            let doubling = row("tests::doubling_twice_quadruples");
            assert_eq!(doubling.status, Status::Killing, "{doubling:?}");
            assert!(
                doubling.kills_b > 0 && doubling.kills_a == 0,
                "{doubling:?}"
            );
            assert_eq!(row("tests::always_true").status, Status::ZeroKill);
            assert_eq!(
                row("tests::exhaustive::a_sweep").status,
                Status::NotMeasured
            );
            assert!(
                repo.0
                    .join("target/mutation/matrix/high/matrix.json")
                    .exists()
            );
        }
    }
}
