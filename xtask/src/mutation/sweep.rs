use crate::floors::{self, Floors};
use crate::metadata::Workspace;
use crate::mutation::matrix::{self, PassSpec, write_json};
use crate::mutation::outcomes::{self, Class, Measured};
use crate::mutation::plan::{self, Baseline, Plan, Unit};
use crate::mutation::{config, tool, verdict};
use crate::scope::Head;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub(crate) const SWEEP_DIR: &str = "target/mutation/sweep";
const ARTIFACT_PREFIX: &str = "mutation-sweep-";
pub(crate) const MAIN: &str = "main";

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct Artifact {
    pub name: String,
    pub expired: bool,
    pub created_at: String,
    pub branch: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct CrateResult {
    #[serde(rename = "crate")]
    pub package: String,
    pub commit: String,
    pub branch: String,
    pub run_id: u64,
    pub run_url: String,
    pub date: String,
    pub cargo_mutants: String,
    pub nextest: String,
    pub toolchain: String,
    pub caught: u64,
    pub missed: u64,
    pub timeout: u64,
    pub unviable: u64,
    pub score: Option<u64>,
    pub outcomes: BTreeMap<String, Class>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Previous {
    #[serde(rename = "crate")]
    pub package: String,
    pub commit: String,
    pub run_id: u64,
    pub file: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Run {
    pub id: u64,
    pub url: String,
    pub date: String,
    pub branch: String,
    pub cargo_mutants: String,
    pub nextest: String,
    pub toolchain: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Sweep,
    Matrix,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Chosen {
    Sweep(Vec<String>),
    Matrix {
        package: String,
        passes: Vec<PassSpec>,
    },
}

#[derive(Debug)]
pub(crate) struct Merged {
    pub results: Vec<CrateResult>,
    pub problems: Vec<String>,
}

pub(crate) fn last_sweeps(history: &[Artifact]) -> BTreeMap<String, String> {
    let mut last: BTreeMap<String, String> = BTreeMap::new();
    for artifact in history
        .iter()
        .filter(|artifact| !artifact.expired && artifact.branch == MAIN)
    {
        let Some(package) = swept_package(&artifact.name) else {
            continue;
        };
        match last.get_mut(package) {
            Some(seen) if *seen >= artifact.created_at => {}
            Some(seen) => seen.clone_from(&artifact.created_at),
            None => {
                last.insert(package.to_owned(), artifact.created_at.clone());
            }
        }
    }
    last
}

fn swept_package(name: &str) -> Option<&str> {
    let (package, commit) = name.strip_prefix(ARTIFACT_PREFIX)?.rsplit_once('-')?;
    let is_commit = commit.len() == 40 && commit.bytes().all(|byte| byte.is_ascii_hexdigit());
    (is_commit && !package.is_empty()).then_some(package)
}

pub(crate) fn order(
    floors: &Floors,
    packages: &[String],
    last: &BTreeMap<String, String>,
) -> Vec<String> {
    let mut names = packages.to_vec();
    names.sort_by_key(|name| {
        (
            floors.hundredths(name).is_none(),
            last.get(name).cloned(),
            name.clone(),
        )
    });
    names
}

pub(crate) fn pick(
    order: &[String],
    budget: usize,
    mut count: impl FnMut(&str) -> Result<usize, String>,
) -> Result<Vec<String>, String> {
    let mut taken = Vec::new();
    let mut total = 0;
    for name in order {
        let mutants = count(name)?;
        if !taken.is_empty() && total + mutants > budget {
            break;
        }
        total += mutants;
        taken.push(name.clone());
    }
    Ok(taken)
}

pub(crate) fn choose(
    workspace: &Workspace,
    floors: &Floors,
    history: &[Artifact],
    requested: Option<&str>,
    kind: Kind,
    budget: usize,
    count: impl FnMut(&str) -> Result<usize, String>,
) -> Result<Chosen, String> {
    if let Some(package) = requested {
        if !workspace.packages.iter().any(|p| p.name == package) {
            return Err(format!("{package} is not a package of this workspace"));
        }
        return Ok(match kind {
            Kind::Sweep => Chosen::Sweep(vec![package.to_owned()]),
            Kind::Matrix => Chosen::Matrix {
                package: package.to_owned(),
                passes: matrix::passes(workspace, package)?,
            },
        });
    }
    if kind == Kind::Matrix {
        return Err(
            "a kill matrix needs a crate: dispatch the workflow with the crate input".to_owned(),
        );
    }
    let packages: Vec<String> = workspace.packages.iter().map(|p| p.name.clone()).collect();
    let ordered = order(floors, &packages, &last_sweeps(history));
    pick(&ordered, budget, count).map(Chosen::Sweep)
}

pub(crate) fn merge(plan: &Plan, results: &BTreeMap<usize, Vec<Measured>>, run: &Run) -> Merged {
    let mut merged = Merged {
        results: Vec::new(),
        problems: Vec::new(),
    };
    for (at, unit) in plan.units.iter().enumerate() {
        let package = &unit.package;
        let mut problems: Vec<String> = verdict::missing_shards(plan, at, results)
            .into_iter()
            .map(|problem| format!("{package}: {problem}"))
            .collect();
        problems.extend(verdict::unit_problems(plan, at, results));
        if !problems.is_empty() {
            merged.problems.extend(problems);
            continue;
        }
        let mut found = CrateResult {
            package: package.clone(),
            commit: plan.head.clone(),
            branch: run.branch.clone(),
            run_id: run.id,
            run_url: run.url.clone(),
            date: run.date.clone(),
            cargo_mutants: run.cargo_mutants.clone(),
            nextest: run.nextest.clone(),
            toolchain: run.toolchain.clone(),
            caught: 0,
            missed: 0,
            timeout: 0,
            unviable: 0,
            score: None,
            outcomes: BTreeMap::new(),
        };
        let shards = (0..plan.shards.len()).filter(|&index| plan.shards[index].unit == at);
        for mutant in shards.flat_map(|index| &results[&index]) {
            match mutant.class {
                Class::Caught => found.caught += 1,
                Class::Timeout => found.timeout += 1,
                Class::Missed => found.missed += 1,
                Class::Unviable => found.unviable += 1,
            }
            found.outcomes.insert(mutant.name.clone(), mutant.class);
        }
        found.score = outcomes::score(found.caught, found.timeout, found.missed);
        merged.results.push(found);
    }
    merged
}

pub(crate) fn disagreements(current: &CrateResult, previous: &[CrateResult]) -> Vec<String> {
    previous
        .iter()
        .filter(|earlier| earlier.package == current.package && earlier.commit == current.commit)
        .flat_map(|earlier| outcomes::differences(&earlier.outcomes, &current.outcomes))
        .map(|difference| format!("{}: {difference}", current.package))
        .collect()
}

fn previous_dir(repo: &Path) -> PathBuf {
    repo.join(SWEEP_DIR).join("previous")
}

const INDEX: &str = "index.json";

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path, what: &str) -> Result<T, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("could not read {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{} is not {what}: {e}", path.display()))
}

pub(crate) fn index_previous(repo: &Path) -> Result<Vec<Previous>, String> {
    let dir = previous_dir(repo);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    let mut index = Vec::new();
    let entries =
        std::fs::read_dir(&dir).map_err(|e| format!("could not read {}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        let file = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        if file == INDEX || path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let found: CrateResult = read_json(&path, "a sweep result")?;
        index.push(Previous {
            package: found.package,
            commit: found.commit,
            run_id: found.run_id,
            file,
        });
    }
    index.sort_by(|a, b| (&a.package, a.run_id).cmp(&(&b.package, b.run_id)));
    write_json(&dir.join(INDEX), &index)?;
    Ok(index)
}

pub(crate) fn read_previous(repo: &Path) -> Result<Vec<CrateResult>, String> {
    let dir = previous_dir(repo);
    let index_path = dir.join(INDEX);
    if !index_path.exists() {
        return Ok(Vec::new());
    }
    let index: Vec<Previous> = read_json(&index_path, "an index of previous sweeps")?;
    let mut found = Vec::new();
    for entry in index {
        if Path::new(&entry.file)
            .file_name()
            .map(|name| name.to_string_lossy())
            != Some(entry.file.as_str().into())
        {
            return Err(format!(
                "{} lists {:?}, which is not a plain file name",
                index_path.display(),
                entry.file
            ));
        }
        let result: CrateResult = read_json(&dir.join(&entry.file), "a sweep result")?;
        if (&result.package, &result.commit, result.run_id)
            != (&entry.package, &entry.commit, entry.run_id)
        {
            return Err(format!(
                "{} does not hold the sweep the index lists for it",
                entry.file
            ));
        }
        found.push(result);
    }
    Ok(found)
}

pub(crate) fn results_dir(repo: &Path) -> PathBuf {
    repo.join(SWEEP_DIR).join("results")
}

pub(crate) fn read_results(repo: &Path) -> Result<Vec<CrateResult>, String> {
    let dir = results_dir(repo);
    let entries = std::fs::read_dir(&dir).map_err(|e| {
        format!(
            "could not read {}: {e}; merge the shards first",
            dir.display()
        )
    })?;
    let mut found = Vec::new();
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            found.push(read_json::<CrateResult>(&path, "a sweep result")?);
        }
    }
    found.sort_by(|a, b| a.package.cmp(&b.package));
    Ok(found)
}

