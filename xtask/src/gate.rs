use crate::scope::{self, Head};
use crate::size::{self, LIMITS_FILE};
use crate::unused_deps::{self, EXEMPTIONS_FILE};
use std::collections::BTreeSet;
use std::path::Path;

const ATTRIBUTES_FILE: &str = ".gitattributes";
const OTHERS_SHOWN: usize = 10;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Direction {
    Loosening,
    Unchanged,
    Tightening,
}

struct Sides<'a> {
    base: Option<&'a str>,
    head: Option<&'a str>,
}

type Matcher = fn(&str) -> bool;
type Judge = fn(&Sides) -> Result<Direction, String>;

const DIRECTIONS: [(Matcher, Judge); 4] = [
    (is_limits_file, limits_direction),
    (is_attributes_file, attributes_direction),
    (is_exemptions_file, exemptions_direction),
    (is_quality_file, quality_direction),
];

fn is_limits_file(path: &str) -> bool {
    path == LIMITS_FILE
}

fn is_exemptions_file(path: &str) -> bool {
    path == EXEMPTIONS_FILE
}

fn is_attributes_file(path: &str) -> bool {
    path.rsplit('/').next() == Some(ATTRIBUTES_FILE)
}

fn is_quality_file(path: &str) -> bool {
    path.starts_with("quality/")
}

fn limits_direction(sides: &Sides) -> Result<Direction, String> {
    let Some(base) = sides.base else {
        return Err(format!(
            "{LIMITS_FILE} has no earlier version to compare with"
        ));
    };
    let base = size::parse_limits(base).map_err(|e| format!("{LIMITS_FILE} is malformed: {e}"))?;
    let head = sides.head.and_then(|text| size::parse_limits(text).ok());
    Ok(match head {
        None => Direction::Loosening,
        Some(head) => match head.size.max_files.cmp(&base.size.max_files) {
            std::cmp::Ordering::Greater => Direction::Loosening,
            std::cmp::Ordering::Equal => Direction::Unchanged,
            std::cmp::Ordering::Less => Direction::Tightening,
        },
    })
}

fn generated_lines(text: &str) -> BTreeSet<&str> {
    text.split('\n')
        .filter(|line| line.contains("linguist-generated"))
        .collect()
}

fn attributes_direction(sides: &Sides) -> Result<Direction, String> {
    let Some(head) = sides.head else {
        return Ok(Direction::Loosening);
    };
    let before = sides.base.map(generated_lines).unwrap_or_default();
    let after = generated_lines(head);
    Ok(if !after.is_subset(&before) {
        Direction::Loosening
    } else if after.len() < before.len() {
        Direction::Tightening
    } else {
        Direction::Unchanged
    })
}

fn exemptions_direction(sides: &Sides) -> Result<Direction, String> {
    let Some(base) = sides.base else {
        return Ok(Direction::Unchanged);
    };
    let base = unused_deps::exemption_pairs(base, "at the base")?;
    let head = sides
        .head
        .and_then(|text| unused_deps::exemption_pairs(text, "at the head").ok());
    Ok(match head {
        None => Direction::Loosening,
        Some(head) if !head.is_subset(&base) => Direction::Loosening,
        Some(head) if head.len() < base.len() => Direction::Tightening,
        Some(_) => Direction::Unchanged,
    })
}

fn quality_direction(sides: &Sides) -> Result<Direction, String> {
    Ok(if sides.base.is_some() {
        Direction::Loosening
    } else {
        Direction::Unchanged
    })
}

fn newly_generated(repo: &Path, base: &str, head: &Head) -> Result<bool, String> {
    let mut paths: BTreeSet<String> = scope::tracked(repo, &Head::Commit(base.to_owned()))?
        .into_iter()
        .collect();
    paths.extend(scope::tracked(repo, head)?);
    let paths: Vec<String> = paths.into_iter().collect();
    let before = scope::attributes(repo, Some(base), &paths)?;
    let after = scope::attributes(repo, head.source(), &paths)?;
    Ok(paths.iter().any(|path| {
        let generated = |found: &std::collections::BTreeMap<String, String>| {
            found
                .get(path)
                .is_some_and(|value| scope::is_generated_for_review(value))
        };
        generated(&after) && !generated(&before)
    }))
}

fn root_manifest_is_version_only(repo: &Path, base: &str, head: &Head) -> Result<bool, String> {
    let before = scope::read_at(repo, base, "Cargo.toml")?;
    let after = scope::head_text(repo, head, "Cargo.toml")?;
    Ok(match (before, after) {
        (Some(before), Some(after)) => scope::manifest_version_only(&before, &after).is_some(),
        _ => false,
    })
}

