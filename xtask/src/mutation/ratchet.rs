use crate::floors::{self, Floors, Measurement};
use crate::mutation::outcomes;
use crate::mutation::sweep::{self, CrateResult};
use std::path::Path;

const RAISE_DIR: &str = "target/mutation/raise";

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Origin {
    pub commit: String,
    pub run_url: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Raise {
    pub package: String,
    pub old: u64,
    pub new: u64,
    pub current: Origin,
    pub previous: Origin,
    pub measurement: Measurement,
}

fn score_of(found: &CrateResult) -> Result<Option<u64>, String> {
    let counted = outcomes::score(found.caught, found.timeout, found.missed);
    if counted == found.score {
        Ok(counted)
    } else {
        Err(format!(
            "{}: the sweep of run {} records a score that its counts do not give",
            found.package, found.run_id
        ))
    }
}

fn origin(found: &CrateResult) -> Origin {
    Origin {
        commit: found.commit.clone(),
        run_url: found.run_url.clone(),
    }
}

pub(crate) fn decide(
    floors: &Floors,
    current: &[CrateResult],
    previous: &[CrateResult],
) -> Result<Vec<Raise>, String> {
    let mut sweeps: Vec<&CrateResult> = current.iter().collect();
    sweeps.sort_by(|a, b| a.package.cmp(&b.package));
    let mut raises = Vec::new();
    let mut problems = Vec::new();
    for now in sweeps {
        let Some(old) = floors.hundredths(&now.package) else {
            continue;
        };
        let package = &now.package;
        let Some(score) = score_of(now)? else {
            problems.push(format!(
                "{package}: the sweep counted no mutant, so it cannot meet the floor {}",
                floors::show(old)
            ));
            continue;
        };
        if score < old {
            problems.push(format!(
                "{package}: the sweep scores {} against the floor {}; add tests that kill the surviving mutants of {package}",
                floors::show(score),
                floors::show(old)
            ));
            continue;
        }
        if now.branch != sweep::MAIN {
            continue;
        }
        let earlier = previous
            .iter()
            .filter(|earlier| earlier.package == *package && earlier.branch == sweep::MAIN)
            .max_by_key(|earlier| earlier.run_id);
        let Some(earlier) = earlier else {
            continue;
        };
        let Some(earlier_score) = score_of(earlier)? else {
            continue;
        };
        let (lower, new) = if earlier_score <= score {
            (earlier, earlier_score)
        } else {
            (now, score)
        };
        if new <= old {
            continue;
        }
        raises.push(Raise {
            package: package.clone(),
            old,
            new,
            current: origin(now),
            previous: origin(earlier),
            measurement: Measurement {
                package: package.clone(),
                cargo_mutants: lower.cargo_mutants.clone(),
                nextest: lower.nextest.clone(),
                toolchain: lower.toolchain.clone(),
                measured: lower.date.clone(),
                commit: lower.commit.clone(),
                caught: lower.caught,
                missed: lower.missed,
                timeout: lower.timeout,
                unviable: lower.unviable,
            },
        });
    }
    if problems.is_empty() {
        Ok(raises)
    } else {
        Err(problems.join("\n"))
    }
}

const INTRO: &str = "Two sweeps of the main branch agree on a higher mutation score for the crates below, so each floor in quality/floors.toml rises to the lower of the two scores, rounded down to hundredths. A floor never falls, and this change enrols no crate.\n\n";
const OUTRO: &str = "Each later sweep that proposes a raise rewrites this branch. Merge it by rebase once the checks have passed.\n";

pub(crate) fn body(raises: &[Raise]) -> String {
    let mut text = String::from(INTRO);
    for raise in raises {
        text.push_str(&format!(
            "### {}\n\nThe floor rises from {} to {}.\n\n- current sweep: commit {}, {}\n- previous sweep: commit {}, {}\n\n",
            raise.package,
            floors::show(raise.old),
            floors::show(raise.new),
            raise.current.commit,
            raise.current.run_url,
            raise.previous.commit,
            raise.previous.run_url
        ));
    }
    text.push_str(OUTRO);
    text
}

pub(crate) fn run(repo: &Path) -> Result<usize, String> {
    let path = repo.join(floors::FLOORS_FILE);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("could not read {}: {e}", path.display()))?;
    let floors =
        floors::parse(&text).map_err(|e| format!("{} is malformed: {e}", floors::FLOORS_FILE))?;
    let current = sweep::read_results(repo)?;
    let previous = sweep::read_previous(repo)?;
    let raises = decide(&floors, &current, &previous)?;
    let dir = repo.join(RAISE_DIR);
    for stale in ["floors.toml", "body.md"] {
        match std::fs::remove_file(dir.join(stale)) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(format!(
                    "could not remove {}: {e}",
                    dir.join(stale).display()
                ));
            }
        }
    }
    if raises.is_empty() {
        return Ok(0);
    }
    let measurements: Vec<Measurement> = raises.iter().map(|r| r.measurement.clone()).collect();
    let rewritten = floors::raise(&text, &measurements)?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    for (name, content) in [("floors.toml", rewritten), ("body.md", body(&raises))] {
        std::fs::write(dir.join(name), content)
            .map_err(|e| format!("could not write {}: {e}", dir.join(name).display()))?;
    }
    for raise in &raises {
        println!(
            "{}: {} to {}",
            raise.package,
            floors::show(raise.old),
            floors::show(raise.new)
        );
    }
    Ok(raises.len())
}

