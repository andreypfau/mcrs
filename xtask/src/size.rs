use crate::git;
use crate::scope::{self, Head};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::Path;

pub const LIMITS_FILE: &str = "quality/limits.toml";

const SOURCE_SUFFIXES: [&str; 7] = [".rs", ".wgsl", ".java", ".sh", ".py", ".gradle", ".bat"];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub size: SizeLimits,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SizeLimits {
    pub max_files: u32,
}

pub struct Enforced {
    pub limits: Limits,
    pub reference: String,
}

pub fn parse_limits(text: &str) -> Result<Limits, String> {
    toml::from_str(text).map_err(|e| e.message().to_owned())
}

pub fn limits_at(repo: &Path, base: &str, head: &Head) -> Result<Option<Enforced>, String> {
    let short = scope::short(base);
    if let Some(reference) = scope::read_at(repo, base, LIMITS_FILE)? {
        let limits = parse_limits(&reference).map_err(|e| {
            format!(
                "{LIMITS_FILE} at the base {short} is malformed: {e}; fix it on the base branch"
            )
        })?;
        return Ok(Some(Enforced { limits, reference }));
    }
    if scope::is_shallow(repo)? {
        return Err(format!(
            "cannot tell from a shallow history whether {LIMITS_FILE} existed; fetch the full history"
        ));
    }
    let removing =
        git::text(repo, &["rev-list", "-1", base, "--", LIMITS_FILE]).map_err(|e| e.to_string())?;
    let removing = removing.trim();
    if removing.is_empty() {
        return Ok(None);
    }
    let restored = scope::head_text(repo, head, LIMITS_FILE)?
        .ok_or_else(|| format!("{LIMITS_FILE} was removed from the base; restore it"))?;
    let limits = parse_limits(&restored)
        .map_err(|e| format!("the restored {LIMITS_FILE} is malformed: {e}"))?;
    let reference =
        scope::read_at(repo, &format!("{removing}~1"), LIMITS_FILE)?.ok_or_else(|| {
            format!("cannot read the last version of {LIMITS_FILE} before it was removed")
        })?;
    Ok(Some(Enforced { limits, reference }))
}

fn is_source(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    !name.contains('.') || SOURCE_SUFFIXES.iter().any(|suffix| name.ends_with(suffix))
}

fn generated_sources(repo: &Path, base: &str, head: &Head) -> Result<(), String> {
    let sources: Vec<String> = scope::tracked(repo, head)?
        .into_iter()
        .filter(|path| is_source(path))
        .collect();
    let at_base = scope::attributes(repo, Some(base), &sources)?;
    let at_head = scope::attributes(repo, head.source(), &sources)?;
    let hidden: BTreeSet<&str> = sources
        .iter()
        .filter(|path| {
            let marked = |found: &std::collections::BTreeMap<String, String>| {
                found.get(*path).is_some_and(|value| scope::is_set(value))
            };
            marked(&at_base) || marked(&at_head)
        })
        .map(String::as_str)
        .collect();
    if hidden.is_empty() {
        return Ok(());
    }
    Err(format!(
        "source files sit under paths marked linguist-generated: {}; narrow the linguist-generated line in .gitattributes so it covers only tool output",
        hidden.into_iter().collect::<Vec<_>>().join(", ")
    ))
}

#[derive(Default)]
struct Counts {
    files: u64,
    added: u64,
    deleted: u64,
    renamed: u64,
    generated: u64,
}

fn count(repo: &Path, base: &str, head: &Head) -> Result<Counts, String> {
    let entries = scope::numstat(repo, base, head)?;
    let mut queried: Vec<String> = entries
        .iter()
        .flat_map(|entry| [Some(entry.path.clone()), entry.from.clone()])
        .flatten()
        .collect();
    queried.sort();
    queried.dedup();
    let generated = scope::attributes(repo, Some(base), &queried)?;
    let is_generated = |path: &str| {
        generated
            .get(path)
            .is_some_and(|value| scope::is_set(value))
    };

    let mut counts = Counts::default();
    let mut carried: Vec<(String, u64)> = Vec::new();
    for entry in &entries {
        let free = is_generated(&entry.path) && entry.from.as_deref().is_none_or(is_generated);
        if free {
            counts.generated += 1;
            continue;
        }
        counts.files += 1;
        let (Some(added), Some(deleted)) = (entry.added, entry.deleted) else {
            continue;
        };
        counts.added += added;
        counts.deleted += deleted;
        if entry.from.is_some() {
            carried.push((entry.path.clone(), added));
        }
    }
    let paths: Vec<String> = carried.iter().map(|(path, _)| path.clone()).collect();
    let totals = scope::line_counts(repo, head, &paths)?;
    counts.renamed = carried
        .iter()
        .zip(totals)
        .map(|((_, added), total)| total.saturating_sub(*added))
        .sum();
    Ok(counts)
}

