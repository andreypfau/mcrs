use crate::mutation::outcomes::{self, Measured};
use crate::mutation::tool::{self, Mode, Spec};
use crate::mutation::worktree::Tree;
use crate::mutation::{config, plan};
use crate::scope::Head;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub(crate) struct Run<'a> {
    pub name: &'a str,
    pub package: &'a str,
    pub test_packages: &'a [String],
    pub test_timeout_seconds: u32,
    pub skip_baseline: bool,
    pub shard: Option<(u32, u32)>,
    pub in_diff: Option<&'a Path>,
    pub names: &'a [String],
}

pub(crate) const DEPENDENTS: &str = "dependents";

const NORMAL_EXITS: [i32; 3] = [0, 2, 3];

pub(crate) fn build_scope(tree: &Tree, packages: &[String]) -> Result<(), String> {
    let mut command = tool::cargo(&tree.path);
    command
        .args(["nextest", "run", "--no-run", "--cargo-profile", "mutants"])
        .env("CARGO_TARGET_DIR", &tree.target_dir);
    for package in packages {
        command.args(["-p", package]);
    }
    let status = command
        .status()
        .map_err(|e| format!("could not run cargo nextest: {e}; install cargo-nextest"))?;
    if status.success() {
        return Ok(());
    }
    Err(format!(
        "the unmutated build of {} failed ({status}); fix the build before measuring",
        packages.join(", ")
    ))
}

pub(crate) fn execute(tree: &Tree, config: &Path, run: &Run) -> Result<PathBuf, String> {
    build_scope(tree, run.test_packages)?;
    let output = tree.out_dir.join(run.name);
    if output.exists() {
        std::fs::remove_dir_all(&output)
            .map_err(|e| format!("could not clear {}: {e}", output.display()))?;
    }
    std::fs::create_dir_all(&output)
        .map_err(|e| format!("could not create {}: {e}", output.display()))?;
    let spec = Spec {
        tree: &tree.path,
        config,
        package: run.package,
        mode: Mode::Run {
            test_packages: run.test_packages,
            output: &output,
            skip_baseline: run.skip_baseline,
            test_timeout_seconds: run.test_timeout_seconds,
            names: run.names,
        },
        shard: run.shard,
        in_diff: run.in_diff,
        files: &[],
    };
    let mut command = tool::command(&spec);
    command.env("CARGO_TARGET_DIR", &tree.target_dir);
    let status = command
        .status()
        .map_err(|e| format!("could not run cargo mutants: {e}; install it and put it on PATH"))?;
    let out_dir = output.join("mutants.out");
    match status.code() {
        Some(code) if NORMAL_EXITS.contains(&code) => Ok(out_dir),
        Some(code) => Err(format!(
            "cargo mutants for {} exited {code}; the end of {}:\n{}",
            run.package,
            out_dir.join("debug.log").display(),
            last_lines(&out_dir.join("debug.log"), 8)
        )),
        None => Err(format!(
            "cargo mutants for {} was ended by a signal ({status}); run it again",
            run.package
        )),
    }
}

