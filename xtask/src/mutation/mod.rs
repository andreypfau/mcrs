pub(crate) mod attribution;
pub(crate) mod classify;
pub(crate) mod config;
pub(crate) mod diff;
pub(crate) mod matrix;
pub(crate) mod outcomes;
pub(crate) mod plan;
pub(crate) mod ratchet;
pub(crate) mod run;
pub(crate) mod surface;
pub(crate) mod sweep;
pub(crate) mod tool;
pub(crate) mod verdict;
pub(crate) mod worktree;

use crate::metadata::Workspace;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub(crate) fn local(
    repo: &Path,
    workspace: &Workspace,
    base: &str,
    head: &str,
) -> Result<String, String> {
    let tree = worktree::prepare(repo, head)?;
    let plan = plan::build(&plan::Inputs {
        repo,
        tree: &tree.path,
        workspace,
        base,
        head,
        baseline: plan::Baseline::Run,
        shard_size: usize::MAX,
        sweep: false,
    })?;
    if !plan.in_force {
        return Ok(format!(
            "not in force yet at {}; {} mutants in changed code",
            crate::scope::short(base),
            plan.total_mutants()
        ));
    }
    for index in 0..plan.shards.len() {
        run::shard(&tree, repo, &plan, index)?;
    }
    let results = verdict::read_results(&tree.out_dir, &plan)?;
    verdict::report(&plan, head, &results)
}

const USAGE: &str = "usage: xtask mutants plan
       xtask mutants shard <index>
       xtask mutants compare <mutants.out> <mutants.out>
       xtask mutants matrix <crate> [--shard <k>/<n>]
       xtask mutants matrix-merge <crate> [<shard dir>...]
       xtask mutants protected [<crate>] [--list]
       xtask mutants sweep-plan [--crate <name>] [--kind sweep|matrix]
       xtask mutants merge --run-id <n> --run-url <url> --date <YYYY-MM-DD> --branch <name>
       xtask mutants ratchet";

