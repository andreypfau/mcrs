use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

const ONE: &str = "src/lib.rs:3:5: replace f -> bool with true";
const TWO: &str = "src/lib.rs:9:5: replace g -> bool with false";
const THREE: &str = "src/lib.rs:20:5: replace h -> bool with true";

const FAKE_CARGO: &str = r#"#!/bin/sh
{ printf '%s\n' "$@"; echo '--'; } >> "$FAKE_DIR/calls"
if [ "$1" = metadata ]; then
  printf '{"workspace_root":"%s","packages":[{"name":"a","manifest_path":"%s/Cargo.toml","edition":"2024","dependencies":[],"targets":[{"kind":["bin"],"name":"t","src_path":"%s/src/main.rs"},{"kind":["lib"],"name":"a","src_path":"%s/src/lib.rs"}]}]}' "$PWD" "$PWD" "$PWD" "$PWD"
  exit 0
fi
[ "$1" = mutants ] || exit 0
shift
list=0
re=0
out=
while [ $# -gt 0 ]; do
  case "$1" in
    --version) echo "cargo-mutants 27.1.0"; exit 0 ;;
    --list) list=1 ;;
    --re) re=1 ;;
    --output) out=$2; shift ;;
  esac
  shift
done
if [ $list = 1 ]; then
  if [ -f "$FAKE_DIR/list_fails" ]; then printf 'boom: bad\n\n' >&2; exit 5; fi
  cat "$FAKE_DIR/listing.json"
  exit 0
fi
mkdir -p "$out/mutants.out"
if [ $re = 1 ]; then pass=second; else pass=first; fi
cp "$FAKE_DIR/$pass.json" "$out/mutants.out/outcomes.json"
exit 2
"#;

type Pass<'a> = &'a [(&'a str, &'a str)];

struct Case<'a> {
    name: &'a str,
    baseline: &'a str,
    floor: Option<u64>,
    test_packages: &'a [&'a str],
    planned: (u32, u32),
    first: Pass<'a>,
    listing: &'a [&'a str],
    second: Pass<'a>,
    calls: &'a [&'a str],
    dependents_written: bool,
    list_fails: bool,
    failure: Option<&'a [&'a str]>,
}