pub(crate) fn shard(
    tree: &Tree,
    repo: &Path,
    plan: &plan::Plan,
    index: usize,
) -> Result<PathBuf, String> {
    let planned = plan.shards.get(index).ok_or_else(|| {
        format!(
            "the plan has {} shards, not {}",
            plan.shards.len(),
            index + 1
        )
    })?;
    let unit = &plan.units[planned.unit];
    let base_config = config::base_config(repo, &plan.base, &Head::Commit(plan.head.clone()))?;
    let config = config::write_gate(repo, &base_config)?;
    let patch = plan::write_patch(repo, plan)?;
    let name = index.to_string();
    let in_diff = unit.floor.is_none().then_some(patch.as_path());
    let skip_baseline = plan.baseline == plan::Baseline::Skip && unit.floor.is_none();
    let own = std::slice::from_ref(&unit.package);
    let first = execute(
        tree,
        &config,
        &Run {
            name: &name,
            package: &unit.package,
            test_packages: own,
            test_timeout_seconds: tool::TEST_TIMEOUT_SECONDS,
            skip_baseline,
            shard: Some((planned.index, planned.count)),
            in_diff,
            names: &[],
        },
    )?;
    if !unit.has_dependents() {
        return Ok(first);
    }
    let survivors = outcomes::missed_names(&measured(&first)?);
    if survivors.is_empty() {
        return Ok(first);
    }
    let listed = config::list_named(&tree.path, &config, &unit.package, in_diff, &survivors)?;
    if let Some(difference) = listing_difference(&listed, &survivors) {
        return Err(format!(
            "the dependents pass of shard {index} ({}) would not test the survivors of the first pass: {difference}",
            unit.package
        ));
    }
    execute(
        tree,
        &config,
        &Run {
            name: &format!("{name}/{DEPENDENTS}"),
            package: &unit.package,
            test_packages: &unit.test_packages,
            test_timeout_seconds: tool::TEST_TIMEOUT_SECONDS,
            skip_baseline,
            shard: Some((0, 1)),
            in_diff,
            names: &survivors,
        },
    )?;
    Ok(first)
}

fn listing_difference(listed: &[String], survivors: &[String]) -> Option<String> {
    let listed: BTreeSet<&str> = listed.iter().map(String::as_str).collect();
    let survivors: BTreeSet<&str> = survivors.iter().map(String::as_str).collect();
    let mut problems: Vec<String> = survivors
        .difference(&listed)
        .map(|name| format!("the listing does not list the survivor {name}"))
        .collect();
    problems.extend(
        listed
            .difference(&survivors)
            .map(|name| format!("the listing names {name}, which is no survivor")),
    );
    (!problems.is_empty()).then(|| problems.join("; "))
}

fn last_lines(path: &Path, count: usize) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(count)..].join("\n")
}

pub(crate) fn measured(out_dir: &Path) -> Result<Vec<Measured>, String> {
    if !out_dir.join("outcomes.json").exists() {
        return Ok(Vec::new());
    }
    outcomes::load_measured(out_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dependents_pass_must_list_exactly_the_survivors() {
        const ONE: &str = "src/a.rs:3:5: replace f -> bool with true";
        const TWO: &str = "src/a.rs:9:5: replace g -> bool with false";
        let names =
            |rows: &[&str]| -> Vec<String> { rows.iter().map(|r| (*r).to_owned()).collect() };
        type Row<'a> = (&'a str, Vec<String>, Vec<String>, Vec<&'a str>);
        let rows: [Row; 5] = [
            (
                "the same mutants",
                names(&[ONE, TWO]),
                names(&[ONE, TWO]),
                vec![],
            ),
            (
                "the same mutants in another order",
                names(&[TWO, ONE]),
                names(&[ONE, TWO]),
                vec![],
            ),
            (
                "a survivor the listing lacks",
                names(&[ONE]),
                names(&[ONE, TWO]),
                vec![TWO, "does not list"],
            ),
            (
                "a mutant the survivors lack",
                names(&[ONE, TWO]),
                names(&[ONE]),
                vec![TWO, "no survivor"],
            ),
            (
                "an empty listing",
                Vec::new(),
                names(&[ONE]),
                vec![ONE, "does not list"],
            ),
        ];
        let mut wrong = Vec::new();
        for (name, listed, survivors, parts) in rows {
            match (listing_difference(&listed, &survivors), parts.is_empty()) {
                (None, true) => {}
                (Some(message), false) => {
                    for part in parts {
                        if !message.contains(part) {
                            wrong.push(format!("{name}: {part} not in {message}"));
                        }
                    }
                }
                (found, _) => wrong.push(format!("{name}: unexpected {found:?}")),
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }
}
