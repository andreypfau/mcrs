use crate::floors::{self, Floors};
use crate::metadata::Workspace;
use crate::mutation::config::{self, CONFIG_FILE, Mutant};
use crate::mutation::{diff, surface, tool};
use crate::scope::{self, Head, Scope};
use regex::RegexSet;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub(crate) const MAX_SHARDS: usize = 256;
pub(crate) const PLAN_DIR: &str = "target/mutation/plan";

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Baseline {
    Run,
    Skip,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub(crate) struct Plan {
    pub base: String,
    pub head: String,
    pub in_force: bool,
    pub baseline: Baseline,
    pub diff: String,
    pub units: Vec<Unit>,
    pub shards: Vec<ShardPlan>,
    pub broken: Vec<Broken>,
    pub surface: Vec<String>,
    pub matrix: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Broken {
    pub package: String,
    pub file: String,
    pub line: u32,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub(crate) struct Unit {
    pub package: String,
    pub test_packages: Vec<String>,
    pub names: Vec<String>,
    pub floor: Option<u64>,
}

impl Unit {
    pub(crate) fn has_dependents(&self) -> bool {
        self.test_packages.iter().any(|name| *name != self.package)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ShardPlan {
    pub unit: usize,
    pub index: u32,
    pub count: u32,
}

impl Plan {
    pub(crate) fn total_mutants(&self) -> usize {
        self.units
            .iter()
            .filter(|unit| unit.floor.is_none())
            .map(|unit| unit.names.len())
            .sum()
    }
}

pub(crate) fn split(counts: &[usize], size: usize) -> Vec<ShardPlan> {
    let total: usize = counts.iter().sum();
    let largest = counts.iter().copied().max().unwrap_or(0);
    let jobs = |size: usize| counts.iter().map(|n| n.div_ceil(size)).sum::<usize>();
    let mut size = size.max(1).max(total.div_ceil(MAX_SHARDS));
    while jobs(size) > MAX_SHARDS && size < largest {
        size += 1;
    }
    counts
        .iter()
        .enumerate()
        .flat_map(|(unit, n)| {
            let count = n.div_ceil(size) as u32;
            (0..count).map(move |index| ShardPlan { unit, index, count })
        })
        .collect()
}

pub(crate) fn uncovered_lines(
    unfiltered: &[Mutant],
    added: &BTreeMap<String, BTreeSet<u32>>,
    in_diff: &BTreeSet<&str>,
    excluded: &RegexSet,
) -> Vec<(String, u32)> {
    let mut by_file: BTreeMap<&str, Vec<&Mutant>> = BTreeMap::new();
    for mutant in unfiltered {
        by_file.entry(&mutant.file).or_default().push(mutant);
    }
    let mut found = Vec::new();
    for (file, numbers) in added {
        let Some(mutants) = by_file.get(file.as_str()) else {
            continue;
        };
        for &line in numbers {
            let mut live = mutants
                .iter()
                .filter(|m| m.span.start.line <= line && line <= m.span.end.line)
                .filter(|m| !excluded.is_match(&m.name))
                .peekable();
            if live.peek().is_some() && !live.any(|m| in_diff.contains(m.name.as_str())) {
                found.push((file.clone(), line));
            }
        }
    }
    found
}

pub(crate) struct Inputs<'a> {
    pub repo: &'a Path,
    pub tree: &'a Path,
    pub workspace: &'a Workspace,
    pub base: &'a str,
    pub head: &'a str,
    pub baseline: Baseline,
    pub shard_size: usize,
    pub sweep: bool,
}

pub(crate) fn sweeps(
    base: Option<&Floors>,
    head: Option<&Floors>,
    changed: &BTreeSet<String>,
    workspace: &Workspace,
) -> BTreeMap<String, u64> {
    let at_base = |name: &str| base.and_then(|floors| floors.hundredths(name));
    let at_head = |name: &str| head.and_then(|floors| floors.hundredths(name));
    let mut swept = BTreeMap::new();
    let mut add = |name: &str| {
        if let Some(floor) = at_base(name) {
            swept.insert(name.to_owned(), floor.max(at_head(name).unwrap_or(0)));
        }
    };
    for package in workspace
        .packages
        .iter()
        .filter(|p| changed.contains(&p.name))
    {
        add(&package.name);
        for dependency in package.dependencies.iter().filter(|d| d.local) {
            add(&dependency.name);
        }
    }
    for (name, floor) in head.into_iter().flat_map(Floors::measured) {
        let new_or_raised = at_base(name).is_none_or(|before| floor > before);
        if new_or_raised && workspace.packages.iter().any(|p| p.name == name) {
            swept.insert(name.to_owned(), floor);
        }
    }
    swept
}

pub(crate) fn write_patch(repo: &Path, plan: &Plan) -> Result<PathBuf, String> {
    let dir = repo.join(PLAN_DIR);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    let path = dir.join("change.diff");
    std::fs::write(&path, &plan.diff)
        .map_err(|e| format!("could not write {}: {e}", path.display()))?;
    Ok(path)
}

pub(crate) fn write_plan(repo: &Path, plan: &Plan) -> Result<PathBuf, String> {
    let dir = repo.join(PLAN_DIR);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    let path = dir.join("plan.json");
    let text = serde_json::to_string_pretty(plan).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("could not write {}: {e}", path.display()))?;
    Ok(path)
}

pub(crate) fn read_plan(repo: &Path) -> Result<Plan, String> {
    let path = repo.join(PLAN_DIR).join("plan.json");
    let text = std::fs::read_to_string(&path).map_err(|e| {
        format!(
            "could not read {}: {e}; run xtask mutants plan first, or download the plan artifact",
            path.display()
        )
    })?;
    serde_json::from_str(&text).map_err(|e| format!("{} is not a plan: {e}", path.display()))
}

pub(crate) fn test_packages(workspace: &Workspace, package: &str) -> Result<Vec<String>, String> {
    let Scope::Packages(packages) =
        Scope::Packages(vec![package.to_owned()]).with_dependents(workspace)
    else {
        return Err("the test scope of a package is a list of packages".to_owned());
    };
    Ok(packages)
}

pub(crate) fn build(inputs: &Inputs) -> Result<Plan, String> {
    let Inputs {
        repo,
        tree,
        workspace,
        base,
        head,
        baseline,
        shard_size,
        sweep,
    } = *inputs;
    let head_ref = Head::Commit(head.to_owned());
    let in_force = scope::configuration_at(repo, base, &head_ref, CONFIG_FILE)?.is_some();
    let changes = diff::build(repo, base, head)?;
    let mut files: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for path in changes.added.keys() {
        if let Some(owner) = workspace.owner(path) {
            files.entry(&owner.name).or_default().push(path.clone());
        }
    }
    let (surface, swept) = if sweep {
        let surface = surface::changed_crates(repo, workspace, base, head)?;
        let (base_floors, head_floors) = floors::pair(repo, base, &head_ref)?;
        let swept = sweeps(
            base_floors.as_ref(),
            head_floors.as_ref(),
            &surface,
            workspace,
        );
        (surface, swept)
    } else {
        (BTreeSet::new(), BTreeMap::new())
    };
    let mut plan = Plan {
        base: base.to_owned(),
        head: head.to_owned(),
        in_force,
        baseline,
        diff: changes.patch,
        units: Vec::new(),
        shards: Vec::new(),
        broken: Vec::new(),
        surface: surface.into_iter().collect(),
        matrix: None,
    };
    if files.is_empty() && swept.is_empty() {
        return Ok(plan);
    }
    tool::require_version(repo)?;
    let base_config = config::base_config(repo, base, &head_ref)?;
    let gate = config::write_gate(repo, &base_config)?;
    let listing = config::write_listing(repo, &base_config)?;
    let excluded = RegexSet::new(base_config.exclusions())
        .map_err(|e| format!("the base exclusions are not regular expressions: {e}"))?;
    let patch = write_patch(repo, &plan)?;
    for (package, changed_files) in &files {
        let listed = config::list_with(tree, &gate, package, Some(&patch), &[])?;
        let unfiltered = config::list_with(tree, &listing, package, None, changed_files)?;
        let in_diff: BTreeSet<&str> = listed.iter().map(|m| m.name.as_str()).collect();
        let known: BTreeSet<&str> = unfiltered.iter().map(|m| m.name.as_str()).collect();
        let added: BTreeMap<String, BTreeSet<u32>> = changed_files
            .iter()
            .filter_map(|file| Some((file.clone(), changes.added.get(file)?.clone())))
            .collect();
        let mut broken: Vec<(String, u32)> =
            uncovered_lines(&unfiltered, &added, &in_diff, &excluded);
        broken.extend(
            listed
                .iter()
                .filter(|m| !known.contains(m.name.as_str()))
                .map(|m| (m.file.clone(), m.span.start.line)),
        );
        broken.sort();
        broken.dedup();
        plan.broken
            .extend(broken.into_iter().map(|(file, line)| Broken {
                package: (*package).to_owned(),
                file,
                line,
            }));
        plan.units.push(Unit {
            package: (*package).to_owned(),
            test_packages: test_packages(workspace, package)?,
            names: in_diff.iter().map(|name| (*name).to_owned()).collect(),
            floor: None,
        });
    }
    for (package, floor) in &swept {
        let listed = config::list_with(tree, &gate, package, None, &[])?;
        let names: BTreeSet<&str> = listed.iter().map(|m| m.name.as_str()).collect();
        plan.units.push(Unit {
            package: package.clone(),
            test_packages: test_packages(workspace, package)?,
            names: names.into_iter().map(str::to_owned).collect(),
            floor: Some(*floor),
        });
    }
    let counts: Vec<usize> = plan.units.iter().map(|unit| unit.names.len()).collect();
    plan.shards = split(&counts, shard_size);
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mutation::config::{Position, Span};

    fn mutant(name: &str, file: &str, from: u32, to: u32) -> Mutant {
        Mutant {
            name: name.to_owned(),
            package: "p".to_owned(),
            file: file.to_owned(),
            function: None,
            span: Span {
                start: Position { line: from },
                end: Position { line: to },
            },
        }
    }

    fn lines(rows: &[(&str, &[u32])]) -> BTreeMap<String, BTreeSet<u32>> {
        rows.iter()
            .map(|(file, numbers)| ((*file).to_owned(), numbers.iter().copied().collect()))
            .collect()
    }

    #[test]
    fn a_changed_body_line_with_no_listed_mutant_fails() {
        const F: &str = "src/a.rs:5:5: replace f -> i32 with 0";
        const G: &str = "src/a.rs:6:9: replace + with - in f";
        let unfiltered = [mutant(F, "src/a.rs", 5, 7), mutant(G, "src/a.rs", 6, 6)];
        let nothing = RegexSet::empty();
        let g_excluded = RegexSet::new([r"^src/a\.rs:\d+:\d+: replace \+ with - in f$"]).unwrap();
        let both_excluded = RegexSet::new([r"^src/a\.rs:\d+:\d+: replace "]).unwrap();
        let listed =
            |names: &[&'static str]| -> BTreeSet<&'static str> { names.iter().copied().collect() };
        struct Row<'a> {
            name: &'a str,
            added: BTreeMap<String, BTreeSet<u32>>,
            in_diff: BTreeSet<&'a str>,
            excluded: &'a RegexSet,
            uncovered: Vec<(&'a str, u32)>,
        }
        let rows = [
            Row {
                name: "a line inside a span with a covering in-diff mutant",
                added: lines(&[("src/a.rs", &[6])]),
                in_diff: listed(&[F, G]),
                excluded: &nothing,
                uncovered: vec![],
            },
            Row {
                name: "the same line with an empty in-diff list",
                added: lines(&[("src/a.rs", &[6])]),
                in_diff: listed(&[]),
                excluded: &nothing,
                uncovered: vec![("src/a.rs", 6)],
            },
            Row {
                name: "a line whose only mutants match exclusions",
                added: lines(&[("src/a.rs", &[6])]),
                in_diff: listed(&[]),
                excluded: &both_excluded,
                uncovered: vec![],
            },
            Row {
                name: "a line covered by an excluded and a listed mutant, with the listed one run",
                added: lines(&[("src/a.rs", &[6])]),
                in_diff: listed(&[F]),
                excluded: &g_excluded,
                uncovered: vec![],
            },
            Row {
                name: "a line covered by an excluded and a live mutant, with nothing listed",
                added: lines(&[("src/a.rs", &[6])]),
                in_diff: listed(&[]),
                excluded: &g_excluded,
                uncovered: vec![("src/a.rs", 6)],
            },
            Row {
                name: "a line outside every span",
                added: lines(&[("src/a.rs", &[20])]),
                in_diff: listed(&[]),
                excluded: &nothing,
                uncovered: vec![],
            },
            Row {
                name: "a file without a listed mutant",
                added: lines(&[("src/b.rs", &[6])]),
                in_diff: listed(&[]),
                excluded: &nothing,
                uncovered: vec![],
            },
            Row {
                name: "two uncovered lines come back in order",
                added: lines(&[("src/a.rs", &[7, 5])]),
                in_diff: listed(&[]),
                excluded: &nothing,
                uncovered: vec![("src/a.rs", 5), ("src/a.rs", 7)],
            },
        ];
        let mut problems = Vec::new();
        for Row {
            name,
            added,
            in_diff,
            excluded,
            uncovered,
        } in rows
        {
            let found = uncovered_lines(&unfiltered, &added, &in_diff, excluded);
            let wanted: Vec<(String, u32)> = uncovered
                .into_iter()
                .map(|(file, line)| (file.to_owned(), line))
                .collect();
            if found != wanted {
                problems.push(format!("{name}: expected {wanted:?}, found {found:?}"));
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
        for (code, part) in [(5, "does not match the tree"), (6, "cannot be parsed")] {
            let hint = config::list_exit_hint(Some(code));
            assert!(hint.contains(part), "exit {code}: {hint}");
        }
        assert_eq!(config::list_exit_hint(Some(1)), "");
    }

    fn chain() -> Workspace {
        let package = |name: &str, deps: &[&str]| crate::metadata::Package {
            name: name.to_owned(),
            dir: format!("crates/{name}"),
            edition: "2024".to_owned(),
            dependencies: deps
                .iter()
                .map(|dep| crate::metadata::Dependency {
                    name: (*dep).to_owned(),
                    kind: crate::metadata::Kind::Normal,
                    local: true,
                })
                .collect(),
            targets: Vec::new(),
        };
        Workspace {
            packages: vec![
                package("low", &[]),
                package("mid", &["low"]),
                package("high", &["mid"]),
            ],
        }
    }

    #[test]
    fn a_test_change_sweeps_the_crate_and_its_floored_dependencies() {
        use crate::floors::tests::{measured, unmeasured};
        let at = |name: &str, floor: &str, counts| measured(name, floor, counts).text();
        let file = |parts: Vec<String>| floors::parse(&parts.concat()).unwrap();
        let eighty_three = (83, 0, 17);
        let eighty = (80, 0, 20);
        type Row<'a> = (
            &'a str,
            Option<Floors>,
            Option<Floors>,
            &'a [&'a str],
            Vec<(&'a str, u64)>,
        );
        let rows: Vec<Row> = vec![
            (
                "a floored crate whose tests changed",
                Some(file(vec![at("high", "0.83", eighty_three)])),
                Some(file(vec![at("high", "0.83", eighty_three)])),
                &["high"],
                vec![("high", 83)],
            ),
            (
                "an unmeasured crate with a floored direct dependency",
                Some(file(vec![
                    at("low", "0.83", eighty_three),
                    unmeasured("mid"),
                ])),
                Some(file(vec![
                    at("low", "0.83", eighty_three),
                    unmeasured("mid"),
                ])),
                &["mid"],
                vec![("low", 83)],
            ),
            (
                "a floored crate that is not a direct dependency",
                Some(file(vec![at("low", "0.83", eighty_three)])),
                Some(file(vec![at("low", "0.83", eighty_three)])),
                &["high"],
                vec![],
            ),
            (
                "a floored dependent is not swept",
                Some(file(vec![at("high", "0.83", eighty_three)])),
                Some(file(vec![at("high", "0.83", eighty_three)])),
                &["low"],
                vec![],
            ),
            (
                "nothing floored",
                Some(file(vec![unmeasured("low"), unmeasured("mid")])),
                Some(file(vec![unmeasured("low"), unmeasured("mid")])),
                &["mid", "low"],
                vec![],
            ),
            (
                "the crate and its dependency both floored",
                Some(file(vec![
                    at("low", "0.80", eighty),
                    at("mid", "0.83", eighty_three),
                ])),
                Some(file(vec![
                    at("low", "0.80", eighty),
                    at("mid", "0.83", eighty_three),
                ])),
                &["mid"],
                vec![("low", 80), ("mid", 83)],
            ),
            (
                "an enrolment with no test change",
                Some(file(vec![unmeasured("mid")])),
                Some(file(vec![at("mid", "0.83", eighty_three)])),
                &[],
                vec![("mid", 83)],
            ),
            (
                "a raise with no test change",
                Some(file(vec![at("low", "0.80", eighty)])),
                Some(file(vec![at("low", "0.83", eighty_three)])),
                &[],
                vec![("low", 83)],
            ),
            (
                "an unchanged floor with no test change",
                Some(file(vec![at("low", "0.83", eighty_three)])),
                Some(file(vec![at("low", "0.83", eighty_three)])),
                &[],
                vec![],
            ),
            (
                "a lowered head floor is swept at the base floor",
                Some(file(vec![at("low", "0.83", eighty_three)])),
                Some(file(vec![at("low", "0.80", eighty)])),
                &["low"],
                vec![("low", 83)],
            ),
            (
                "a floors file that is new at the head",
                None,
                Some(file(vec![at("low", "0.83", eighty_three)])),
                &[],
                vec![("low", 83)],
            ),
            (
                "a floor of a package outside the workspace",
                Some(file(vec![unmeasured("low")])),
                Some(file(vec![at("gone", "0.83", eighty_three)])),
                &[],
                vec![],
            ),
        ];
        let mut wrong = Vec::new();
        for (name, base, head, changed, expected) in rows {
            let changed: BTreeSet<String> = changed.iter().map(|c| (*c).to_owned()).collect();
            let found = sweeps(base.as_ref(), head.as_ref(), &changed, &chain());
            let wanted: BTreeMap<String, u64> = expected
                .into_iter()
                .map(|(name, floor)| (name.to_owned(), floor))
                .collect();
            if found != wanted {
                wrong.push(format!("{name}: expected {wanted:?}, found {found:?}"));
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }

    #[test]
    fn a_plan_splits_units_into_shards_of_the_measured_size() {
        let size = tool::MUTANTS_PER_SHARD;
        let shard = |unit, index, count| ShardPlan { unit, index, count };
        assert_eq!(split(&[], size), []);
        assert_eq!(split(&[0], size), []);
        assert_eq!(split(&[size], size), [shard(0, 0, 1)]);
        assert_eq!(split(&[size + 1], size), [shard(0, 0, 2), shard(0, 1, 2)]);
        assert_eq!(
            split(&[size + 1, 0, 1], size),
            [shard(0, 0, 2), shard(0, 1, 2), shard(2, 0, 1)]
        );
        assert_eq!(
            split(&[500, 7], usize::MAX),
            [shard(0, 0, 1), shard(1, 0, 1)]
        );
        for counts in [vec![size * 300], vec![size * 200, size * 200]] {
            let found = split(&counts, size);
            assert_eq!(found.len(), MAX_SHARDS, "{counts:?}");
            for (unit, &count) in counts.iter().enumerate() {
                let of_unit: Vec<_> = found.iter().filter(|s| s.unit == unit).collect();
                let shards = of_unit.len();
                assert!(
                    of_unit.iter().all(|s| s.count as usize == shards),
                    "{counts:?}"
                );
                let indexes: Vec<u32> = of_unit.iter().map(|s| s.index).collect();
                assert_eq!(indexes, (0..shards as u32).collect::<Vec<_>>());
                assert!(
                    count.div_ceil(shards) > size,
                    "the size was not raised: {counts:?}"
                );
            }
        }
    }

    mod cargo_mutants {
        use super::*;
        use crate::scope::tests::Repo;

        #[test]
        fn an_in_diff_that_does_not_match_the_tree_fails_the_listing() {
            let repo = Repo::new("diff-mismatch");
            repo.init();
            repo.write(
                "Cargo.toml",
                "[package]\nname = \"listed\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n",
            );
            repo.write(
                "src/lib.rs",
                "pub fn double(x: i32) -> i32 {\n    x * 2\n}\n",
            );
            repo.commit("base");
            let config = config::write_listing(&repo.0, &config::parse_shape(&format!("{}exclude_re = []\n", "test_tool = \"nextest\"\nprofile = \"mutants\"\nadditional_cargo_test_args = [\"--profile=mutants\", \"--no-tests=pass\"]\ngitignore = true\n")).unwrap()).unwrap();
            let patch = repo.0.join("mismatch.diff");
            std::fs::write(
                &patch,
                "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -2 +2 @@\n-    x * 2\n+    x * 3\n",
            )
            .unwrap();
            let found = config::list_with(&repo.0, &config, "listed", Some(&patch), &[]);
            let message = found.expect_err("the diff does not match the tree");
            assert!(message.contains("exit 5"), "{message}");
        }
    }
}