fn outcomes(rows: Pass) -> String {
    let outcome = |name: &str, class: &str| {
        let (summary, phases) = match class {
            "caught" => (
                "CaughtMutant",
                r#"{"phase":"Build","process_status":"Success"},{"phase":"Test","process_status":{"Failure":100}}"#,
            ),
            "missed" => (
                "MissedMutant",
                r#"{"phase":"Build","process_status":"Success"},{"phase":"Test","process_status":"Success"}"#,
            ),
            "unviable" => (
                "Unviable",
                r#"{"phase":"Build","process_status":{"Failure":101}}"#,
            ),
            other => panic!("no outcome called {other}"),
        };
        format!(
            r#"{{"scenario":{{"Mutant":{{"name":"{name}"}}}},"summary":"{summary}","log_path":"log/{name}.log","phase_results":[{phases}]}}"#
        )
    };
    let all: Vec<String> = rows
        .iter()
        .map(|(name, class)| outcome(name, class))
        .collect();
    format!(r#"{{"outcomes":[{}]}}"#, all.join(","))
}

fn listing(names: &[&str]) -> String {
    let all: Vec<String> = names
        .iter()
        .map(|name| {
            format!(
                r#"{{"name":"{name}","package":"a","file":"src/lib.rs","function":null,"span":{{"start":{{"line":1}},"end":{{"line":1}}}}}}"#
            )
        })
        .collect();
    format!("[{}]", all.join(","))
}

fn git(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {out:?}");
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

fn summarize(call: &[String]) -> Option<String> {
    let value = |flag: &str| {
        call.windows(2)
            .find(|pair| pair[0] == flag)
            .map(|pair| pair[1].clone())
    };
    let re = call.iter().filter(|arg| *arg == "--re").count();
    let diff = call.iter().any(|arg| arg == "--in-diff");
    match call[0].as_str() {
        "metadata" => Some("metadata".to_owned()),
        "nextest" => {
            let packages: Vec<&str> = call
                .windows(2)
                .filter(|pair| pair[0] == "-p")
                .map(|pair| pair[1].as_str())
                .collect();
            Some(format!("build {}", packages.join(",")))
        }
        "mutants" if call.iter().any(|arg| arg == "--version") => None,
        "mutants" if call.iter().any(|arg| arg == "--list") => {
            Some(format!("list re={re} diff={diff}"))
        }
        "mutants" => {
            let baseline = call
                .iter()
                .find_map(|arg| arg.strip_prefix("--baseline="))
                .unwrap();
            Some(format!(
                "run shard={} baseline={baseline} packages={} re={re} diff={diff} build_timeout={}",
                value("--shard").unwrap(),
                value("--test-package").unwrap(),
                value("--build-timeout").unwrap()
            ))
        }
        other => panic!("unexpected cargo call {other}"),
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "builds a throwaway repository with a fake cargo, runs the shard against it and checks the calls, marks and outcomes it left"
)]
fn run(case: &Case, label: usize) -> Result<(), String> {
    let root = std::env::temp_dir().join(format!("xtask-shard-{}-{label}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let repo: PathBuf = root.join("repo");
    let bin = root.join("bin");
    let fake = root.join("fake");
    for dir in [&repo, &bin, &fake] {
        std::fs::create_dir_all(dir).unwrap();
    }
    git(&repo, &["init", "--quiet"]);
    std::fs::write(repo.join("README"), "x").unwrap();
    git(&repo, &["add", "README"]);
    git(&repo, &["commit", "--quiet", "-m", "base"]);
    let head = git(&repo, &["rev-parse", "HEAD"]);

    std::fs::create_dir_all(repo.join("src")).unwrap();
    let lib = repo.join("src/lib.rs");
    std::fs::write(&lib, "pub fn f() {}\n").unwrap();
    let stale = SystemTime::now() - Duration::from_secs(3600);
    std::fs::File::options()
        .write(true)
        .open(&lib)
        .unwrap()
        .set_modified(stale)
        .unwrap();
    std::fs::write(repo.join("src/main.rs"), "fn main() {}\n").unwrap();

    let cargo = bin.join("cargo");
    std::fs::write(&cargo, FAKE_CARGO).unwrap();
    std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(fake.join("first.json"), outcomes(case.first)).unwrap();
    std::fs::write(fake.join("second.json"), outcomes(case.second)).unwrap();
    std::fs::write(fake.join("listing.json"), listing(case.listing)).unwrap();
    if case.list_fails {
        std::fs::write(fake.join("list_fails"), "").unwrap();
    }

    let names: Vec<String> = case
        .first
        .iter()
        .map(|(name, _)| format!("{name:?}"))
        .collect();
    let packages: Vec<String> = case
        .test_packages
        .iter()
        .map(|name| format!("{name:?}"))
        .collect();
    let floor = case
        .floor
        .map_or_else(|| "null".to_owned(), |floor| floor.to_string());
    let (index, count) = case.planned;
    let plan = format!(
        r#"{{"base":"{head}","head":"{head}","in_force":true,"baseline":"{}","diff":"","units":[{{"package":"a","test_packages":[{}],"names":[{}],"floor":{floor}}}],"shards":[{{"unit":0,"index":{index},"count":{count}}}],"broken":[],"surface":[],"matrix":null}}"#,
        case.baseline,
        packages.join(","),
        names.join(",")
    );
    let plan_dir = repo.join("target/mutation/plan");
    std::fs::create_dir_all(&plan_dir).unwrap();
    std::fs::write(plan_dir.join("plan.json"), plan).unwrap();

    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let out = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["mutants", "shard", "0"])
        .current_dir(&repo)
        .env("PATH", path)
        .env("FAKE_DIR", &fake)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();

    let log = std::fs::read_to_string(fake.join("calls")).unwrap();
    let calls: Vec<Vec<String>> = log
        .split("\n--\n")
        .filter(|call| !call.is_empty())
        .map(|call| call.lines().map(str::to_owned).collect())
        .collect();
    let found: Vec<String> = calls.iter().filter_map(|call| summarize(call)).collect();
    let mut problems = Vec::new();
    if found != case.calls {
        problems.push(format!("calls {found:#?}"));
    }
    let marked = std::fs::metadata(&lib).unwrap().modified().unwrap();
    if marked <= stale + Duration::from_secs(60) {
        problems.push("the library source was not marked as changed".to_owned());
    }
    let shard = repo.join("target/mutation/shards/0");
    if !shard.join("mutants.out/outcomes.json").exists() {
        problems.push("the first pass wrote no outcomes".to_owned());
    }
    let wrote = shard.join("dependents/mutants.out/outcomes.json").exists();
    if wrote != case.dependents_written {
        problems.push(format!("the dependents outcomes were written: {wrote}"));
    }
    match case.failure {
        None if !out.status.success() => problems.push(format!("failed: {stderr}")),
        Some(parts) => {
            if out.status.success() {
                problems.push("succeeded".to_owned());
            }
            for part in parts {
                if !stderr.contains(part) {
                    problems.push(format!("{part} not in {stderr}"));
                }
            }
        }
        None => {}
    }
    let _ = std::fs::remove_dir_all(&root);
    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!("{}: {}", case.name, problems.join("; ")))
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "a table of shard scenarios with the tool calls each must make"
)]
fn a_shard_tests_its_survivors_against_the_dependents_and_nothing_else() {
    let own_pass = "run shard=1/2 baseline=skip packages=a re=0 diff=true build_timeout=120";
    let cases = [
        Case {
            name: "the survivors are rerun with the dependents, as one shard",
            baseline: "skip",
            floor: None,
            test_packages: &["a", "b"],
            planned: (1, 2),
            first: &[(ONE, "caught"), (TWO, "missed"), (THREE, "missed")],
            listing: &[TWO, THREE],
            second: &[(TWO, "caught"), (THREE, "missed")],
            calls: &[
                "build a",
                "metadata",
                "build a",
                own_pass,
                "list re=2 diff=true",
                "build a,b",
                "metadata",
                "build a,b",
                "run shard=0/1 baseline=skip packages=a,b re=2 diff=true build_timeout=120",
            ],
            dependents_written: true,
            list_fails: false,
            failure: None,
        },
        Case {
            name: "a swept crate runs both passes without a diff and with a baseline",
            baseline: "skip",
            floor: Some(80),
            test_packages: &["a", "b"],
            planned: (1, 2),
            first: &[(ONE, "missed")],
            listing: &[ONE],
            second: &[(ONE, "caught")],
            calls: &[
                "build a",
                "metadata",
                "build a",
                "run shard=1/2 baseline=run packages=a re=0 diff=false build_timeout=120",
                "list re=1 diff=false",
                "build a,b",
                "metadata",
                "build a,b",
                "run shard=0/1 baseline=run packages=a,b re=1 diff=false build_timeout=120",
            ],
            dependents_written: true,
            list_fails: false,
            failure: None,
        },
        Case {
            name: "a plan that runs the baseline keeps it in both passes",
            baseline: "run",
            floor: None,
            test_packages: &["a", "b"],
            planned: (1, 2),
            first: &[(ONE, "missed")],
            listing: &[ONE],
            second: &[(ONE, "caught")],
            calls: &[
                "build a",
                "metadata",
                "build a",
                "run shard=1/2 baseline=run packages=a re=0 diff=true build_timeout=120",
                "list re=1 diff=true",
                "build a,b",
                "metadata",
                "build a,b",
                "run shard=0/1 baseline=run packages=a,b re=1 diff=true build_timeout=120",
            ],
            dependents_written: true,
            list_fails: false,
            failure: None,
        },
        Case {
            name: "no survivor, no second pass",
            baseline: "skip",
            floor: None,
            test_packages: &["a", "b"],
            planned: (1, 2),
            first: &[(ONE, "caught"), (TWO, "unviable")],
            listing: &[],
            second: &[],
            calls: &["build a", "metadata", "build a", own_pass],
            dependents_written: false,
            list_fails: false,
            failure: None,
        },
        Case {
            name: "a crate without dependents runs one pass",
            baseline: "skip",
            floor: None,
            test_packages: &["a"],
            planned: (1, 2),
            first: &[(ONE, "missed")],
            listing: &[ONE],
            second: &[],
            calls: &["build a", "metadata", "build a", own_pass],
            dependents_written: false,
            list_fails: false,
            failure: None,
        },
        Case {
            name: "a listing that misses a survivor stops the shard before the second pass",
            baseline: "skip",
            floor: None,
            test_packages: &["a", "b"],
            planned: (1, 2),
            first: &[(TWO, "missed"), (THREE, "missed")],
            listing: &[TWO],
            second: &[],
            calls: &[
                "build a",
                "metadata",
                "build a",
                own_pass,
                "list re=2 diff=true",
            ],
            dependents_written: false,
            list_fails: false,
            failure: Some(&["shard 0 (a)", THREE]),
        },
        Case {
            name: "a listing that fails reports the last line cargo mutants printed",
            baseline: "skip",
            floor: None,
            test_packages: &["a", "b"],
            planned: (1, 2),
            first: &[(ONE, "missed")],
            listing: &[ONE],
            second: &[],
            calls: &[
                "build a",
                "metadata",
                "build a",
                own_pass,
                "list re=1 diff=true",
            ],
            dependents_written: false,
            list_fails: true,
            failure: Some(&["boom: bad", "exit 5"]),
        },
    ];
    let problems: Vec<String> = cases
        .iter()
        .enumerate()
        .filter_map(|(label, case)| run(case, label).err())
        .collect();
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