fn read_history(repo: &Path) -> Result<Vec<Artifact>, String> {
    let path = repo.join(SWEEP_DIR).join("history.json");
    if !path.exists() {
        return Err(format!(
            "{} is missing; list the earlier sweep artifacts into it first (the workflow does, with an empty list when there are none)",
            path.display()
        ));
    }
    read_json(&path, "a list of sweep artifacts")
}

struct Lister<'a> {
    repo: &'a Path,
    gate: PathBuf,
    cache: BTreeMap<String, BTreeSet<String>>,
}

impl Lister<'_> {
    fn names(&mut self, package: &str) -> Result<&BTreeSet<String>, String> {
        if !self.cache.contains_key(package) {
            let listed = config::list_with(self.repo, &self.gate, package, None, &[])?;
            self.cache.insert(
                package.to_owned(),
                listed.into_iter().map(|mutant| mutant.name).collect(),
            );
        }
        Ok(&self.cache[package])
    }
}

pub(crate) fn assemble(
    workspace: &Workspace,
    floors: &Floors,
    head: &str,
    chosen: &Chosen,
    mut names: impl FnMut(&str) -> Result<BTreeSet<String>, String>,
) -> Result<Plan, String> {
    let mut units = Vec::new();
    let mut matrix_package = None;
    match chosen {
        Chosen::Sweep(packages) => {
            for package in packages {
                units.push(Unit {
                    package: package.clone(),
                    test_packages: plan::test_packages(workspace, package)?,
                    names: names(package)?.into_iter().collect(),
                    floor: Some(floors.hundredths(package).unwrap_or(0)),
                });
            }
        }
        Chosen::Matrix { package, passes } => {
            matrix_package = Some(package.clone());
            for pass in passes {
                units.push(Unit {
                    package: pass.package.clone(),
                    test_packages: pass.test_packages.clone(),
                    names: names(&pass.package)?.into_iter().collect(),
                    floor: Some(0),
                });
            }
        }
    }
    let counts: Vec<usize> = if matrix_package.is_some() {
        vec![units.iter().map(|unit| unit.names.len()).sum()]
    } else {
        units.iter().map(|unit| unit.names.len()).collect()
    };
    Ok(Plan {
        base: head.to_owned(),
        head: head.to_owned(),
        in_force: true,
        baseline: Baseline::Run,
        diff: String::new(),
        units,
        shards: plan::split(&counts, tool::MUTANTS_PER_SHARD),
        broken: Vec::new(),
        surface: Vec::new(),
        matrix: matrix_package,
    })
}

