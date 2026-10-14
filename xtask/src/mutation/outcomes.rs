use crate::mutation::tool;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

const TEST_FAILURE: i32 = 100;

#[derive(Debug, Deserialize)]
struct Lab {
    outcomes: Vec<ScenarioOutcome>,
}

#[derive(Debug, Deserialize)]
struct ScenarioOutcome {
    scenario: Scenario,
    summary: Summary,
    log_path: Option<String>,
    phase_results: Vec<PhaseResult>,
}

#[derive(Debug, Deserialize)]
enum Scenario {
    Baseline,
    Mutant(Named),
}

#[derive(Debug, Deserialize)]
struct Named {
    name: String,
}

#[derive(Debug, Deserialize)]
struct PhaseResult {
    phase: Phase,
    process_status: Exit,
}

#[derive(Debug, Deserialize, PartialEq, Clone, Copy)]
enum Phase {
    Check,
    Build,
    Test,
}

#[derive(Debug, Deserialize)]
enum Exit {
    Success,
    Failure(i32),
    Timeout,
    Signalled(i32),
    Other,
}

impl fmt::Display for Exit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Exit::Success => f.write_str("Success"),
            Exit::Failure(code) => write!(f, "Failure({code})"),
            Exit::Timeout => f.write_str("Timeout"),
            Exit::Signalled(signal) => write!(f, "Signalled({signal})"),
            Exit::Other => f.write_str("Other"),
        }
    }
}

#[derive(Debug, Deserialize, PartialEq)]
enum Summary {
    Success,
    CaughtMutant,
    MissedMutant,
    Unviable,
    Failure,
    Timeout,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Class {
    Caught,
    Timeout,
    Missed,
    Unviable,
}

impl fmt::Display for Class {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Class::Caught => "caught",
            Class::Timeout => "timeout",
            Class::Missed => "missed",
            Class::Unviable => "unviable",
        })
    }
}

fn judge(name: &str, phases: &[PhaseResult], summary: &Summary) -> Result<Class, String> {
    let unmeasured = |what: String| {
        Err(format!(
            "{name}: {what}; the run did not measure this mutant, so fix the runner and run again"
        ))
    };
    let status = |phase: Phase| {
        phases
            .iter()
            .find(|result| result.phase == phase)
            .map(|result| &result.process_status)
    };
    let class = match status(Phase::Build) {
        Some(Exit::Success) => match status(Phase::Test) {
            Some(Exit::Success) => Class::Missed,
            Some(Exit::Failure(TEST_FAILURE)) => Class::Caught,
            Some(Exit::Timeout) => Class::Timeout,
            Some(other) => {
                return unmeasured(format!(
                    "the Test phase ended with {other}, not nextest's test failure {TEST_FAILURE}"
                ));
            }
            None => return unmeasured("it has no Test phase".to_owned()),
        },
        Some(Exit::Failure(_)) => Class::Unviable,
        Some(other) => return unmeasured(format!("the Build phase ended with {other}")),
        None => return unmeasured("it has no Build phase".to_owned()),
    };
    if *summary == Summary::Failure {
        return unmeasured("cargo-mutants reported summary Failure".to_owned());
    }
    Ok(class)
}

pub(crate) fn score(caught: u64, timeout: u64, missed: u64) -> Option<u64> {
    let denominator = caught + timeout + missed;
    (denominator > 0).then(|| (caught + timeout) * 100 / denominator)
}

#[derive(Debug, PartialEq)]
pub(crate) struct Measured {
    pub name: String,
    pub class: Class,
    pub log_path: String,
}

pub(crate) fn missed_names(measured: &[Measured]) -> Vec<String> {
    measured
        .iter()
        .filter(|mutant| mutant.class == Class::Missed)
        .map(|mutant| mutant.name.clone())
        .collect()
}

pub(crate) fn parse_measured(json: &str) -> Result<Vec<Measured>, String> {
    let lab: Lab = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let mut seen = std::collections::BTreeSet::new();
    let mut measured = Vec::new();
    for outcome in lab.outcomes {
        let Scenario::Mutant(Named { name }) = &outcome.scenario else {
            continue;
        };
        let class = judge(name, &outcome.phase_results, &outcome.summary)?;
        if !seen.insert(name.clone()) {
            return Err(format!("{name} appears twice"));
        }
        let log_path = outcome
            .log_path
            .ok_or_else(|| format!("{name}: the outcome names no log"))?;
        measured.push(Measured {
            name: name.clone(),
            class,
            log_path,
        });
    }
    Ok(measured)
}

pub(crate) fn parse(json: &str) -> Result<BTreeMap<String, Class>, String> {
    Ok(parse_measured(json)?
        .into_iter()
        .map(|m| (m.name, m.class))
        .collect())
}

fn read_outcomes<T>(
    dir: &Path,
    parse: impl FnOnce(&str) -> Result<T, String>,
) -> Result<T, String> {
    let path = dir.join("outcomes.json");
    let json = std::fs::read_to_string(&path)
        .map_err(|e| format!("could not read {}: {e}", path.display()))?;
    parse(&json).map_err(|e| {
        format!(
            "{} is not usable as an outcomes file of cargo-mutants {}: {e}",
            path.display(),
            tool::VERSION
        )
    })
}