pub fn check(repo: &Path, base: &str, head: &Head) -> Result<String, String> {
    let enforced = limits_at(repo, base, head)?;
    generated_sources(repo, base, head)?;
    let counts = count(repo, base, head)?;
    let reach = match &enforced {
        Some(enforced) => {
            let limit = u64::from(enforced.limits.size.max_files);
            if counts.files > limit {
                return Err(format!(
                    "{} files changed, more than the limit of {limit}; split the change into pull requests of at most {limit} files",
                    counts.files
                ));
            }
            format!("{} of {limit} files", counts.files)
        }
        None => format!("{} files, limit not in force yet", counts.files),
    };
    Ok(format!(
        "{reach}; {} added, {} deleted, {} renamed lines; {} generated files not counted",
        counts.added, counts.deleted, counts.renamed, counts.generated
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scope::{resolve_base, tests::Repo};
    use std::os::unix::fs::PermissionsExt;

    const LIMIT: &str = "[size]\nmax_files = 100\n";

    enum Expect {
        Pass(String),
        Fail(&'static str),
    }

    type Step = fn(&Repo);

    struct Row {
        name: &'static str,
        limits: Option<&'static str>,
        base: Step,
        head: Step,
        expect: Expect,
    }

    fn nothing(_: &Repo) {}

    fn line(n: usize) -> String {
        format!("line number {n} padded out so that similarity stays high\n")
    }

    fn lines(count: usize) -> String {
        (1..=count).map(line).collect()
    }

    fn edited(count: usize, changed: &[usize]) -> String {
        (1..=count)
            .map(|n| {
                if changed.contains(&n) {
                    format!("replaced content for line {n}\n")
                } else {
                    line(n)
                }
            })
            .collect()
    }

    fn many(repo: &Repo, count: usize) {
        for n in 0..count {
            repo.write(&format!("crates/x/f{n}.txt"), "x\n");
        }
    }

    fn counts(files: u64, added: u64, deleted: u64, renamed: u64, generated: u64) -> Expect {
        Expect::Pass(format!(
            "{files} of 100 files; {added} added, {deleted} deleted, {renamed} renamed lines; {generated} generated files not counted"
        ))
    }

    fn seed(repo: &Repo, limits: Option<&str>) {
        repo.write(".gitattributes", "gen/** linguist-generated\n");
        repo.write("README.txt", "readme\n");
        if let Some(text) = limits {
            repo.write(LIMITS_FILE, text);
        }
    }

    fn row(name: &'static str, base: Step, head: Step, expect: Expect) -> Row {
        Row {
            name,
            limits: Some(LIMIT),
            base,
            head,
            expect,
        }
    }

    fn rows() -> Vec<Row> {
        vec![
            row("a hundred files pass", nothing, |r| many(r, 100), counts(100, 100, 0, 0, 0)),
            row(
                "a hundred and one fail with the count, the limit and the fix",
                nothing,
                |r| many(r, 101),
                Expect::Fail(
                    "101 files changed, more than the limit of 100; split the change into pull requests of at most 100 files",
                ),
            ),
            row(
                "a pure rename of a thousand lines counts one file and no added line",
                |r| r.write("docs/big.txt", &lines(1000)),
                |r| r.rename("docs/big.txt", "docs/moved/big.txt"),
                counts(1, 0, 0, 1000, 0),
            ),
            row(
                "a rename with edits counts the edits and carries the rest",
                |r| r.write("docs/a.txt", &lines(10)),
                |r| {
                    r.remove("docs/a.txt");
                    r.write("docs/b.txt", &edited(10, &[2, 5, 8]));
                },
                counts(1, 3, 3, 7, 0),
            ),
            row(
                "a file under a generated path is not counted",
                nothing,
                |r| {
                    r.write("gen/a.json", "x\n");
                    r.write("src/b.txt", "y\n");
                },
                counts(1, 1, 0, 0, 1),
            ),
            row(
                "a hand-written file renamed into a generated directory is counted",
                |r| r.write("docs/h.txt", &lines(5)),
                |r| r.rename("docs/h.txt", "gen/h.txt"),
                counts(1, 0, 0, 5, 0),
            ),
            row(
                "a rename between generated paths is not counted",
                |r| r.write("gen/a.json", &lines(5)),
                |r| r.rename("gen/a.json", "gen/b.json"),
                counts(0, 0, 0, 0, 1),
            ),
            row(
                "a file of a hundred thousand added lines passes",
                nothing,
                |r| r.write("big/file.txt", &"x\n".repeat(100_000)),
                counts(1, 100_000, 0, 0, 0),
            ),
            row(
                "a binary file, an empty file and a mode change count one each with no lines",
                |r| r.write("tool.txt", "x\n"),
                |r| {
                    r.write_bytes("bin/a.bin", b"\0\0\0\0");
                    r.write("empty.txt", "");
                    let path = r.0.join("tool.txt");
                    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
                },
                counts(3, 0, 0, 0, 0),
            ),
            row(
                "a last line without a newline is a line",
                nothing,
                |r| r.write("t/a.txt", "a\nb"),
                counts(1, 2, 0, 0, 0),
            ),
            row(
                "a renamed last line without a newline is a line",
                |r| r.write("t/a.txt", "a\nb"),
                |r| r.rename("t/a.txt", "t/b.txt"),
                counts(1, 0, 0, 2, 0),
            ),
            row("an empty diff counts nothing", nothing, nothing, counts(0, 0, 0, 0, 0)),
            row(
                "a source file under a path the base marks generated fails",
                |r| r.write("gen/x.rs", "fn a() {}\n"),
                nothing,
                Expect::Fail("gen/x.rs"),
            ),
            row(
                "a gradle file under a generated path fails",
                |r| r.write("gen/build.gradle", "x\n"),
                nothing,
                Expect::Fail("gen/build.gradle"),
            ),
            row(
                "a file with no extension under a generated path fails",
                |r| r.write("gen/Makefile", "x\n"),
                nothing,
                Expect::Fail("gen/Makefile"),
            ),
            row(
                "a source file only the head marks generated fails",
                |r| r.write("other/y.rs", "fn a() {}\n"),
                |r| {
                    r.write(
                        ".gitattributes",
                        "gen/** linguist-generated\nother/** linguist-generated\n",
                    )
                },
                Expect::Fail("other/y.rs"),
            ),
            row(
                "a head that raises its own limit does not get it",
                nothing,
                |r| {
                    r.write(LIMITS_FILE, "[size]\nmax_files = 200\n");
                    many(r, 100);
                },
                Expect::Fail("101 files changed, more than the limit of 100"),
            ),
            row(
                "a head that lowers the limit is not held to it yet",
                nothing,
                |r| {
                    r.write(LIMITS_FILE, "[size]\nmax_files = 50\n");
                    many(r, 59);
                },
                counts(60, 60, 1, 0, 0),
            ),
            Row {
                limits: None,
                ..row(
                    "a base without a limits file passes and says so",
                    nothing,
                    |r| many(r, 3),
                    Expect::Pass(
                        "3 files, limit not in force yet; 3 added, 0 deleted, 0 renamed lines; 0 generated files not counted"
                            .to_owned(),
                    ),
                )
            },
            Row {
                limits: None,
                ..row(
                    "the source rule runs while the limit is not in force",
                    |r| r.write("gen/x.rs", "fn a() {}\n"),
                    nothing,
                    Expect::Fail("gen/x.rs"),
                )
            },
            Row {
                limits: Some("[size]\nmax_files = 100\nmax_lines = 5\n"),
                ..row("an unknown key is malformed", nothing, nothing, Expect::Fail("quality/limits.toml"))
            },
            Row {
                limits: Some("[size]\nmax_files = -1\n"),
                ..row("a negative limit is malformed", nothing, nothing, Expect::Fail("quality/limits.toml"))
            },
            Row {
                limits: Some("[size]\nmax_files = 100.5\n"),
                ..row("a fractional limit is malformed", nothing, nothing, Expect::Fail("quality/limits.toml"))
            },
            Row {
                limits: Some("[size]\nmax_files = \"100\"\n"),
                ..row("a string limit is malformed", nothing, nothing, Expect::Fail("quality/limits.toml"))
            },
            Row {
                limits: Some("[size]\nmax_files =\n"),
                ..row("a missing value is malformed", nothing, nothing, Expect::Fail("quality/limits.toml"))
            },
            Row {
                limits: Some("[other]\nmax_files = 100\n"),
                ..row("a missing size table is malformed", nothing, nothing, Expect::Fail("quality/limits.toml"))
            },
            Row {
                limits: Some("[size]\nmax_files = 1_00\n"),
                ..row("digit separators are the same number", nothing, |r| many(r, 100), counts(100, 100, 0, 0, 0))
            },
        ]
    }

    fn history(label: &str, limits: Option<&str>, base: Step, head: Step) -> (Repo, String, Head) {
        let repo = Repo::new(label);
        repo.init();
        seed(&repo, limits);
        base(&repo);
        repo.commit("base");
        let base_sha = repo.sha();
        head(&repo);
        repo.commit("head");
        let head = Head::Commit(repo.sha());
        (repo, base_sha, head)
    }

    #[test]
    fn size_counts_files_and_lines_against_the_base_limit() {
        for row in rows() {
            let (repo, base, head) = history("size", row.limits, row.base, row.head);
            let found = check(&repo.0, &base, &head);
            match (&row.expect, found) {
                (Expect::Pass(detail), Ok(found)) => assert_eq!(&found, detail, "{}", row.name),
                (Expect::Fail(part), Err(message)) => {
                    assert!(message.contains(part), "{}: {message}", row.name)
                }
                (_, other) => panic!("{}: {other:?}", row.name),
            }
        }
    }

    #[test]
    fn the_limit_is_the_one_on_the_merge_base_until_the_branch_is_rebased() {
        let repo = Repo::new("rebase");
        repo.init();
        seed(&repo, Some(LIMIT));
        repo.commit("base");
        repo.git(&["checkout", "--quiet", "-b", "topic"]);
        many(&repo, 60);
        repo.commit("topic");
        repo.git(&["checkout", "--quiet", "main"]);
        repo.write(LIMITS_FILE, "[size]\nmax_files = 50\n");
        repo.commit("lower the limit");
        repo.git(&["checkout", "--quiet", "topic"]);

        let head = Head::Commit(repo.sha());
        let base = resolve_base(&repo.0, "main", &head).unwrap();
        assert!(
            check(&repo.0, &base, &head)
                .unwrap()
                .starts_with("60 of 100 files")
        );

        repo.git(&["rebase", "--quiet", "main"]);
        let head = Head::Commit(repo.sha());
        let base = resolve_base(&repo.0, "main", &head).unwrap();
        let message = check(&repo.0, &base, &head).unwrap_err();
        assert!(message.contains("more than the limit of 50"), "{message}");
    }

    fn worktree_scenario() -> (Repo, String) {
        let repo = Repo::new("worktree");
        repo.init();
        seed(&repo, Some(LIMIT));
        repo.write("a.txt", &lines(5));
        repo.write("b.txt", "1\n");
        repo.commit("base");
        let base = repo.sha();
        repo.write("a.txt", &edited(5, &[2]));
        repo.write("c.txt", "new\n");
        repo.git(&["add", "c.txt"]);
        repo.git(&["mv", "b.txt", "d.txt"]);
        repo.write("untracked.txt", "u\n");
        (repo, base)
    }

    #[test]
    fn the_working_tree_head_counts_staged_and_unstaged_tracked_changes() {
        let (repo, base) = worktree_scenario();
        let found = check(&repo.0, &base, &Head::Worktree).unwrap();
        assert_eq!(
            found,
            "3 of 100 files; 2 added, 1 deleted, 1 renamed lines; 0 generated files not counted"
        );
    }

    #[test]
    fn two_runs_agree_and_leave_the_repository_alone() {
        let (repo, base) = worktree_scenario();
        let (status, head) = (repo.status(), repo.sha());
        let first = check(&repo.0, &base, &Head::Worktree);
        let second = check(&repo.0, &base, &Head::Worktree);
        assert_eq!(first, second);
        assert!(first.is_ok());
        assert_eq!((repo.status(), repo.sha()), (status, head));

        let (repo, base, head) = history(
            "determinism",
            Some(LIMIT),
            |r| r.write("gen/x.rs", "fn a() {}\n"),
            |r| r.write("gen/y.rs", "fn b() {}\n"),
        );
        let (status, tip) = (repo.status(), repo.sha());
        let first = check(&repo.0, &base, &head);
        let second = check(&repo.0, &base, &head);
        assert_eq!(first, second);
        let message = first.unwrap_err();
        assert!(message.contains("gen/x.rs, gen/y.rs"), "{message}");
        assert_eq!((repo.status(), repo.sha()), (status, tip));
    }

    #[test]
    fn a_shallow_history_cannot_show_that_the_limits_file_never_existed() {
        let origin = Repo::new("origin");
        origin.init();
        origin.write("a.txt", "a\n");
        origin.commit("one");
        origin.commit("two");
        let clone = Repo::new("shallow");
        let url = format!("file://{}", origin.0.display());
        clone.git(&["clone", "--quiet", "--depth", "1", &url, "."]);
        let message = limits_at(&clone.0, &clone.sha(), &Head::Worktree)
            .err()
            .unwrap();
        assert!(message.contains("shallow history"), "{message}");
    }
}