pub(crate) fn plan_command(requested: Option<&str>, kind: Kind) -> Result<(), String> {
    let repo = crate::checks::root_of(Path::new("."))?;
    let head = super::head_of(&repo)?;
    tool::require_version(&repo)?;
    let workspace = Workspace::load(&repo)?;
    let floors_path = repo.join(floors::FLOORS_FILE);
    let floors_text = std::fs::read_to_string(&floors_path)
        .map_err(|e| format!("could not read {}: {e}", floors_path.display()))?;
    let floors = floors::parse(&floors_text)
        .map_err(|e| format!("{} is malformed: {e}", floors::FLOORS_FILE))?;
    let history = read_history(&repo)?;
    let config = config::base_config(&repo, &head, &Head::Commit(head.clone()))?;
    let mut lister = Lister {
        repo: &repo,
        gate: config::write_gate(&repo, &config)?,
        cache: BTreeMap::new(),
    };
    let chosen = choose(
        &workspace,
        &floors,
        &history,
        requested,
        kind,
        tool::SWEEP_MUTANT_BUDGET,
        |name| lister.names(name).map(BTreeSet::len),
    )?;
    let plan = assemble(&workspace, &floors, &head, &chosen, |name| {
        lister.names(name).cloned()
    })?;
    plan::write_plan(&repo, &plan)?;
    let previous = index_previous(&repo)?;
    match &chosen {
        Chosen::Sweep(_) => {
            for (at, unit) in plan.units.iter().enumerate() {
                let shards = plan.shards.iter().filter(|s| s.unit == at).count();
                println!(
                    "{}: {} mutants in {shards} shards",
                    unit.package,
                    unit.names.len()
                );
            }
        }
        Chosen::Matrix { package, passes } => {
            for (pass, unit) in passes.iter().zip(&plan.units) {
                println!(
                    "{package} {}: {} mutants of {}",
                    pass.name,
                    unit.names.len(),
                    unit.package
                );
            }
        }
    }
    let crates: Vec<&str> = match &chosen {
        Chosen::Sweep(packages) => packages.iter().map(String::as_str).collect(),
        Chosen::Matrix { package, .. } => vec![package.as_str()],
    };
    let indexes: Vec<usize> = (0..plan.shards.len()).collect();
    println!("previous={}", previous.len());
    println!(
        "kind={}",
        match kind {
            Kind::Sweep => "sweep",
            Kind::Matrix => "matrix",
        }
    );
    println!(
        "crates={}",
        serde_json::to_string(&crates).map_err(|e| e.to_string())?
    );
    println!("count={}", indexes.len());
    println!(
        "shards={}",
        serde_json::to_string(&indexes).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn copy_flat(from: &Path, to: &Path) -> Result<(), String> {
    if to.exists() {
        std::fs::remove_dir_all(to)
            .map_err(|e| format!("could not clear {}: {e}", to.display()))?;
    }
    std::fs::create_dir_all(to).map_err(|e| format!("could not create {}: {e}", to.display()))?;
    let entries =
        std::fs::read_dir(from).map_err(|e| format!("could not read {}: {e}", from.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if let (true, Some(name)) = (path.is_file(), path.file_name()) {
            std::fs::copy(&path, to.join(name))
                .map_err(|e| format!("could not copy {}: {e}", path.display()))?;
        }
    }
    Ok(())
}

pub(crate) fn matrix_shard(
    repo: &Path,
    plan: &Plan,
    index: usize,
) -> Result<Option<PathBuf>, String> {
    let Some(package) = &plan.matrix else {
        return Ok(None);
    };
    let planned = plan.shards.get(index).ok_or_else(|| {
        format!(
            "the plan has {} shards, not {}",
            plan.shards.len(),
            index + 1
        )
    })?;
    let measured = matrix::measure(
        repo,
        package,
        Some((planned.index, planned.count)),
        &plan.base,
    )?;
    let out = repo.join("target/mutation/shards").join(index.to_string());
    copy_flat(&measured, &out)?;
    Ok(Some(out))
}

#[derive(Deserialize)]
struct NextestConfig {
    #[serde(rename = "nextest-version")]
    version: String,
}

fn detect_run(repo: &Path, args: &[String]) -> Result<Run, String> {
    let usage = "usage: xtask mutants merge --run-id <n> --run-url <url> --date <YYYY-MM-DD> --branch <name>";
    let mut given: BTreeMap<&str, &str> = BTreeMap::new();
    let mut rest = args.iter();
    while let Some(flag) = rest.next() {
        let name = flag
            .strip_prefix("--")
            .filter(|name| ["run-id", "run-url", "date", "branch"].contains(name));
        let (Some(name), Some(value)) = (name, rest.next()) else {
            return Err(usage.to_owned());
        };
        given.insert(name, value);
    }
    let need = |name: &str| given.get(name).copied().ok_or_else(|| usage.to_owned());
    let id = need("run-id")?
        .parse::<u64>()
        .map_err(|_| "--run-id must be a number".to_owned())?;
    let date = need("date")?;
    if !floors::is_date(date) {
        return Err(format!("--date must be written YYYY-MM-DD, not {date:?}"));
    }
    let config_path = repo.join(".config/nextest.toml");
    let nextest: NextestConfig = toml::from_str(
        &std::fs::read_to_string(&config_path)
            .map_err(|e| format!("could not read {}: {e}", config_path.display()))?,
    )
    .map_err(|e| {
        format!(
            "{} has no nextest-version string: {e}",
            config_path.display()
        )
    })?;
    let output = std::process::Command::new("rustc")
        .arg("--version")
        .current_dir(repo)
        .output()
        .map_err(|e| format!("could not run rustc: {e}"))?;
    if !output.status.success() {
        return Err("rustc --version failed".to_owned());
    }
    Ok(Run {
        id,
        url: need("run-url")?.to_owned(),
        date: date.to_owned(),
        branch: need("branch")?.to_owned(),
        cargo_mutants: tool::VERSION.to_owned(),
        nextest: nextest.version,
        toolchain: String::from_utf8_lossy(&output.stdout).trim().to_owned(),
    })
}

pub(crate) fn describe(found: &CrateResult) -> String {
    format!(
        "{}: {} ({} caught, {} missed, {} timeout, {} unviable)",
        found.package,
        found
            .score
            .map_or_else(|| "no score".to_owned(), floors::show),
        found.caught,
        found.missed,
        found.timeout,
        found.unviable
    )
}

pub(crate) fn merge_command(args: &[String]) -> Result<(), String> {
    let repo = crate::checks::root_of(Path::new("."))?;
    let run = detect_run(&repo, args)?;
    let plan = plan::read_plan(&repo)?;
    if plan.matrix.is_some() {
        return Err("this plan measures a kill matrix; merge it with matrix-merge".to_owned());
    }
    let results = verdict::read_results(&repo.join("target/mutation/shards"), &plan)?;
    let merged = merge(&plan, &results, &run);
    let previous = read_previous(&repo)?;
    let dir = results_dir(&repo);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .map_err(|e| format!("could not clear {}: {e}", dir.display()))?;
    }
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    let mut problems = merged.problems;
    for found in &merged.results {
        write_json(&dir.join(format!("{}.json", found.package)), found)?;
        println!("{}", describe(found));
        problems.extend(disagreements(found, &previous));
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::floors::{self, tests::measured, tests::unmeasured};
    use crate::metadata::{Dependency, Kind as DependencyKind, Package};
    use crate::mutation::plan::{Baseline, ShardPlan, Unit};
    use crate::scope::tests::Repo;

    pub(crate) fn run() -> Run {
        Run {
            id: 7,
            url: "https://example.test/runs/7".to_owned(),
            date: "2026-12-07".to_owned(),
            branch: "main".to_owned(),
            cargo_mutants: "27.1.0".to_owned(),
            nextest: "0.9.143".to_owned(),
            toolchain: "rustc 1.100.0-nightly (5db7f4be8 2026-09-01)".to_owned(),
        }
    }

    pub(crate) fn result(
        package: &str,
        commit: &str,
        branch: &str,
        run_id: u64,
        score: u64,
    ) -> CrateResult {
        CrateResult {
            package: package.to_owned(),
            commit: commit.to_owned(),
            branch: branch.to_owned(),
            run_id,
            run_url: format!("https://example.test/runs/{run_id}"),
            date: "2026-12-07".to_owned(),
            cargo_mutants: "27.1.0".to_owned(),
            nextest: "0.9.143".to_owned(),
            toolchain: "rustc 1.100.0-nightly (5db7f4be8 2026-09-01)".to_owned(),
            caught: score,
            missed: 100 - score,
            timeout: 0,
            unviable: 3,
            score: Some(score),
            outcomes: BTreeMap::new(),
        }
    }

    fn artifact(package: &str, created_at: &str, branch: &str, expired: bool) -> Artifact {
        Artifact {
            name: format!("{ARTIFACT_PREFIX}{package}-{}", "a".repeat(40)),
            expired,
            created_at: created_at.to_owned(),
            branch: branch.to_owned(),
        }
    }

    fn workspace(rows: &[(&str, &[&str])]) -> Workspace {
        Workspace {
            packages: rows
                .iter()
                .map(|(name, dependencies)| Package {
                    name: (*name).to_owned(),
                    dir: format!("crates/{name}"),
                    edition: "2024".to_owned(),
                    dependencies: dependencies
                        .iter()
                        .map(|dependency| Dependency {
                            name: (*dependency).to_owned(),
                            kind: DependencyKind::Normal,
                            local: true,
                        })
                        .collect(),
                    targets: Vec::new(),
                })
                .collect(),
        }
    }

    fn names(chosen: Chosen) -> Vec<String> {
        match chosen {
            Chosen::Sweep(names) => names,
            Chosen::Matrix { .. } => panic!("a matrix was chosen"),
        }
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "a table of histories and budgets with the crates each sweep takes"
    )]
    fn the_sweep_plan_takes_the_stalest_crates_within_the_budget() {
        struct Row<'a> {
            name: &'a str,
            floored: &'a [&'a str],
            history: Vec<Artifact>,
            counts: &'a [(&'a str, usize)],
            budget: usize,
            taken: &'a [&'a str],
        }
        let rows = [
            Row {
                name: "floored crates come first",
                floored: &["b"],
                history: vec![],
                counts: &[("a", 10), ("b", 10), ("c", 10)],
                budget: 100,
                taken: &["b", "a", "c"],
            },
            Row {
                name: "a never-swept crate precedes an old sweep",
                floored: &[],
                history: vec![
                    artifact("a", "2026-01-01T00:00:00Z", "main", false),
                    artifact("b", "2026-02-01T00:00:00Z", "main", false),
                ],
                counts: &[("a", 10), ("b", 10), ("c", 10)],
                budget: 100,
                taken: &["c", "a", "b"],
            },
            Row {
                name: "an older sweep precedes a newer one",
                floored: &[],
                history: vec![
                    artifact("a", "2026-03-01T00:00:00Z", "main", false),
                    artifact("b", "2026-01-01T00:00:00Z", "main", false),
                ],
                counts: &[("a", 10), ("b", 10)],
                budget: 100,
                taken: &["b", "a"],
            },
            Row {
                name: "the newest of several sweeps of one crate counts",
                floored: &[],
                history: vec![
                    artifact("a", "2026-01-01T00:00:00Z", "main", false),
                    artifact("a", "2026-04-01T00:00:00Z", "main", false),
                    artifact("b", "2026-03-01T00:00:00Z", "main", false),
                ],
                counts: &[("a", 10), ("b", 10)],
                budget: 100,
                taken: &["b", "a"],
            },
            Row {
                name: "crates are added while the sum stays within the budget",
                floored: &[],
                history: vec![],
                counts: &[("a", 40), ("b", 40), ("c", 40)],
                budget: 100,
                taken: &["a", "b"],
            },
            Row {
                name: "the sum may reach the budget exactly",
                floored: &[],
                history: vec![],
                counts: &[("a", 50), ("b", 50), ("c", 1)],
                budget: 100,
                taken: &["a", "b"],
            },
            Row {
                name: "a crate that does not fit ends the selection",
                floored: &[],
                history: vec![],
                counts: &[("a", 60), ("b", 60), ("c", 10)],
                budget: 100,
                taken: &["a"],
            },
            Row {
                name: "a first crate larger than the budget is taken alone",
                floored: &[],
                history: vec![],
                counts: &[("a", 500), ("b", 1)],
                budget: 100,
                taken: &["a"],
            },
            Row {
                name: "ties are broken by name",
                floored: &[],
                history: vec![],
                counts: &[("c", 1), ("a", 1), ("b", 1)],
                budget: 100,
                taken: &["a", "b", "c"],
            },
            Row {
                name: "a sweep of another branch does not count",
                floored: &[],
                history: vec![
                    artifact("a", "2026-05-01T00:00:00Z", "feature", false),
                    artifact("b", "2026-01-01T00:00:00Z", "main", false),
                ],
                counts: &[("a", 10), ("b", 10)],
                budget: 100,
                taken: &["a", "b"],
            },
            Row {
                name: "an expired artifact does not count",
                floored: &[],
                history: vec![
                    artifact("a", "2026-05-01T00:00:00Z", "main", true),
                    artifact("b", "2026-01-01T00:00:00Z", "main", false),
                ],
                counts: &[("a", 10), ("b", 10)],
                budget: 100,
                taken: &["a", "b"],
            },
        ];
        let mut wrong = Vec::new();
        for row in rows {
            let packages: Vec<&str> = row.counts.iter().map(|(name, _)| *name).collect();
            let text: String = packages
                .iter()
                .map(|name| {
                    if row.floored.contains(name) {
                        measured(name, "0.80", (80, 0, 20)).text()
                    } else {
                        unmeasured(name)
                    }
                })
                .collect();
            let floors = floors::parse(&text).unwrap();
            let workspace = floors::tests::workspace_of(&packages);
            let counts: BTreeMap<&str, usize> = row.counts.iter().copied().collect();
            let found = choose(
                &workspace,
                &floors,
                &row.history,
                None,
                Kind::Sweep,
                row.budget,
                |name| Ok(counts[name]),
            )
            .map(names);
            let wanted: Vec<String> = row.taken.iter().map(|n| (*n).to_owned()).collect();
            if found.as_ref() != Ok(&wanted) {
                wrong.push(format!(
                    "{}: expected {wanted:?}, found {found:?}",
                    row.name
                ));
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }

    #[test]
    fn a_dispatched_crate_is_swept_alone_whatever_the_budget() {
        let workspace = workspace(&[
            ("a", &["b", "c"]),
            ("b", &[]),
            ("c", &[]),
            ("e", &["a"]),
            ("big", &[]),
        ]);
        let floors = floors::parse(&["a", "b", "c", "e", "big"].map(unmeasured).concat()).unwrap();
        let unlisted = |name: &str| -> Result<usize, String> {
            Err(format!("{name} must not be listed to be dispatched"))
        };
        let swept = choose(
            &workspace,
            &floors,
            &[],
            Some("big"),
            Kind::Sweep,
            100,
            unlisted,
        );
        assert_eq!(swept, Ok(Chosen::Sweep(vec!["big".to_owned()])));

        let unknown = choose(
            &workspace,
            &floors,
            &[],
            Some("nope"),
            Kind::Sweep,
            100,
            unlisted,
        )
        .unwrap_err();
        assert!(unknown.contains("nope"), "{unknown}");

        let anonymous =
            choose(&workspace, &floors, &[], None, Kind::Matrix, 100, unlisted).unwrap_err();
        assert!(anonymous.contains("crate"), "{anonymous}");

        let Chosen::Matrix { package, passes } = choose(
            &workspace,
            &floors,
            &[],
            Some("a"),
            Kind::Matrix,
            100,
            unlisted,
        )
        .unwrap() else {
            panic!("a matrix was dispatched");
        };
        assert_eq!(package, "a");
        let shape: Vec<(&str, &str, Vec<&str>)> = passes
            .iter()
            .map(|pass| {
                (
                    pass.name.as_str(),
                    pass.package.as_str(),
                    pass.test_packages.iter().map(String::as_str).collect(),
                )
            })
            .collect();
        assert_eq!(
            shape,
            [
                ("pass-a", "a", vec!["a", "e"]),
                ("pass-b-b", "b", vec!["a"]),
                ("pass-b-c", "c", vec!["a"]),
            ]
        );

        let unknown_matrix = choose(
            &workspace,
            &floors,
            &[],
            Some("nope"),
            Kind::Matrix,
            100,
            unlisted,
        )
        .unwrap_err();
        assert!(unknown_matrix.contains("nope"), "{unknown_matrix}");
    }

    fn sweep_plan() -> Plan {
        let unit = |package: &str, names: &[&str], floor| Unit {
            package: package.to_owned(),
            test_packages: vec![package.to_owned()],
            names: names.iter().map(|n| (*n).to_owned()).collect(),
            floor: Some(floor),
        };
        Plan {
            base: "c".repeat(40),
            head: "c".repeat(40),
            in_force: true,
            baseline: Baseline::Run,
            diff: String::new(),
            units: vec![unit("x", &["x1", "x2", "x3"], 0), unit("y", &["y1"], 80)],
            shards: vec![
                ShardPlan {
                    unit: 0,
                    index: 0,
                    count: 2,
                },
                ShardPlan {
                    unit: 0,
                    index: 1,
                    count: 2,
                },
                ShardPlan {
                    unit: 1,
                    index: 0,
                    count: 1,
                },
            ],
            broken: Vec::new(),
            surface: Vec::new(),
            matrix: None,
        }
    }

    fn measured_mutant(name: &str, class: Class) -> Measured {
        Measured {
            name: name.to_owned(),
            class,
            log_path: format!("log/{name}.log"),
        }
    }

    fn all_shards() -> BTreeMap<usize, Vec<Measured>> {
        BTreeMap::from([
            (
                0,
                vec![
                    measured_mutant("x1", Class::Caught),
                    measured_mutant("x2", Class::Missed),
                ],
            ),
            (1, vec![measured_mutant("x3", Class::Timeout)]),
            (2, vec![measured_mutant("y1", Class::Unviable)]),
        ])
    }

    #[test]
    fn a_sweep_result_needs_every_shard_and_every_listed_mutant() {
        let plan = sweep_plan();
        let complete = merge(&plan, &all_shards(), &run());
        assert!(complete.problems.is_empty(), "{:?}", complete.problems);
        let by_crate: BTreeMap<&str, &CrateResult> = complete
            .results
            .iter()
            .map(|r| (r.package.as_str(), r))
            .collect();
        let x = by_crate["x"];
        assert_eq!(
            (x.caught, x.timeout, x.missed, x.unviable, x.score),
            (1, 1, 1, 0, Some(66))
        );
        assert_eq!(
            (
                x.commit.as_str(),
                x.run_id,
                x.run_url.as_str(),
                x.date.as_str()
            ),
            (
                plan.head.as_str(),
                7,
                "https://example.test/runs/7",
                "2026-12-07"
            )
        );
        assert_eq!(
            (
                x.cargo_mutants.as_str(),
                x.nextest.as_str(),
                x.branch.as_str()
            ),
            ("27.1.0", "0.9.143", "main")
        );
        assert_eq!(
            x.outcomes,
            BTreeMap::from([
                ("x1".to_owned(), Class::Caught),
                ("x2".to_owned(), Class::Missed),
                ("x3".to_owned(), Class::Timeout),
            ])
        );
        assert_eq!((by_crate["y"].unviable, by_crate["y"].score), (1, None));

        let mut without_shard = all_shards();
        without_shard.remove(&1);
        let found = merge(&plan, &without_shard, &run());
        assert_eq!(found.problems, ["x: shard 1 is missing"]);
        assert_eq!(
            found
                .results
                .iter()
                .map(|r| r.package.as_str())
                .collect::<Vec<_>>(),
            ["y"]
        );

        let mut without_mutant = all_shards();
        without_mutant.insert(1, Vec::new());
        let found = merge(&plan, &without_mutant, &run());
        assert_eq!(found.problems, ["x: x3 was not run"]);

        let mut extra = all_shards();
        extra
            .get_mut(&2)
            .unwrap()
            .push(measured_mutant("y2", Class::Caught));
        let found = merge(&plan, &extra, &run());
        assert_eq!(
            found.problems,
            ["y: y2 was run, but the plan does not list it"]
        );
    }

    #[test]
    fn two_sweeps_of_one_commit_must_agree() {
        let with = |commit: &str, package: &str, outcomes: &[(&str, Class)]| CrateResult {
            outcomes: outcomes
                .iter()
                .map(|(n, c)| ((*n).to_owned(), *c))
                .collect(),
            ..result(package, commit, "main", 1, 90)
        };
        let both = [("m1", Class::Caught), ("m2", Class::Missed)];
        let flipped = [("m1", Class::Caught), ("m2", Class::Caught)];
        let current = with("same", "x", &both);

        assert!(disagreements(&current, &[with("same", "x", &both)]).is_empty());
        assert!(disagreements(&current, &[]).is_empty());
        assert!(disagreements(&current, &[with("other", "x", &flipped)]).is_empty());
        assert!(disagreements(&current, &[with("same", "y", &flipped)]).is_empty());

        let found = disagreements(&current, &[with("same", "x", &flipped)]);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].contains("m2") && found[0].contains('x') && found[0].contains("missed"),
            "{found:?}"
        );

        let wide = with(
            "same",
            "x",
            &[
                ("m1", Class::Missed),
                ("m2", Class::Caught),
                ("m3", Class::Caught),
            ],
        );
        let found = disagreements(&wide, &[with("same", "x", &both)]);
        for name in ["m1", "m2", "m3"] {
            assert!(
                found.iter().any(|line| line.contains(name)),
                "{name} in {found:?}"
            );
        }
    }

    #[test]
    fn the_previous_index_is_written_even_on_a_first_sweep() {
        let repo = Repo::new("previous-index");
        assert_eq!(index_previous(&repo.0).unwrap(), []);
        let index = repo.0.join(SWEEP_DIR).join("previous/index.json");
        let text = std::fs::read_to_string(&index).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&text).unwrap(),
            serde_json::json!([])
        );
        assert_eq!(read_previous(&repo.0).unwrap(), []);

        for (package, commit, run_id) in [("b", "bbb", 5), ("a", "aaa", 4)] {
            let found = result(package, commit, "main", run_id, 90);
            repo.write(
                &format!("{SWEEP_DIR}/previous/{package}.json"),
                &serde_json::to_string(&found).unwrap(),
            );
        }
        let listed = index_previous(&repo.0).unwrap();
        assert_eq!(
            listed,
            [
                Previous {
                    package: "a".to_owned(),
                    commit: "aaa".to_owned(),
                    run_id: 4,
                    file: "a.json".to_owned()
                },
                Previous {
                    package: "b".to_owned(),
                    commit: "bbb".to_owned(),
                    run_id: 5,
                    file: "b.json".to_owned()
                },
            ]
        );
        let read = read_previous(&repo.0).unwrap();
        assert_eq!(
            read.iter().map(|r| r.package.as_str()).collect::<Vec<_>>(),
            ["a", "b"]
        );
    }

    mod cargo_mutants {
        use super::*;
        use crate::mutation::matrix::tests::cargo_mutants::floor_workspace;

        #[test]
        fn a_dispatched_sweep_runs_through_plan_shards_and_merge() {
            let repo = floor_workspace("dispatched-sweep");
            let head = repo.sha();
            let workspace = Workspace::load(&repo.0).unwrap();
            let floors = floors::parse(&["low", "high"].map(unmeasured).concat()).unwrap();
            let chosen = choose(
                &workspace,
                &floors,
                &[],
                Some("low"),
                Kind::Sweep,
                100,
                |_| Ok(0),
            )
            .unwrap();
            let base = config::base_config(&repo.0, &head, &Head::Commit(head.clone())).unwrap();
            let gate = config::write_gate(&repo.0, &base).unwrap();
            let plan = assemble(&workspace, &floors, &head, &chosen, |package| {
                let listed = config::list_with(&repo.0, &gate, package, None, &[])?;
                Ok(listed.into_iter().map(|mutant| mutant.name).collect())
            })
            .unwrap();
            plan::write_plan(&repo.0, &plan).unwrap();
            let tree = crate::mutation::worktree::checkout(&repo.0);
            for index in 0..plan.shards.len() {
                crate::mutation::run::shard(&tree, &repo.0, &plan, index).unwrap();
            }
            let results = verdict::read_results(&tree.out_dir, &plan).unwrap();
            let merged = merge(&plan, &results, &run());
            assert!(merged.problems.is_empty(), "{:?}", merged.problems);
            let [found] = merged.results.as_slice() else {
                panic!("one crate was swept: {:?}", merged.results);
            };
            assert_eq!(found.package, "low");
            assert_eq!(found.commit, head);
            assert_eq!(found.missed, 0, "{found:?}");
            assert_eq!(found.score, Some(100), "{found:?}");
            assert_eq!(found.outcomes.len(), plan.units[0].names.len());
        }

        #[test]
        fn a_dispatched_matrix_runs_through_plan_shards_and_merge() {
            let repo = floor_workspace("dispatched-matrix");
            let head = repo.sha();
            let workspace = Workspace::load(&repo.0).unwrap();
            let floors = floors::parse(&["low", "high"].map(unmeasured).concat()).unwrap();
            let chosen = choose(
                &workspace,
                &floors,
                &[],
                Some("high"),
                Kind::Matrix,
                100,
                |_| Ok(0),
            )
            .unwrap();
            let base = config::base_config(&repo.0, &head, &Head::Commit(head.clone())).unwrap();
            let gate = config::write_gate(&repo.0, &base).unwrap();
            let plan = assemble(&workspace, &floors, &head, &chosen, |package| {
                let listed = config::list_with(&repo.0, &gate, package, None, &[])?;
                Ok(listed.into_iter().map(|mutant| mutant.name).collect())
            })
            .unwrap();
            assert_eq!(plan.matrix.as_deref(), Some("high"));
            let packages: Vec<&str> = plan.units.iter().map(|u| u.package.as_str()).collect();
            assert_eq!(packages, ["high", "low"]);
            assert!(!plan.shards.is_empty());

            for index in 0..plan.shards.len() {
                let out = matrix_shard(&repo.0, &plan, index).unwrap().unwrap();
                assert!(out.join("pass-a.json").exists(), "{}", out.display());
                assert!(out.join("pass-b-low.json").exists(), "{}", out.display());
            }
            let dirs = matrix::shard_dirs(&repo.0.join("target/mutation/shards")).unwrap();
            assert_eq!(dirs.len(), plan.shards.len());
            let merged = matrix::merge_dirs(&repo.0, "high", &dirs).unwrap();
            let status = |name: &str| {
                merged
                    .tests
                    .iter()
                    .find(|row| row.id.name == name)
                    .map(|row| row.status)
            };
            assert_eq!(
                status("tests::doubling_twice_quadruples"),
                Some(matrix::Status::Killing)
            );
            assert_eq!(status("tests::always_true"), Some(matrix::Status::ZeroKill));
        }
    }
}