pub(crate) fn cli(args: &[String]) -> ExitCode {
    match args {
        [command] if command == "plan" => finish(plan_command()),
        [command, index] if command == "shard" => finish(shard_command(index)),
        [command, first, second] if command == "compare" => {
            compare(Path::new(first), Path::new(second))
        }
        [command, package, rest @ ..] if command == "matrix" => matrix(package, rest),
        [command, package, dirs @ ..] if command == "matrix-merge" => matrix_merge(package, dirs),
        [command, rest @ ..] if command == "sweep-plan" => finish(sweep_plan_command(rest)),
        [command, rest @ ..] if command == "merge" => finish(sweep::merge_command(rest)),
        [command] if command == "ratchet" => finish(ratchet::command()),
        [command, rest @ ..] if command == "protected" => protected(rest),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn head_of(repo: &Path) -> Result<String, String> {
    let sha = crate::git::text(repo, &["rev-parse", "--verify", "HEAD^{commit}"])
        .map_err(|e| e.to_string())?;
    Ok(sha.trim().to_owned())
}

fn plan_command() -> Result<(), String> {
    let repo = crate::checks::root_of(Path::new("."))?;
    let head = head_of(&repo)?;
    let base = crate::scope::resolve_base(
        &repo,
        &crate::base_ref(),
        &crate::scope::Head::Commit(head.clone()),
    )
    .map_err(|e| e.to_string())?;
    let workspace = Workspace::load(&repo)?;
    let plan = plan::build(&plan::Inputs {
        repo: &repo,
        tree: &repo,
        workspace: &workspace,
        base: &base,
        head: &head,
        baseline: plan::Baseline::Skip,
        shard_size: tool::MUTANTS_PER_SHARD,
        sweep: true,
    })?;
    plan::write_plan(&repo, &plan)?;
    if !plan.in_force {
        println!("not in force yet at {}", crate::scope::short(&base));
    }
    for (index, unit) in plan.units.iter().enumerate() {
        let shards = plan.shards.iter().filter(|s| s.unit == index).count();
        println!(
            "{}: {} mutants in {shards} shards",
            unit.package,
            unit.names.len()
        );
    }
    if !plan.surface.is_empty() {
        let base_report = classify::report_at(&repo, &workspace, &base)?;
        let head_report = classify::report_at(&repo, &workspace, &head)?;
        let text =
            classify::changed_counts(&plan.surface, (&base, &base_report), (&head, &head_report));
        print!("{text}");
        crate::append_summary(&text);
    }
    for broken in &plan.broken {
        println!(
            "{}: line {} of {} lies in a function body but lists no mutant",
            broken.package, broken.line, broken.file
        );
    }
    let indexes: Vec<usize> = (0..plan.shards.len()).collect();
    println!("count={}", indexes.len());
    println!(
        "shards={}",
        serde_json::to_string(&indexes).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn shard_command(index: &str) -> Result<(), String> {
    let index: usize = index
        .parse()
        .map_err(|_| format!("the shard index must be a number, not {index:?}"))?;
    let repo = crate::checks::root_of(Path::new("."))?;
    tool::require_version(&repo)?;
    let plan = plan::read_plan(&repo)?;
    let head = head_of(&repo)?;
    if head != plan.head {
        return Err(format!(
            "the checkout is at {}, but the plan was made for {}; check out the planned commit",
            crate::scope::short(&head),
            crate::scope::short(&plan.head)
        ));
    }
    let out = match sweep::matrix_shard(&repo, &plan, index)? {
        Some(out) => out,
        None => run::shard(&worktree::checkout(&repo), &repo, &plan, index)?,
    };
    println!("wrote {}", out.display());
    Ok(())
}

fn sweep_plan_command(args: &[String]) -> Result<(), String> {
    let (mut package, mut kind) = (None, sweep::Kind::Sweep);
    let mut rest = args.iter();
    while let Some(flag) = rest.next() {
        match (flag.as_str(), rest.next()) {
            ("--crate", Some(name)) if !name.is_empty() => package = Some(name.as_str()),
            ("--kind", Some(value)) if value == "sweep" => kind = sweep::Kind::Sweep,
            ("--kind", Some(value)) if value == "matrix" => kind = sweep::Kind::Matrix,
            _ => return Err(USAGE.to_owned()),
        }
    }
    sweep::plan_command(package, kind)
}

fn compare(first: &Path, second: &Path) -> ExitCode {
    let sets = outcomes::load(first).and_then(|a| outcomes::load(second).map(|b| (a, b)));
    let (first_set, second_set) = match sets {
        Ok(sets) => sets,
        Err(problem) => {
            eprintln!("{problem}");
            return ExitCode::from(1);
        }
    };
    let differences = outcomes::differences(&first_set, &second_set);
    if differences.is_empty() {
        println!("identical: {} mutants", first_set.len());
        return ExitCode::SUCCESS;
    }
    println!("{} differences:", differences.len());
    for difference in &differences {
        println!("{difference}");
    }
    ExitCode::from(1)
}

fn parse_shard(text: &str) -> Result<(u32, u32), String> {
    let wrong = || format!("--shard takes <k>/<n> with k below n, not {text:?}");
    let (index, count) = text.split_once('/').ok_or_else(wrong)?;
    let (index, count) = (
        index.parse::<u32>().map_err(|_| wrong())?,
        count.parse::<u32>().map_err(|_| wrong())?,
    );
    if index < count {
        Ok((index, count))
    } else {
        Err(wrong())
    }
}

fn matrix(package: &str, rest: &[String]) -> ExitCode {
    let shard = match rest {
        [] => Ok(None),
        [flag, value] if flag == "--shard" => parse_shard(value).map(Some),
        _ => Err(USAGE.to_owned()),
    };
    let result = shard.and_then(|shard| {
        let repo = crate::checks::root_of(Path::new("."))?;
        let dir = matrix::measure(&repo, package, shard, &crate::base_ref())?;
        println!("wrote {}", dir.display());
        if shard.is_some() {
            return Ok(());
        }
        let merged = matrix::merge_dirs(&repo, package, &[dir])?;
        println!("{}", matrix::summary(&merged));
        Ok(())
    });
    finish(result)
}

fn matrix_merge(package: &str, dirs: &[String]) -> ExitCode {
    let result = crate::checks::root_of(Path::new(".")).and_then(|repo| {
        let dirs: Vec<PathBuf> = if dirs.is_empty() {
            matrix::shard_dirs(&repo.join("target/mutation/shards"))?
        } else {
            dirs.iter().map(PathBuf::from).collect()
        };
        let merged = matrix::merge_dirs(&repo, package, &dirs)?;
        println!("{}", matrix::summary(&merged));
        Ok(())
    });
    finish(result)
}

fn finish(result: Result<(), String>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(problem) => {
            eprintln!("{problem}");
            ExitCode::from(1)
        }
    }
}

fn protected(rest: &[String]) -> ExitCode {
    let list = rest.iter().any(|arg| arg == "--list");
    let named: Vec<&String> = rest.iter().filter(|arg| *arg != "--list").collect();
    if named.len() > 1 || named.iter().any(|arg| arg.starts_with("--")) {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let result = crate::checks::root_of(Path::new(".")).and_then(|repo| {
        let workspace = crate::metadata::Workspace::load(&repo)?;
        if let Some(package) = named.first()
            && !workspace.packages.iter().any(|p| &p.name == *package)
        {
            return Err(format!("{package} is not a package of this workspace"));
        }
        let report = classify::workspace_report(&repo, &workspace)?;
        let mut names: Vec<&str> = workspace.packages.iter().map(|p| p.name.as_str()).collect();
        names.sort_unstable();
        for name in names
            .iter()
            .filter(|n| named.first().is_none_or(|p| p == n))
        {
            println!(
                "{}",
                classify::summary_line(&report, name, Some(name), &classify::ROWS)
            );
            if list {
                for line in classify::protected_lines(&report, name) {
                    println!("{line}");
                }
            }
        }
        if named.is_empty() {
            println!(
                "{}",
                classify::summary_line(&report, "total", None, &classify::ROWS)
            );
        }
        for line in classify::table_lines(&report) {
            println!("{line}");
        }
        Ok(())
    });
    finish(result)
}

#[cfg(test)]
mod tests {
    mod cargo_mutants {
        use super::super::*;
        use crate::metadata::Package;
        use crate::scope::tests::Repo;

        const CARGO: &str = "[package]\nname = \"listed\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n\n[profile.mutants]\ninherits = \"test\"\ndebug = \"none\"\n";
        const NEXTEST: &str = "[profile.default]\ndefault-filter = \"not test(/(^|::)(exhaustive|gpu|online|vanilla_server)::/)\"\n\n[profile.mutants]\nfail-fast = true\nretries = 0\n";
        const KEYS: &str = "test_tool = \"nextest\"\nprofile = \"mutants\"\nadditional_cargo_test_args = [\"--profile=mutants\", \"--no-tests=pass\"]\ngitignore = true\n";
        const SPARE_EXCLUDED: &str = "exclude_re = [\n  \"^src/lib\\\\.rs:\\\\d+:\\\\d+: replace spare -> i32 with .*$\", # equivalent: the helper is unused\n  \"^src/lib\\\\.rs:\\\\d+:\\\\d+: replace \\\\+ with .* in spare$\", # equivalent: the helper is unused\n]\n";
        const NEGATE: &str = "pub fn negate(x: i32) -> i32 {\n    -x\n}\n\n";

        fn source(items: &str, tests: &str, with_unused: bool, spare: &str) -> String {
            let unused = if with_unused {
                "    let _unused = x;\n"
            } else {
                ""
            };
            format!(
                "pub fn double(x: i32) -> i32 {{\n{unused}    x * 2\n}}\n\npub fn spare(x: i32) -> i32 {{\n    {spare}\n}}\n\n{items}#[cfg(test)]\nmod tests {{\n    use super::*;\n\n    #[test]\n    fn doubles() {{\n        assert_eq!(double(2), 4);\n    }}\n{tests}}}\n"
            )
        }

        fn base_source() -> String {
            source("", "", true, "x + 1")
        }

        fn listed_workspace() -> Workspace {
            Workspace {
                packages: vec![Package {
                    name: "listed".to_owned(),
                    dir: String::new(),
                    edition: "2024".to_owned(),
                    dependencies: Vec::new(),
                    targets: Vec::new(),
                }],
            }
        }

        struct Row {
            name: &'static str,
            base_files: Vec<(&'static str, String)>,
            from_unconfigured: bool,
            head_files: Vec<(&'static str, String)>,
            passes: bool,
            parts: Vec<&'static str>,
        }

        fn row(
            name: &'static str,
            head_source: String,
            passes: bool,
            parts: Vec<&'static str>,
        ) -> Row {
            Row {
                name,
                base_files: Vec::new(),
                from_unconfigured: false,
                head_files: vec![("src/lib.rs", head_source)],
                passes,
                parts,
            }
        }

        fn fixture(label: &str) -> (Repo, String, String) {
            let repo = Repo::new(label);
            repo.init();
            repo.write(".gitignore", "/target\n");
            repo.write("Cargo.toml", CARGO);
            repo.write("src/lib.rs", &base_source());
            repo.write(".config/nextest.toml", NEXTEST);
            let status = std::process::Command::new("cargo")
                .args(["generate-lockfile", "--offline"])
                .current_dir(&repo.0)
                .status()
                .unwrap();
            assert!(status.success());
            repo.commit("without configuration");
            let unconfigured = repo.sha();
            repo.write(".cargo/mutants.toml", &format!("{KEYS}exclude_re = []\n"));
            repo.commit("with configuration");
            let configured = repo.sha();
            (repo, unconfigured, configured)
        }

        #[test]
        fn shards_run_in_place_are_judged_from_their_output_directories() {
            const TRIPLE: &str = "pub fn triple(x: i32) -> i32 {\n    x * 3\n}\n\n";
            const TRIPLES: &str =
                "\n    #[test]\n    fn triples() {\n        assert_eq!(triple(2), 6);\n    }\n";
            let (repo, _, configured) = fixture("shard-output");
            let judged = |items: &str, tests: &str, drop_shard: Option<usize>| {
                repo.git(&["checkout", "--quiet", "-B", "head", &configured]);
                repo.write("src/lib.rs", &source(items, tests, true, "x + 1"));
                repo.commit("head");
                let head = repo.sha();
                let plan = plan::build(&plan::Inputs {
                    repo: &repo.0,
                    tree: &repo.0,
                    workspace: &listed_workspace(),
                    base: &configured,
                    head: &head,
                    baseline: plan::Baseline::Skip,
                    shard_size: 2,
                    sweep: false,
                })
                .unwrap();
                plan::write_plan(&repo.0, &plan).unwrap();
                let tree = worktree::checkout(&repo.0);
                for index in 0..plan.shards.len() {
                    run::shard(&tree, &repo.0, &plan, index).unwrap();
                }
                if let Some(index) = drop_shard {
                    std::fs::remove_dir_all(tree.out_dir.join(index.to_string())).unwrap();
                }
                (plan.shards.len(), verdict::check(&repo.0, &head))
            };

            let (shards, found) = judged(TRIPLE, TRIPLES, None);
            assert_eq!(shards, 3);
            let detail = found.unwrap();
            assert!(
                detail.contains("5 mutants in changed code: 5 caught"),
                "{detail}"
            );

            let (_, found) = judged(TRIPLE, TRIPLES, Some(1));
            let detail = found.unwrap_err();
            assert!(detail.contains("shard 1 is missing"), "{detail}");

            let (_, found) = judged(NEGATE, "", None);
            let detail = found.unwrap_err();
            assert!(
                detail.contains("survive") && detail.contains("negate"),
                "{detail}"
            );
        }

        const LOW: &str = "pub fn double(x: i32) -> i32 {\n    x * 2\n}\n\npub fn inc(x: i32) -> i32 {\n    x + 1\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn inc_adds_one() {\n        assert_eq!(inc(1), 2);\n    }\n}\n";
        const DOUBLING: &str = "    #[test]\n    fn doubling_twice_quadruples() {\n        assert_eq!(low::double(low::double(1)), 4);\n    }\n\n";
        const REFORMATTED: &str = "    #[test]\n    fn doubling_twice_quadruples() {\n        assert_eq!(\n            low::double(low::double(1)),\n            4\n        );\n    }\n\n";

        fn high(doubling: &str) -> String {
            format!(
                "pub fn marker() -> bool {{\n    true\n}}\n\n#[cfg(test)]\nmod tests {{\n{doubling}    #[test]\n    fn always_true() {{\n        assert!(true);\n    }}\n}}\n"
            )
        }

        #[test]
        fn a_floored_crate_is_swept_when_its_tests_change() {
            use crate::floors::tests::{measured, unmeasured};
            let repo = Repo::new("floor-sweep");
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
            repo.write("high/src/lib.rs", &high(DOUBLING));
            repo.write(".config/nextest.toml", NEXTEST);
            repo.write(".cargo/mutants.toml", &format!("{KEYS}exclude_re = []\n"));
            repo.write(
                crate::floors::FLOORS_FILE,
                &format!(
                    "{}{}",
                    measured("low", "1", (10, 0, 0)).text(),
                    unmeasured("high")
                ),
            );
            let status = std::process::Command::new("cargo")
                .args(["generate-lockfile", "--offline"])
                .current_dir(&repo.0)
                .status()
                .unwrap();
            assert!(status.success());
            repo.commit("base");
            let base = repo.sha();
            let workspace = Workspace::load(&repo.0).unwrap();

            let judged = |doubling: &str| {
                repo.git(&["checkout", "--quiet", "-B", "head", &base]);
                repo.write("high/src/lib.rs", &high(doubling));
                repo.commit("head");
                let head = repo.sha();
                let plan = plan::build(&plan::Inputs {
                    repo: &repo.0,
                    tree: &repo.0,
                    workspace: &workspace,
                    base: &base,
                    head: &head,
                    baseline: plan::Baseline::Skip,
                    shard_size: 100,
                    sweep: true,
                })
                .unwrap();
                plan::write_plan(&repo.0, &plan).unwrap();
                let tree = worktree::checkout(&repo.0);
                for index in 0..plan.shards.len() {
                    run::shard(&tree, &repo.0, &plan, index).unwrap();
                }
                (plan, verdict::check(&repo.0, &head))
            };

            let (plan, found) = judged("");
            assert_eq!(plan.surface, ["high"]);
            let swept: Vec<_> = plan
                .units
                .iter()
                .map(|unit| (unit.package.as_str(), unit.floor))
                .collect();
            assert_eq!(swept, [("low", Some(100))]);
            let message = found.unwrap_err();
            assert!(
                message.contains("low: the sweep scores")
                    && message.contains("against the floor 1.00"),
                "{message}"
            );

            let (plan, found) = judged(REFORMATTED);
            assert!(plan.surface.is_empty(), "{plan:?}");
            assert!(
                plan.units.iter().all(|unit| unit.floor.is_none()),
                "{plan:?}"
            );
            assert!(plan.shards.is_empty(), "{plan:?}");
            let detail = found.unwrap();
            assert!(detail.contains("no mutant in changed code"), "{detail}");
        }

        fn low_with_new_functions() -> String {
            LOW.replace(
                "#[cfg(test)]",
                "pub fn triple(x: i32) -> i32 {\n    x * 3\n}\n\npub fn quadruple(x: i32) -> i32 {\n    x * 4\n}\n\npub fn negate(x: i32) -> i32 {\n    -x\n}\n\n#[cfg(test)]",
            )
            .replace(
                "    #[test]\n    fn inc_adds_one",
                "    #[test]\n    fn triple_multiplies_by_three() {\n        assert_eq!(triple(2), 6);\n    }\n\n    #[test]\n    fn inc_adds_one",
            )
        }

        #[test]
        fn a_mutant_only_a_dependent_catches_is_caught_in_the_second_pass() {
            const QUADRUPLING: &str = "    #[test]\n    fn quadrupling_multiplies_by_four() {\n        assert_eq!(low::quadruple(2), 8);\n    }\n\n";
            let repo = Repo::new("two-passes");
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
            repo.write("high/src/lib.rs", &high(DOUBLING));
            repo.write(".config/nextest.toml", NEXTEST);
            repo.write(".cargo/mutants.toml", &format!("{KEYS}exclude_re = []\n"));
            let status = std::process::Command::new("cargo")
                .args(["generate-lockfile", "--offline"])
                .current_dir(&repo.0)
                .status()
                .unwrap();
            assert!(status.success());
            repo.commit("base");
            let base = repo.sha();
            repo.write("low/src/lib.rs", &low_with_new_functions());
            repo.write(
                "high/src/lib.rs",
                &high(&format!("{DOUBLING}{QUADRUPLING}")),
            );
            repo.commit("head");
            let head = repo.sha();
            let workspace = Workspace::load(&repo.0).unwrap();

            let message = local(&repo.0, &workspace, &base, &head).unwrap_err();
            assert!(message.contains("negate"), "{message}");
            assert!(
                !message.contains("triple") && !message.contains("quadruple"),
                "{message}"
            );

            let shard = repo.0.join("target/mutation/out/0");
            let first = outcomes::load(&shard.join("mutants.out")).unwrap();
            let second = outcomes::load(&shard.join("dependents/mutants.out")).unwrap();
            let named = |set: &std::collections::BTreeMap<String, outcomes::Class>, function| {
                set.keys().filter(|name| name.contains(function)).count()
            };
            for function in ["triple", "quadruple", "negate"] {
                assert!(named(&first, function) > 0, "{function} in {first:?}");
            }
            assert_eq!(named(&second, "triple"), 0, "{second:?}");
            let missed: Vec<&String> = first
                .iter()
                .filter(|(_, class)| **class == outcomes::Class::Missed)
                .map(|(name, _)| name)
                .collect();
            let second_names: Vec<&String> = second.keys().collect();
            assert_eq!(second_names, missed);
            for (name, class) in &second {
                let expected = if name.contains("quadruple") {
                    outcomes::Class::Caught
                } else {
                    outcomes::Class::Missed
                };
                assert_eq!(*class, expected, "{name}");
            }
            assert!(named(&second, "quadruple") > 0, "{second:?}");
            assert!(named(&second, "negate") > 0, "{second:?}");
        }

        #[test]
        fn the_diff_gate_judges_changed_code() {
            let (repo, unconfigured, configured) = fixture("diff-gate");

            let none = "no mutant in changed code";
            let excluded = format!("{KEYS}{SPARE_EXCLUDED}");
            let rows = vec![
                row(
                    "a new tested function",
                    source(
                        "pub fn triple(x: i32) -> i32 {\n    x * 3\n}\n\n",
                        "\n    #[test]\n    fn triples() {\n        assert_eq!(triple(2), 6);\n    }\n",
                        true,
                        "x + 1",
                    ),
                    true,
                    vec!["caught"],
                ),
                row(
                    "a new untested function",
                    source(NEGATE, "", true, "x + 1"),
                    false,
                    vec!["negate", "survive"],
                ),
                row(
                    "a deletion inside a function",
                    source("", "", false, "x + 1"),
                    true,
                    vec![none],
                ),
                row(
                    "a struct and nothing else",
                    source(
                        "pub struct Point {\n    pub x: i32,\n}\n\n",
                        "",
                        true,
                        "x + 1",
                    ),
                    true,
                    vec![none],
                ),
                row(
                    "a comment and nothing else",
                    source("// a note for readers\n", "", true, "x + 1"),
                    true,
                    vec![none],
                ),
                row(
                    "a change to a test only",
                    source(
                        "",
                        "\n    #[test]\n    fn doubles_zero() {\n        assert_eq!(double(0), 0);\n    }\n",
                        true,
                        "x + 1",
                    ),
                    true,
                    vec![none],
                ),
                Row {
                    base_files: vec![(".cargo/mutants.toml", excluded.clone())],
                    ..row(
                        "a change whose only mutants match a base exclusion",
                        source("", "", true, "x + 2"),
                        true,
                        vec![none],
                    )
                },
                Row {
                    head_files: vec![
                        ("src/lib.rs", source("", "", true, "x + 2")),
                        (".cargo/mutants.toml", excluded),
                    ],
                    ..row(
                        "an exclusion that exists only at the head",
                        String::new(),
                        false,
                        vec!["spare", "survive"],
                    )
                },
                Row {
                    from_unconfigured: true,
                    ..row(
                        "the same untested function without a base configuration",
                        source(NEGATE, "", true, "x + 1"),
                        true,
                        vec!["not in force yet at "],
                    )
                },
            ];
            let mut problems = Vec::new();
            for Row {
                name,
                base_files,
                from_unconfigured,
                head_files,
                passes,
                parts,
            } in rows
            {
                let start = if from_unconfigured {
                    &unconfigured
                } else {
                    &configured
                };
                repo.git(&["checkout", "--quiet", "-B", "row", start]);
                for (file, text) in &base_files {
                    repo.write(file, text);
                }
                let base = if base_files.is_empty() {
                    start.clone()
                } else {
                    repo.commit("base");
                    repo.sha()
                };
                for (file, text) in &head_files {
                    repo.write(file, text);
                }
                repo.commit("head");
                let head = repo.sha();
                let found = local(&repo.0, &listed_workspace(), &base, &head);
                let (detail, expected) = match (passes, found) {
                    (true, Ok(detail)) | (false, Err(detail)) => (detail, true),
                    (_, Ok(detail)) | (_, Err(detail)) => (detail, false),
                };
                if !expected {
                    problems.push(format!("{name}: unexpected outcome: {detail}"));
                    continue;
                }
                for part in parts {
                    if !detail.contains(part) {
                        problems.push(format!("{name}: {part} not in {detail}"));
                    }
                }
            }
            assert!(problems.is_empty(), "{}", problems.join("\n"));
        }
    }
}