pub(crate) fn load(dir: &Path) -> Result<BTreeMap<String, Class>, String> {
    read_outcomes(dir, parse)
}

pub(crate) fn load_measured(dir: &Path) -> Result<Vec<Measured>, String> {
    read_outcomes(dir, parse_measured)
}

pub(crate) fn differences(
    first: &BTreeMap<String, Class>,
    second: &BTreeMap<String, Class>,
) -> Vec<String> {
    let mut found = Vec::new();
    for (name, class) in first {
        match second.get(name) {
            Some(other) if other == class => {}
            Some(other) => found.push(format!(
                "{name}: {class} in the first run, {other} in the second"
            )),
            None => found.push(format!("{name}: only in the first run ({class})")),
        }
    }
    for (name, class) in second {
        if !first.contains_key(name) {
            found.push(format!("{name}: only in the second run ({class})"));
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phases(build: &str, test: Option<&str>) -> Vec<PhaseResult> {
        let mut json = format!("[{{\"phase\":\"Build\",\"process_status\":{build}}}");
        if let Some(test) = test {
            json.push_str(&format!(
                ",{{\"phase\":\"Test\",\"process_status\":{test}}}"
            ));
        }
        json.push(']');
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "a table of build and test outcomes with the class each is given"
    )]
    fn a_test_phase_counts_as_caught_only_on_a_test_failure() {
        let ok = "\"Success\"";
        type Row<'a> = (
            &'a str,
            &'a str,
            Option<&'a str>,
            &'a str,
            Result<Class, Vec<&'a str>>,
        );
        let rows: Vec<Row> = vec![
            (
                "a test failure",
                ok,
                Some("{\"Failure\":100}"),
                "CaughtMutant",
                Ok(Class::Caught),
            ),
            (
                "a test timeout",
                ok,
                Some("\"Timeout\""),
                "Timeout",
                Ok(Class::Timeout),
            ),
            (
                "a passing test",
                ok,
                Some("\"Success\""),
                "MissedMutant",
                Ok(Class::Missed),
            ),
            (
                "a failed build",
                "{\"Failure\":101}",
                None,
                "Unviable",
                Ok(Class::Unviable),
            ),
            (
                "a build timeout",
                "\"Timeout\"",
                None,
                "Timeout",
                Err(vec!["Build", "Timeout"]),
            ),
            (
                "a build killed by a signal",
                "{\"Signalled\":9}",
                None,
                "Failure",
                Err(vec!["Build", "Signalled(9)"]),
            ),
            (
                "no test selected",
                ok,
                Some("{\"Failure\":4}"),
                "CaughtMutant",
                Err(vec!["Test", "Failure(4)"]),
            ),
            (
                "a build failure inside the test run",
                ok,
                Some("{\"Failure\":101}"),
                "CaughtMutant",
                Err(vec!["Failure(101)"]),
            ),
            (
                "a test list failure",
                ok,
                Some("{\"Failure\":104}"),
                "CaughtMutant",
                Err(vec!["Failure(104)"]),
            ),
            (
                "a failed setup script",
                ok,
                Some("{\"Failure\":105}"),
                "CaughtMutant",
                Err(vec!["Failure(105)"]),
            ),
            (
                "an incomplete run",
                ok,
                Some("{\"Failure\":106}"),
                "CaughtMutant",
                Err(vec!["Failure(106)"]),
            ),
            (
                "any other exit code",
                ok,
                Some("{\"Failure\":1}"),
                "CaughtMutant",
                Err(vec!["Failure(1)"]),
            ),
            (
                "a test killed by a signal",
                ok,
                Some("{\"Signalled\":9}"),
                "Failure",
                Err(vec!["Signalled(9)"]),
            ),
            (
                "an unknown status",
                ok,
                Some("\"Other\""),
                "Failure",
                Err(vec!["Other"]),
            ),
            (
                "summary Failure",
                ok,
                Some("{\"Failure\":100}"),
                "Failure",
                Err(vec!["summary Failure"]),
            ),
            (
                "a missing test phase",
                ok,
                None,
                "MissedMutant",
                Err(vec!["no Test phase"]),
            ),
        ];
        for (name, build, test, summary, expected) in rows {
            let summary: Summary = serde_json::from_str(&format!("\"{summary}\"")).unwrap();
            let found = judge("m", &phases(build, test), &summary);
            match (expected, found) {
                (Ok(class), Ok(found)) => assert_eq!(found, class, "{name}"),
                (Err(parts), Err(message)) => {
                    assert!(message.starts_with("m: "), "{name}: {message}");
                    for part in parts {
                        assert!(message.contains(part), "{name}: {part} not in {message}");
                    }
                }
                (expected, found) => panic!("{name}: expected {expected:?}, found {found:?}"),
            }
        }
    }

    #[test]
    fn the_score_counts_timeouts_as_caught_and_rounds_down_to_hundredths() {
        let rows = [
            ((83, 1, 17), Some(83)),
            ((2, 0, 1), Some(66)),
            ((0, 0, 0), None),
            ((1, 0, 0), Some(100)),
        ];
        for ((caught, timeout, missed), expected) in rows {
            assert_eq!(
                score(caught, timeout, missed),
                expected,
                "{caught} {timeout} {missed}"
            );
        }
    }

    const LAB: &str = r#"{"outcomes":[
        {"scenario":"Baseline","summary":"Success","log_path":"log/baseline.log","diff_path":null,
         "phase_results":[{"phase":"Build","duration":1.5,"process_status":"Success","argv":["cargo"]}]},
        {"scenario":{"Mutant":{"name":"crates/x/src/lib.rs:18:9: replace f -> i32 with 0","package":"x","file":"crates/x/src/lib.rs",
          "function":{"function_name":"f","return_type":"-> i32","span":{"start":{"line":17,"column":5},"end":{"line":19,"column":6}}},
          "span":{"start":{"line":18,"column":9},"end":{"line":18,"column":31}},"replacement":"0","genre":"FnValue"}},
         "summary":"CaughtMutant","log_path":"log/a.log","diff_path":"diff/a.diff",
         "phase_results":[{"phase":"Build","duration":1.5,"process_status":"Success","argv":["cargo"]},
                          {"phase":"Test","duration":5.8,"process_status":{"Failure":100},"argv":["cargo"]}]},
        {"scenario":{"Mutant":{"name":"crates/x/src/lib.rs:30:5: replace g -> bool with true","package":"x","file":"crates/x/src/lib.rs",
          "function":null,"span":{"start":{"line":30,"column":5},"end":{"line":30,"column":9}},"replacement":"true","genre":"FnValue"}},
         "summary":"MissedMutant","log_path":"log/b.log","diff_path":"diff/b.diff",
         "phase_results":[{"phase":"Build","duration":1.5,"process_status":"Success","argv":["cargo"]},
                          {"phase":"Test","duration":5.0,"process_status":"Success","argv":["cargo"]}]},
        {"scenario":{"Mutant":{"name":"crates/x/src/lib.rs:40:5: replace h -> u8 with 7","package":"x","file":"crates/x/src/lib.rs",
          "function":null,"span":{"start":{"line":40,"column":5},"end":{"line":40,"column":9}},"replacement":"7","genre":"FnValue"}},
         "summary":"Unviable","log_path":"log/c.log","diff_path":"diff/c.diff",
         "phase_results":[{"phase":"Build","duration":0.7,"process_status":{"Failure":101},"argv":["cargo"]}]}
        ],"total_mutants":3,"missed":1,"caught":1,"timeout":0,"unviable":1,"success":0,
        "start_time":"2026-10-11T09:37:56Z","end_time":"2026-10-11T12:23:20Z","cargo_mutants_version":"27.1.0"}"#;

    #[test]
    fn outcomes_are_read_as_cargo_mutants_writes_them() {
        let set = parse(LAB).unwrap();
        let listed: Vec<(&str, Class)> = set.iter().map(|(n, c)| (n.as_str(), *c)).collect();
        assert_eq!(
            listed,
            [
                (
                    "crates/x/src/lib.rs:18:9: replace f -> i32 with 0",
                    Class::Caught
                ),
                (
                    "crates/x/src/lib.rs:30:5: replace g -> bool with true",
                    Class::Missed
                ),
                (
                    "crates/x/src/lib.rs:40:5: replace h -> u8 with 7",
                    Class::Unviable
                ),
            ]
        );
    }

    #[test]
    fn two_outcome_sets_compare_by_name_and_outcome() {
        let set = |rows: &[(&str, Class)]| -> BTreeMap<String, Class> {
            rows.iter().map(|(n, c)| ((*n).to_owned(), *c)).collect()
        };
        let base = set(&[
            ("a.rs:1:1: x", Class::Caught),
            ("a.rs:2:1: y", Class::Missed),
        ]);
        assert_eq!(differences(&base, &base.clone()), Vec::<String>::new());
        let rows = [
            (
                "a changed outcome",
                set(&[
                    ("a.rs:1:1: x", Class::Caught),
                    ("a.rs:2:1: y", Class::Caught),
                ]),
                "a.rs:2:1: y",
            ),
            (
                "a missing mutant",
                set(&[("a.rs:1:1: x", Class::Caught)]),
                "a.rs:2:1: y",
            ),
            (
                "a changed line number",
                set(&[
                    ("a.rs:1:1: x", Class::Caught),
                    ("a.rs:3:1: y", Class::Missed),
                ]),
                "a.rs:3:1: y",
            ),
        ];
        for (name, other, named) in rows {
            let found = differences(&base, &other);
            assert!(!found.is_empty(), "{name}");
            assert!(
                found.iter().any(|line| line.contains(named)),
                "{name}: {found:?}"
            );
        }
    }
}