pub(crate) fn command() -> Result<(), String> {
    let repo = crate::checks::root_of(Path::new("."))?;
    let raises = run(&repo)?;
    println!("raises={raises}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::floors::tests::{measured, unmeasured};
    use crate::mutation::sweep::tests::result;
    use crate::scope::tests::Repo;
    use std::collections::BTreeMap;

    fn floors_with(entries: &[(&str, Option<u64>)]) -> Floors {
        let text: String = entries
            .iter()
            .map(|(name, floor)| match floor {
                Some(f) => measured(name, &floors::show(*f), (*f, 0, 100 - *f)).text(),
                None => unmeasured(name),
            })
            .collect();
        floors::parse(&text).unwrap()
    }

    enum Expect {
        Nothing,
        Raised { new: u64, from_commit: &'static str },
        Fails(&'static [&'static str]),
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "a table of sweep pairs with the floor each leaves"
    )]
    fn a_floor_rises_only_when_two_sweeps_agree() {
        struct Row {
            name: &'static str,
            floor: Option<u64>,
            current: u64,
            previous: Option<(u64, &'static str)>,
            expect: Expect,
        }
        let rows = [
            Row {
                name: "two sweeps above the floor raise it to the lower",
                floor: Some(80),
                current: 85,
                previous: Some((83, "main")),
                expect: Expect::Raised {
                    new: 83,
                    from_commit: "previous",
                },
            },
            Row {
                name: "the current sweep is the lower one",
                floor: Some(80),
                current: 84,
                previous: Some((90, "main")),
                expect: Expect::Raised {
                    new: 84,
                    from_commit: "current",
                },
            },
            Row {
                name: "no previous sweep means no raise",
                floor: Some(80),
                current: 85,
                previous: None,
                expect: Expect::Nothing,
            },
            Row {
                name: "a lower score equal to the floor is no raise",
                floor: Some(83),
                current: 85,
                previous: Some((83, "main")),
                expect: Expect::Nothing,
            },
            Row {
                name: "a lower score under the floor is no raise",
                floor: Some(83),
                current: 90,
                previous: Some((80, "main")),
                expect: Expect::Nothing,
            },
            Row {
                name: "a sweep below the floor fails the run",
                floor: Some(83),
                current: 82,
                previous: Some((90, "main")),
                expect: Expect::Fails(&["a", "0.82", "0.83"]),
            },
            Row {
                name: "an unmeasured crate is never enrolled",
                floor: None,
                current: 95,
                previous: Some((95, "main")),
                expect: Expect::Nothing,
            },
            Row {
                name: "a previous sweep of another branch is ignored",
                floor: Some(80),
                current: 85,
                previous: Some((83, "feature")),
                expect: Expect::Nothing,
            },
        ];
        let mut wrong = Vec::new();
        for row in rows {
            let floors = floors_with(&[("a", row.floor)]);
            let current = [result("a", "current", "main", 2, row.current)];
            let previous: Vec<CrateResult> = row
                .previous
                .iter()
                .map(|(score, branch)| result("a", "previous", branch, 1, *score))
                .collect();
            let found = decide(&floors, &current, &previous);
            let problem = match (&row.expect, &found) {
                (Expect::Nothing, Ok(raises)) if raises.is_empty() => None,
                (Expect::Raised { new, from_commit }, Ok(raises)) => match raises.as_slice() {
                    [raise]
                        if raise.new == *new
                            && raise.old == row.floor.unwrap()
                            && raise.measurement.commit == *from_commit
                            && raise.measurement.hundredths() == Some(*new)
                            && raise.current.commit == "current"
                            && raise.previous.commit == "previous" =>
                    {
                        None
                    }
                    other => Some(format!("unexpected raises {other:?}")),
                },
                (Expect::Fails(parts), Err(message))
                    if parts.iter().all(|p| message.contains(p)) =>
                {
                    None
                }
                (_, other) => Some(format!("unexpected outcome {other:?}")),
            };
            if let Some(problem) = problem {
                wrong.push(format!("{}: {problem}", row.name));
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }

    fn setup(label: &str) -> Repo {
        let repo = Repo::new(label);
        repo.write(
            floors::FLOORS_FILE,
            &format!(
                "{}{}",
                measured("a", "0.80", (80, 0, 20)).text(),
                measured("b", "0.90", (90, 0, 10)).text()
            ),
        );
        repo
    }

    fn write_result(repo: &Repo, dir: &str, found: &CrateResult) {
        repo.write(
            &format!("{}/{dir}/{}.json", sweep::SWEEP_DIR, found.package),
            &serde_json::to_string(found).unwrap(),
        );
    }

    #[test]
    fn a_missing_or_empty_previous_index_means_no_raise() {
        let rows: [(&str, Option<&str>, usize); 4] = [
            ("an absent index", None, 0),
            ("an empty index", Some("[]"), 0),
            (
                "an index listing only another crate",
                Some(r#"[{"crate":"b","commit":"previous","run_id":1,"file":"b.json"}]"#),
                0,
            ),
            (
                "an index listing the crate's own earlier sweep",
                Some(r#"[{"crate":"a","commit":"previous","run_id":1,"file":"a.json"}]"#),
                1,
            ),
        ];
        let mut wrong = Vec::new();
        for (name, index, raises) in rows {
            let repo = setup("no-previous");
            write_result(&repo, "results", &result("a", "current", "main", 2, 85));
            if let Some(index) = index {
                write_result(&repo, "previous", &result("b", "previous", "main", 1, 95));
                write_result(&repo, "previous", &result("a", "previous", "main", 1, 83));
                repo.write(&format!("{}/previous/index.json", sweep::SWEEP_DIR), index);
            }
            let found = run(&repo.0);
            let written = repo.0.join(RAISE_DIR).join("floors.toml").exists();
            if found != Ok(raises) || written != (raises > 0) {
                wrong.push(format!("{name}: {found:?}, floors file written {written}"));
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }

    fn tables(text: &str) -> BTreeMap<String, String> {
        text.split("[crate.")
            .skip(1)
            .map(|table| {
                let (name, _) = table.split_once(']').unwrap();
                (name.to_owned(), table.to_owned())
            })
            .collect()
    }

    #[test]
    fn a_ratchet_with_raises_writes_the_floors_file_and_the_body() {
        let repo = setup("with-raise");
        let input = std::fs::read_to_string(repo.0.join(floors::FLOORS_FILE)).unwrap();
        write_result(&repo, "results", &result("a", "current", "main", 2, 85));
        write_result(&repo, "results", &result("b", "current", "main", 2, 90));
        write_result(&repo, "previous", &result("a", "previous", "main", 1, 83));
        write_result(&repo, "previous", &result("b", "previous", "main", 1, 90));
        repo.write(
            &format!("{}/previous/index.json", sweep::SWEEP_DIR),
            r#"[{"crate":"a","commit":"previous","run_id":1,"file":"a.json"},{"crate":"b","commit":"previous","run_id":1,"file":"b.json"}]"#,
        );
        assert_eq!(run(&repo.0), Ok(1));
        let floors_out =
            std::fs::read_to_string(repo.0.join(RAISE_DIR).join("floors.toml")).unwrap();
        let body_out = std::fs::read_to_string(repo.0.join(RAISE_DIR).join("body.md")).unwrap();
        let (before, after) = (tables(&input), tables(&floors_out));
        assert_eq!(
            before.keys().collect::<Vec<_>>(),
            after.keys().collect::<Vec<_>>()
        );
        assert_eq!(before["b"], after["b"]);
        assert_ne!(before["a"], after["a"]);
        assert!(after["a"].contains("floor = 0.83"), "{}", after["a"]);
        assert!(
            body_out.contains('a') && body_out.contains("0.83"),
            "{body_out}"
        );

        let quiet = setup("without-raise");
        write_result(&quiet, "results", &result("a", "current", "main", 2, 85));
        assert_eq!(run(&quiet.0), Ok(0));
        let dir = quiet.0.join(RAISE_DIR);
        assert!(!dir.join("floors.toml").exists() && !dir.join("body.md").exists());
    }

    #[test]
    fn the_raise_body_names_each_crate_with_both_sweeps() {
        let floors = floors_with(&[("a", Some(80)), ("b", Some(90))]);
        let current = [
            result("b", "cb", "main", 4, 95),
            result("a", "ca", "main", 3, 85),
        ];
        let previous = [
            result("a", "pa", "main", 1, 83),
            result("b", "pb", "main", 2, 93),
        ];
        let raises = decide(&floors, &current, &previous).unwrap();
        let text = body(&raises);
        assert!(!text.contains('@'), "{text}");
        let mut last = 0;
        for needles in [
            ["a", "0.80", "0.83", "ca", "pa", "/runs/3", "/runs/1"],
            ["b", "0.90", "0.93", "cb", "pb", "/runs/4", "/runs/2"],
        ] {
            let at = text.find(&format!("### {}", needles[0])).unwrap_or(0);
            assert!(at >= last, "crates are listed out of order in {text}");
            last = at;
            for needle in &needles[1..] {
                assert!(text[at..].contains(needle), "{needle} after {at} in {text}");
            }
        }
    }
}