pub fn check(repo: &Path, base: &str, head: &Head) -> Result<String, String> {
    let Some(enforced) = size::limits_at(repo, base, head)? else {
        return Ok(format!(
            "gate configuration not in force yet at {}",
            scope::short(base)
        ));
    };
    let changed = scope::diff_names(repo, base, head)?;
    let mut loosening = BTreeSet::new();
    let mut others = BTreeSet::new();
    let mut tightened = false;
    for path in &changed {
        if let Some((_, judge)) = DIRECTIONS.iter().find(|(matches, _)| matches(path)) {
            let before = if is_limits_file(path) {
                Some(enforced.reference.clone())
            } else {
                scope::read_at(repo, base, path)?
            };
            let after = scope::head_text(repo, head, path)?;
            let sides = Sides {
                base: before.as_deref(),
                head: after.as_deref(),
            };
            match judge(&sides)? {
                Direction::Loosening => {
                    loosening.insert(path.as_str());
                }
                Direction::Tightening => tightened = true,
                Direction::Unchanged => {}
            }
        } else if path == "Cargo.lock" {
            continue;
        } else if path == "Cargo.toml" {
            if !root_manifest_is_version_only(repo, base, head)? {
                others.insert(path.as_str());
            }
        } else {
            others.insert(path.as_str());
        }
    }
    let attribute_files: Vec<&str> = changed
        .iter()
        .map(String::as_str)
        .filter(|path| is_attributes_file(path))
        .collect();
    if !attribute_files.is_empty() && newly_generated(repo, base, head)? {
        loosening.extend(attribute_files);
    }
    if loosening.is_empty() {
        return Ok(if tightened {
            "gate configuration tightened"
        } else {
            "no gate change"
        }
        .to_owned());
    }
    if others.is_empty() {
        return Ok("gate configuration loosened alone".to_owned());
    }
    let shown: Vec<&str> = others.iter().copied().take(OTHERS_SHOWN).collect();
    let more = others.len() - shown.len();
    let tail = if more > 0 {
        format!(" and {more} more")
    } else {
        String::new()
    };
    Err(format!(
        "gate configuration is loosened in {} beside other changes ({}{tail}); ship the loosened gate configuration in a pull request of its own; only the version commit may sit beside it",
        loosening.into_iter().collect::<Vec<_>>().join(", "),
        shown.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scope::{resolve_base, tests::Repo};
    use crate::size::{self, LIMITS_FILE};

    const LIMIT_100: &str = "[size]\nmax_files = 100\n";
    const LIMIT_200: &str = "[size]\nmax_files = 200\n";
    const CRATE_FILE: &str = "crates/x/src/lib.rs";
    const XTASK_FILE: &str = "xtask/src/main.rs";
    const ATTRIBUTES: &str = ".gitattributes";

    enum Base {
        Standard,
        NeverCarried,
        Removed,
        ExemptionsRemoved,
    }

    enum Expect {
        Pass(&'static str),
        Fail(&'static str),
    }

    type Edit = (String, Option<String>);

    fn put(path: &str, text: &str) -> Edit {
        (path.to_owned(), Some(text.to_owned()))
    }

    fn del(path: &str) -> Edit {
        (path.to_owned(), None)
    }

    const EXEMPTIONS: &str = EXEMPTIONS_FILE;

    fn pairs(list: &[(&str, &str)], reason: &str) -> String {
        list.iter()
            .map(|(package, dependency)| {
                format!(
                    "[[exemption]]\npackage = \"{package}\"\ndependency = \"{dependency}\"\nreason = \"{reason}\"\n\n"
                )
            })
            .collect()
    }

    fn crate_change() -> Edit {
        put(CRATE_FILE, "fn changed() {}\n")
    }

    fn manifest(version: &str) -> String {
        format!(
            "[package]\nname = \"mcrs\"\nversion = \"{version}\"\n\n[workspace]\nmembers = [\"crates/*\"]\n\n\
             [workspace.package]\nversion = \"{version}\"\nedition = \"2024\"\n"
        )
    }

    fn lock(version: &str) -> String {
        format!("version = 4\n\n[[package]]\nname = \"mcrs\"\nversion = \"{version}\"\n")
    }

    fn apply(repo: &Repo, edits: &[Edit]) {
        for (path, text) in edits {
            match text {
                Some(text) => repo.write(path, text),
                None => repo.remove(path),
            }
        }
    }

    fn standard(repo: &Repo, with_limits: bool) {
        repo.write("Cargo.toml", &manifest("0.5.1"));
        repo.write("Cargo.lock", &lock("0.5.1"));
        repo.write(ATTRIBUTES, "gen/** linguist-generated\n");
        repo.write("quality/other.toml", "a = 1\n");
        repo.write(CRATE_FILE, "fn a() {}\n");
        repo.write(XTASK_FILE, "fn main() {}\n");
        repo.write(".github/workflows/w.yml", "name: w\n");
        repo.write(".config/nextest.toml", "[profile.default]\n");
        if with_limits {
            repo.write(LIMITS_FILE, LIMIT_100);
        }
    }

    struct Row {
        name: &'static str,
        base: Base,
        base_edits: Vec<Edit>,
        edits: Vec<Edit>,
        gate: Expect,
        size: Option<Expect>,
    }

    fn row(name: &'static str, edits: Vec<Edit>, gate: Expect) -> Row {
        Row {
            name,
            base: Base::Standard,
            base_edits: Vec::new(),
            edits,
            gate,
            size: None,
        }
    }

    fn files(count: usize) -> Vec<Edit> {
        (0..count)
            .map(|n| put(&format!("crates/x/f{n}.txt"), "x\n"))
            .collect()
    }

    fn rows() -> Vec<Row> {
        let raise = || put(LIMITS_FILE, LIMIT_200);
        let loosened = Expect::Pass("gate configuration loosened alone");
        let shipped_alone = "pull request of its own";
        let one = || pairs(&[("c", "b")], "needed");
        let two = || pairs(&[("c", "b"), ("a", "b")], "needed");
        let exemptions = |base: String| vec![put(EXEMPTIONS, &base)];
        vec![
            Row {
                base_edits: exemptions(one()),
                ..row(
                    "an added exemption beside a crate file fails",
                    vec![put(EXEMPTIONS, &two()), crate_change()],
                    Expect::Fail(shipped_alone),
                )
            },
            Row {
                base_edits: exemptions(one()),
                ..row(
                    "an added exemption alone passes",
                    vec![put(EXEMPTIONS, &two())],
                    Expect::Pass("gate configuration loosened alone"),
                )
            },
            Row {
                base_edits: exemptions(two()),
                ..row(
                    "a removed exemption beside a crate file passes",
                    vec![put(EXEMPTIONS, &one()), crate_change()],
                    Expect::Pass("gate configuration tightened"),
                )
            },
            Row {
                base_edits: exemptions(one()),
                ..row(
                    "a reworded reason beside a crate file passes",
                    vec![
                        put(
                            EXEMPTIONS,
                            &pairs(&[("c", "b")], "needed by the wasm build"),
                        ),
                        crate_change(),
                    ],
                    Expect::Pass("no gate change"),
                )
            },
            Row {
                base_edits: exemptions(one()),
                ..row(
                    "a deleted exemption file beside a crate file fails",
                    vec![del(EXEMPTIONS), crate_change()],
                    Expect::Fail(shipped_alone),
                )
            },
            Row {
                base_edits: exemptions(one()),
                ..row(
                    "an unparsable exemption file beside a crate file fails",
                    vec![put(EXEMPTIONS, "not toml ["), crate_change()],
                    Expect::Fail(shipped_alone),
                )
            },
            row(
                "an exemption file introduced with two pairs passes beside a crate file",
                vec![put(EXEMPTIONS, &two()), crate_change()],
                Expect::Pass("no gate change"),
            ),
            Row {
                base: Base::ExemptionsRemoved,
                ..row(
                    "an exemption file added back after an earlier removal passes as an introduction",
                    vec![put(EXEMPTIONS, &two()), crate_change()],
                    Expect::Pass("no gate change"),
                )
            },
            row("a limit raise alone passes", vec![raise()], loosened),
            row(
                "a limit raise with a crate file fails",
                vec![raise(), crate_change()],
                Expect::Fail(shipped_alone),
            ),
            row(
                "a limit raise with the version lines and the lock passes",
                vec![
                    raise(),
                    put("Cargo.toml", &manifest("0.5.2")),
                    put("Cargo.lock", &lock("0.5.2")),
                ],
                Expect::Pass("gate configuration loosened alone"),
            ),
            row(
                "a limit raise beside an xtask file fails",
                vec![raise(), put(XTASK_FILE, "fn main() { changed(); }\n")],
                Expect::Fail(XTASK_FILE),
            ),
            row(
                "a limit raise beside a manifest change beyond the version lines fails",
                vec![
                    raise(),
                    put("Cargo.toml", &format!("{}# more\n", manifest("0.5.1"))),
                ],
                Expect::Fail("Cargo.toml"),
            ),
            row(
                "a limit cut with a crate file passes",
                vec![put(LIMITS_FILE, "[size]\nmax_files = 99\n"), crate_change()],
                Expect::Pass("gate configuration tightened"),
            ),
            row(
                "the same limit written with digit separators is no change",
                vec![
                    put(LIMITS_FILE, "[size]\nmax_files = 1_00\n"),
                    crate_change(),
                ],
                Expect::Pass("no gate change"),
            ),
            row(
                "a head that cannot be parsed is loosening",
                vec![put(LIMITS_FILE, "max_files = \"x\"\n"), crate_change()],
                Expect::Fail(LIMITS_FILE),
            ),
            row(
                "a head without the size table is loosening",
                vec![put(LIMITS_FILE, "[other]\nx = 1\n"), crate_change()],
                Expect::Fail(LIMITS_FILE),
            ),
            row(
                "a new generated line with a crate file fails",
                vec![
                    put(
                        ATTRIBUTES,
                        "gen/** linguist-generated\ncrates/** linguist-generated\n",
                    ),
                    crate_change(),
                ],
                Expect::Fail(ATTRIBUTES),
            ),
            Row {
                base_edits: vec![
                    put(
                        ATTRIBUTES,
                        "a/** linguist-generated\na/keep/** -linguist-generated\n",
                    ),
                    put("a/keep/k.txt", "k\n"),
                ],
                ..row(
                    "two lines swapped with a crate file fails",
                    vec![
                        put(
                            ATTRIBUTES,
                            "a/keep/** -linguist-generated\na/** linguist-generated\n",
                        ),
                        crate_change(),
                    ],
                    Expect::Fail(ATTRIBUTES),
                )
            },
            Row {
                base_edits: vec![put(
                    ATTRIBUTES,
                    "gen/** linguist-generated\n[attr]gen linguist-generated\n",
                )],
                ..row(
                    "a macro that expands to the attribute fails",
                    vec![
                        put(
                            ATTRIBUTES,
                            "gen/** linguist-generated\n[attr]gen linguist-generated\nfoo/** gen\n",
                        ),
                        put("foo/a.txt", "x\n"),
                        crate_change(),
                    ],
                    Expect::Fail(ATTRIBUTES),
                )
            },
            row(
                "a nested attributes file that marks everything fails",
                vec![
                    put("crates/x/.gitattributes", "* linguist-generated\n"),
                    crate_change(),
                ],
                Expect::Fail("crates/x/.gitattributes"),
            ),
            row(
                "an attribute with an explicit true value fails",
                vec![
                    put(
                        ATTRIBUTES,
                        "gen/** linguist-generated\nbar/** linguist-generated=true\n",
                    ),
                    crate_change(),
                ],
                Expect::Fail(ATTRIBUTES),
            ),
            row(
                "a deleted attributes file with a crate file fails",
                vec![del(ATTRIBUTES), crate_change()],
                Expect::Fail(ATTRIBUTES),
            ),
            row(
                "a deleted limits file with a crate file fails",
                vec![del(LIMITS_FILE), crate_change()],
                Expect::Fail(LIMITS_FILE),
            ),
            row(
                "a quality file absent from the base introduces a gate and may ship with other changes",
                vec![
                    put("quality/new.toml", "b = 1\n"),
                    put(XTASK_FILE, "fn main() { changed(); }\n"),
                    crate_change(),
                ],
                Expect::Pass("no gate change"),
            ),
            row(
                "a quality file on the base changed beside a crate file fails",
                vec![put("quality/other.toml", "a = 2\n"), crate_change()],
                Expect::Fail("quality/other.toml"),
            ),
            row(
                "a quality file on the base changed alone passes",
                vec![put("quality/other.toml", "a = 2\n")],
                Expect::Pass("gate configuration loosened alone"),
            ),
            row(
                "a quality file on the base deleted beside a crate file fails",
                vec![del("quality/other.toml"), crate_change()],
                Expect::Fail("quality/other.toml"),
            ),
            row(
                "tool configuration and workflows are ordinary paths",
                vec![
                    put(XTASK_FILE, "fn main() { changed(); }\n"),
                    put(".github/workflows/w.yml", "name: changed\n"),
                    put(".config/nextest.toml", "[profile.ci]\n"),
                    crate_change(),
                ],
                Expect::Pass("no gate change"),
            ),
            Row {
                base: Base::NeverCarried,
                size: Some(Expect::Pass("limit not in force yet")),
                ..row(
                    "a base whose history never carried the limits file passes everything",
                    vec![
                        put(
                            ATTRIBUTES,
                            "gen/** linguist-generated\ndocs/** linguist-generated\n",
                        ),
                        crate_change(),
                    ],
                    Expect::Pass("gate configuration not in force yet"),
                )
            },
            Row {
                base: Base::Removed,
                size: Some(Expect::Fail(
                    "quality/limits.toml was removed from the base; restore it",
                )),
                ..row(
                    "a removed limits file fails until it is restored",
                    vec![crate_change()],
                    Expect::Fail("quality/limits.toml was removed from the base; restore it"),
                )
            },
            Row {
                base: Base::Removed,
                size: Some(Expect::Pass("of 100 files")),
                ..row(
                    "a restore at the last value beside other changes passes",
                    vec![put(LIMITS_FILE, LIMIT_100), crate_change()],
                    Expect::Pass("no gate change"),
                )
            },
            Row {
                base: Base::Removed,
                size: Some(Expect::Pass("of 200 files")),
                ..row(
                    "a restore at a higher value beside other changes fails",
                    vec![put(LIMITS_FILE, LIMIT_200), crate_change()],
                    Expect::Fail(LIMITS_FILE),
                )
            },
            Row {
                base: Base::Removed,
                size: Some(Expect::Fail("more than the limit of 100")),
                ..row(
                    "a restore that changes a hundred and one files fails size",
                    [vec![put(LIMITS_FILE, LIMIT_100)], files(100)].concat(),
                    Expect::Pass("no gate change"),
                )
            },
            Row {
                base_edits: vec![put(LIMITS_FILE, "[size]\nmax_files = 100\nmax_lines = 5\n")],
                size: Some(Expect::Fail("malformed")),
                ..row(
                    "a malformed base file fails",
                    vec![crate_change()],
                    Expect::Fail("malformed"),
                )
            },
        ]
    }

    fn build(row: &Row) -> (Repo, String, Head) {
        let repo = Repo::new("gate");
        repo.init();
        standard(&repo, !matches!(row.base, Base::NeverCarried));
        if matches!(row.base, Base::Removed) {
            repo.commit("carry the limits file");
            repo.remove(LIMITS_FILE);
        }
        if matches!(row.base, Base::ExemptionsRemoved) {
            repo.write(EXEMPTIONS, &pairs(&[("c", "b")], "needed"));
            repo.commit("carry the exemption file");
            repo.remove(EXEMPTIONS);
        }
        apply(&repo, &row.base_edits);
        repo.commit("base");
        let base = repo.sha();
        apply(&repo, &row.edits);
        repo.commit("head");
        let head = Head::Commit(repo.sha());
        (repo, base, head)
    }

    fn verify(name: &str, what: &str, found: Result<String, String>, expect: &Expect) {
        match (expect, found) {
            (Expect::Pass(part), Ok(detail)) => {
                assert!(detail.contains(part), "{name} ({what}): {detail}")
            }
            (Expect::Fail(part), Err(message)) => {
                assert!(message.contains(part), "{name} ({what}): {message}")
            }
            (_, other) => panic!("{name} ({what}): {other:?}"),
        }
    }

    #[test]
    fn gate_config_requires_loosened_configuration_to_ship_alone() {
        for row in rows() {
            let (repo, base, head) = build(&row);
            verify(
                row.name,
                "gate-config",
                check(&repo.0, &base, &head),
                &row.gate,
            );
            if let Some(expect) = &row.size {
                verify(row.name, "size", size::check(&repo.0, &base, &head), expect);
            }
        }
    }

    #[test]
    fn a_policy_head_is_judged_as_data_while_head_stays_at_the_base() {
        let repo = Repo::new("policy");
        repo.init();
        standard(&repo, true);
        repo.commit("base");
        let base = repo.sha();
        repo.git(&["checkout", "--quiet", "-b", "topic"]);
        apply(
            &repo,
            &[vec![put(LIMITS_FILE, LIMIT_200)], files(101)].concat(),
        );
        repo.commit("raise the limit and add files");
        let head = Head::Commit(repo.sha());
        repo.git(&["checkout", "--quiet", "main"]);
        assert_eq!(resolve_base(&repo.0, "main", &head).unwrap(), base);

        let (status, tip) = (repo.status(), repo.sha());
        let size = size::check(&repo.0, &base, &head).unwrap_err();
        assert!(size.contains("more than the limit of 100"), "{size}");
        let gate = check(&repo.0, &base, &head).unwrap_err();
        assert!(gate.contains(LIMITS_FILE), "{gate}");
        assert_eq!((repo.status(), repo.sha()), (status, tip));
        assert!(!repo.0.join("crates/x/f0.txt").exists());
    }
}
