use crate::gate;
use crate::git;
use crate::scope::{self, Head};
use crate::source::{self, failure};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};
use syn::Item;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

pub(crate) const VERSION: &str = "5.4.0";
const DEFAULT_MIN_TOKENS: u32 = 70;
const FIX: &str = "reuse the existing code or extract one implementation; do not add a generic abstraction only to dodge this check";

#[derive(Deserialize, Debug)]
struct Report {
    duplicates: Vec<Duplicate>,
    statistics: Option<Statistics>,
}

#[derive(Deserialize, Debug)]
struct Statistics {
    total: Total,
}

#[derive(Deserialize, Debug)]
struct Total {
    clones: usize,
}

#[derive(Deserialize, Debug)]
struct Duplicate {
    #[serde(default)]
    kind: String,
    #[serde(default, rename = "isNew")]
    is_new: bool,
    tokens: u32,
    #[serde(rename = "firstFile")]
    first: Fragment,
    #[serde(rename = "secondFile")]
    second: Fragment,
}

#[derive(Deserialize, Debug)]
struct Fragment {
    name: String,
    start: usize,
    end: usize,
}

#[derive(Default, Debug)]
struct Judged {
    new: Vec<Duplicate>,
    total: usize,
}

type Added = BTreeMap<String, Vec<(usize, usize)>>;

fn install_command() -> String {
    format!("npm install --global --ignore-scripts jscpd@{VERSION}")
}

fn first_line(text: &str) -> &str {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
}

fn version_matches(output: &str) -> Result<(), String> {
    let mut words = output.split_whitespace();
    if let (Some("jscpd"), Some(VERSION), None) | (Some(VERSION), None, None) =
        (words.next(), words.next(), words.next())
    {
        return Ok(());
    }
    let found = match first_line(output) {
        "" => "no answer",
        line => line,
    };
    Err(format!(
        "jscpd {VERSION} is required, found {found}; run `{}`",
        install_command()
    ))
}

fn without_git_environment(command: &mut Command) -> &mut Command {
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("GIT_") {
            command.env_remove(name);
        }
    }
    command
}

fn run_tool(dir: &Path, args: &[String]) -> Result<Output, String> {
    let mut command = Command::new("jscpd");
    command.args(args).current_dir(dir);
    without_git_environment(&mut command).output().map_err(|e| {
        format!(
            "could not run jscpd: {e}; run `{}` and put it on PATH",
            install_command()
        )
    })
}

fn require_version(dir: &Path) -> Result<(), String> {
    let out = run_tool(dir, &["--version".to_owned()])?;
    let text = if out.status.success() {
        String::from_utf8_lossy(&out.stdout).into_owned()
    } else {
        String::from_utf8_lossy(&out.stderr).into_owned()
    };
    version_matches(&text)
}

fn judge(
    status: Option<i32>,
    report: Result<Report, String>,
    stderr: &str,
) -> Result<Judged, String> {
    let Some(status) = status else {
        return Err("jscpd was stopped by a signal; run it again".to_owned());
    };
    let said = stderr
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("Error") || line.starts_with("ERROR"))
        .unwrap_or_else(|| first_line(stderr));
    if !matches!(status, 0 | 1) {
        return Err(format!("jscpd exited with status {status}: {said}"));
    }
    let report = report
        .map_err(|e| format!("jscpd left no usable report (status {status}): {e}; {said}"))?;
    let total = report
        .statistics
        .map_or(report.duplicates.len(), |statistics| {
            statistics.total.clones
        });
    let new: Vec<Duplicate> = report
        .duplicates
        .into_iter()
        .filter(|duplicate| duplicate.is_new)
        .collect();
    match (status, new.is_empty()) {
        (0, true) | (1, false) => Ok(Judged { new, total }),
        (0, false) => Err(format!(
            "jscpd exited with status 0 but its report holds {} new clones",
            new.len()
        )),
        _ => Err("jscpd exited with status 1 but its report holds no new clone".to_owned()),
    }
}

fn added_lines(diff: &str) -> Result<Added, String> {
    let mut added = Added::new();
    let mut current: Option<String> = None;
    let mut in_hunks = false;
    for line in diff.lines() {
        if line.starts_with("diff --git ") {
            current = None;
            in_hunks = false;
        } else if let (false, Some(name)) = (in_hunks, line.strip_prefix("+++ ")) {
            current = match (name.strip_prefix("b/"), name) {
                (Some(path), _) => Some(path.trim_end_matches('\t').to_owned()),
                (None, "/dev/null") => None,
                _ => {
                    return Err(format!(
                        "cannot read the path {name} in the diff; rename the file"
                    ));
                }
            };
        } else if let Some(header) = line.strip_prefix("@@ ") {
            in_hunks = true;
            let Some(path) = &current else { continue };
            let token = header
                .split(' ')
                .find_map(|word| word.strip_prefix('+'))
                .ok_or_else(|| format!("cannot read the hunk header {line:?}"))?;
            let (start, count) = token.split_once(',').unwrap_or((token, "1"));
            let number = |text: &str| {
                text.parse::<usize>()
                    .map_err(|_| format!("cannot read the hunk header {line:?}"))
            };
            let (start, count) = (number(start)?, number(count)?);
            if count > 0 {
                added
                    .entry(path.clone())
                    .or_default()
                    .push((start, start + count - 1));
            }
        }
    }
    Ok(added)
}

fn overlaps(ranges: &[(usize, usize)], start: usize, end: usize) -> bool {
    ranges
        .iter()
        .any(|&(first, last)| start <= last && first <= end)
}

struct Declarations(Vec<(usize, usize)>);

impl<'ast> Visit<'ast> for Declarations {
    fn visit_item(&mut self, item: &'ast Item) {
        let declaration = match item {
            Item::Struct(_) | Item::Enum(_) | Item::Use(_) => true,
            Item::Macro(invocation) => invocation.ident.is_none(),
            _ => false,
        };
        if declaration {
            let span = item.span();
            self.0.push((span.start().line, span.end().line));
        } else {
            visit::visit_item(self, item);
        }
    }
}

fn declaration_only(path: &str, text: &str, start: usize, end: usize) -> Result<bool, String> {
    let file = source::parse(path, text)?;
    let mut declarations = Declarations(Vec::new());
    declarations.visit_file(&file);
    let lines: Vec<&str> = text.lines().collect();
    Ok((start..=end).all(|number| {
        let line = number
            .checked_sub(1)
            .and_then(|index| lines.get(index))
            .map_or("", |line| line.trim());
        line.is_empty()
            || line.starts_with("//")
            || line.starts_with("#[")
            || line.starts_with("#![")
            || declarations
                .0
                .iter()
                .any(|&(first, last)| first <= number && number <= last)
    }))
}

fn listed_paths(repo: &Path, head: &Head, exclude: &[String]) -> Result<Vec<String>, String> {
    let specs = std::iter::once("*.rs".to_owned())
        .chain(exclude.iter().map(|glob| format!(":(exclude,glob){glob}")));
    let mut args: Vec<String> = match head {
        Head::Worktree => vec!["ls-files".to_owned(), "-z".to_owned()],
        Head::Commit(sha) => {
            let empty = git::pipe(repo, &["hash-object", "-t", "tree", "--stdin"], b"")
                .map_err(|e| e.to_string())?;
            vec![
                "diff".to_owned(),
                "--name-only".to_owned(),
                "-z".to_owned(),
                "--no-renames".to_owned(),
                String::from_utf8_lossy(&empty).trim().to_owned(),
                sha.clone(),
            ]
        }
    };
    args.push("--".to_owned());
    args.extend(specs);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let listing = git::run(repo, &args).map_err(|e| e.to_string())?;
    let mut paths = scope::paths_of(&listing)?;
    if matches!(head, Head::Worktree) {
        paths.retain(|path| repo.join(path).symlink_metadata().is_ok());
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Result<Scratch, String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let dir = std::env::temp_dir().join(format!("xtask-clones-{}-{stamp}", std::process::id()));
        std::fs::create_dir(&dir)
            .map_err(|e| format!("could not create {}: {e}", dir.display()))?;
        let canonical = std::fs::canonicalize(&dir)
            .map_err(|e| format!("could not resolve {}: {e}", dir.display()))?;
        Ok(Scratch(canonical))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn export(repo: &Path, sha: &str, paths: &[String], tree: &Path) -> Result<(), String> {
    let specs: Vec<String> = paths.iter().map(|path| format!("{sha}:{path}")).collect();
    for (path, blob) in paths.iter().zip(scope::blobs(repo, &specs)?) {
        let bytes = blob.ok_or_else(|| format!("{path} is not in {sha}"))?;
        if !Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
        {
            return Err(format!("the path {path} in {sha} leaves the tree"));
        }
        let target = tree.join(path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("could not create {}: {e}", parent.display()))?;
        }
        std::fs::write(&target, bytes)
            .map_err(|e| format!("could not write {}: {e}", target.display()))?;
    }
    Ok(())
}

fn borrow_objects(repo: &Path, tree: &Path) -> Result<(), String> {
    let mut init = Command::new("git");
    init.args(["init", "--quiet"]).current_dir(tree);
    let created = without_git_environment(&mut init)
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;
    if !created.status.success() {
        return Err(format!(
            "git init failed in {}: {}",
            tree.display(),
            String::from_utf8_lossy(&created.stderr).trim()
        ));
    }
    let objects = git::text(
        repo,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "objects",
        ],
    )
    .map_err(|e| e.to_string())?;
    let alternates = tree.join(".git/objects/info/alternates");
    std::fs::write(&alternates, format!("{}\n", objects.trim()))
        .map_err(|e| format!("could not write {}: {e}", alternates.display()))
}

fn read_report(out: &Path) -> Result<Report, String> {
    let mut reports: Vec<PathBuf> = std::fs::read_dir(out)
        .map_err(|e| format!("could not read {}: {e}", out.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    let (Some(report), None) = (reports.pop(), reports.pop()) else {
        return Err("jscpd did not write exactly one report".to_owned());
    };
    let bytes =
        std::fs::read(&report).map_err(|e| format!("could not read {}: {e}", report.display()))?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

fn relative(root: &Path, name: &str) -> Result<String, String> {
    let path = Path::new(name);
    match path.strip_prefix(root) {
        Ok(inside) => Ok(inside.to_string_lossy().into_owned()),
        Err(_) if path.is_relative() => Ok(name.to_owned()),
        Err(_) => Err(format!(
            "jscpd named {name}, which is outside {}",
            root.display()
        )),
    }
}

fn scan(
    repo: &Path,
    base: &str,
    head: &Head,
    paths: &[String],
    min_tokens: u32,
) -> Result<Judged, String> {
    let scratch = Scratch::new()?;
    let out = scratch.0.join("out");
    let config = scratch.0.join("config.json");
    std::fs::create_dir(&out).map_err(|e| format!("could not create {}: {e}", out.display()))?;
    std::fs::write(&config, "{}")
        .map_err(|e| format!("could not write {}: {e}", config.display()))?;
    let root = match head {
        Head::Worktree => std::fs::canonicalize(repo)
            .map_err(|e| format!("could not resolve {}: {e}", repo.display()))?,
        Head::Commit(sha) => {
            let tree = scratch.0.join("tree");
            export(repo, sha, paths, &tree)?;
            borrow_objects(repo, &tree)?;
            tree
        }
    };
    let mut args: Vec<String> = [
        "--format",
        "rust",
        "--ignore-identifiers",
        "--min-tokens",
        &min_tokens.to_string(),
        "--reporters",
        "json",
        "--output",
        &out.to_string_lossy(),
        "--absolute",
        "--no-colors",
        "--no-gitignore",
        "--config",
        &config.to_string_lossy(),
        "--max-size",
        "100mb",
        "--baseline-from-ref",
        base,
        "--fail-on-new-clones",
        "0",
    ]
    .map(str::to_owned)
    .to_vec();
    args.extend(paths.iter().map(|path| {
        if path.starts_with('-') {
            format!("./{path}")
        } else {
            path.clone()
        }
    }));
    let ran = run_tool(&root, &args)?;
    let stderr = String::from_utf8_lossy(&ran.stderr);
    let mut judged = judge(ran.status.code(), read_report(&out), &stderr)?;
    for duplicate in &mut judged.new {
        for side in [&mut duplicate.first, &mut duplicate.second] {
            side.name = relative(&root, &side.name)?;
        }
    }
    Ok(judged)
}

fn diff_text(repo: &Path, base: &str, head: &Head) -> Result<String, String> {
    let mut args = vec![
        "diff",
        "-U0",
        "-M",
        "--no-color",
        "--no-ext-diff",
        "--src-prefix=a/",
        "--dst-prefix=b/",
        base,
    ];
    if let Head::Commit(sha) = head {
        args.push(sha);
    }
    args.push("--");
    args.push("*.rs");
    git::text(repo, &args).map_err(|e| e.to_string())
}

fn on_added_code(
    repo: &Path,
    base: &str,
    head: &Head,
    new: Vec<Duplicate>,
) -> Result<Vec<Duplicate>, String> {
    if new.is_empty() {
        return Ok(new);
    }
    let added = added_lines(&diff_text(repo, base, head)?)?;
    let mut sources = source::Files::new();
    let mut kept = Vec::new();
    for duplicate in new {
        let mut keep = false;
        for side in [&duplicate.first, &duplicate.second] {
            let Some(ranges) = added.get(&side.name) else {
                continue;
            };
            if !overlaps(ranges, side.start, side.end) {
                continue;
            }
            if !sources.contains_key(&side.name) {
                sources.extend(source::read_files(
                    repo,
                    head,
                    std::slice::from_ref(&side.name),
                )?);
            }
            let text = sources
                .get(&side.name)
                .ok_or_else(|| format!("{} is not at the head", side.name))?;
            if !declaration_only(&side.name, text, side.start, side.end)? {
                keep = true;
                break;
            }
        }
        if keep {
            kept.push(duplicate);
        }
    }
    Ok(kept)
}

fn describe(duplicate: &Duplicate) -> String {
    let mut sides = [&duplicate.first, &duplicate.second];
    sides.sort_by(|a, b| (&a.name, a.start).cmp(&(&b.name, b.start)));
    let [first, second] = sides;
    format!(
        "{}:{}-{} <-> {}:{}-{} ({} tokens, {})",
        first.name,
        first.start,
        first.end,
        second.name,
        second.start,
        second.end,
        duplicate.tokens,
        duplicate.kind
    )
}

pub(crate) fn check(repo: &Path, base: &str, head: &Head) -> Result<String, String> {
    require_version(repo)?;
    let configured = gate::lints_at(repo, base, head)?.and_then(|lints| lints.clones);
    let (min_tokens, exclude) = configured
        .as_ref()
        .map_or((DEFAULT_MIN_TOKENS, &[][..]), |clones| {
            (clones.min_tokens, clones.exclude.as_slice())
        });
    let paths = listed_paths(repo, head, exclude)?;
    let judged = if paths.is_empty() {
        Judged::default()
    } else {
        scan(repo, base, head, &paths, min_tokens)?
    };
    let kept = on_added_code(repo, base, head, judged.new)?;
    if configured.is_none() {
        return Ok(format!(
            "not in force yet at {}; {} new clones",
            scope::short(base),
            kept.len()
        ));
    }
    if kept.is_empty() {
        return Ok(format!("no new clone; {} clones at the head", judged.total));
    }
    let mut lines: Vec<String> = kept.iter().map(describe).collect();
    lines.sort();
    Err(format!(
        "{} new {}: {}",
        kept.len(),
        if kept.len() == 1 { "clone" } else { "clones" },
        failure(&lines, FIX)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scope::tests::{Repo, judge_rows_in_parallel};
    use serde_json::json;

    #[test]
    fn the_version_check_accepts_only_the_pinned_jscpd() {
        assert_eq!(version_matches("jscpd 5.4.0\n"), Ok(()));
        for output in ["jscpd 5.4.1\n", "jscpd 5.4.00", "", "\n", "5.4.0 jscpd"] {
            let error = version_matches(output).expect_err(output);
            assert!(
                error.contains("npm install --global --ignore-scripts jscpd@5.4.0"),
                "{output:?}: {error}"
            );
        }
    }

    fn duplicate(is_new: bool) -> serde_json::Value {
        json!({
            "format": "rust", "kind": "renamed", "isNew": is_new, "lines": 19, "tokens": 126,
            "fragment": "fn a() {}",
            "firstFile": {"name": "/w/a.rs", "start": 1, "end": 19,
                "startLoc": {"line": 1, "column": 0, "position": 0}},
            "secondFile": {"name": "/w/b.rs", "start": 3, "end": 21},
        })
    }

    fn report(flags: &[bool]) -> Result<Report, String> {
        let text = json!({
            "duplicates": flags.iter().map(|new| duplicate(*new)).collect::<Vec<_>>(),
            "statistics": {"total": {"clones": flags.len() + 2, "newClones": 1}},
            "detectionDate": "2026-10-14",
        })
        .to_string();
        serde_json::from_str(&text).map_err(|e| e.to_string())
    }

    #[test]
    fn a_jscpd_run_is_trusted_only_as_a_parsed_report() {
        let unparsable =
            || serde_json::from_str::<Report>("{\"duplicates\": 3}").map_err(|e| e.to_string());
        type Expected = Result<(usize, usize), &'static str>;
        let cases: Vec<(&str, Option<i32>, Result<Report, String>, &str, Expected)> = vec![
            ("clean", Some(0), report(&[false]), "", Ok((0, 3))),
            ("new", Some(1), report(&[true, false]), "", Ok((1, 4))),
            (
                "exit 1, none new",
                Some(1),
                report(&[false]),
                "",
                Err("no new clone"),
            ),
            (
                "exit 0, one new",
                Some(0),
                report(&[true]),
                "",
                Err("status 0"),
            ),
            (
                "exit 2",
                Some(2),
                Err("no report".to_owned()),
                "Using config from /tmp/c.json\nError: git ref 'x' not found\nsecond line",
                Err("git ref 'x' not found"),
            ),
            ("signal", None, report(&[]), "", Err("signal")),
            (
                "no report",
                Some(0),
                Err("no report".to_owned()),
                "",
                Err("no usable report"),
            ),
            (
                "unparsable",
                Some(1),
                unparsable(),
                "",
                Err("no usable report"),
            ),
        ];
        for (name, status, report, stderr, expected) in cases {
            let found = judge(status, report, stderr).map(|j| (j.new.len(), j.total));
            match (found, expected) {
                (Ok(found), Ok(expected)) => assert_eq!(found, expected, "{name}"),
                (Err(error), Err(needle)) => {
                    assert!(error.contains(needle), "{name}: {error}");
                    assert!(!error.contains("second line"), "{name}: {error}");
                    assert!(!error.contains("Using config"), "{name}: {error}");
                }
                (found, expected) => panic!("{name}: got {found:?}, expected {expected:?}"),
            }
        }
    }

    #[test]
    fn added_lines_come_from_the_new_side_of_each_hunk_header() {
        let diff = concat!(
            "diff --git a/a.rs b/a.rs\n",
            "index 111..222 100644\n",
            "--- a/a.rs\n",
            "+++ b/a.rs\n",
            "@@ -3,0 +4,2 @@ fn x\n",
            "+one\n",
            "+two\n",
            "@@ -10 +12 @@\n",
            "-gone\n",
            "+new\n",
            "@@ -20,2 +21,0 @@\n",
            "-x\n",
            "-y\n",
            "diff --git a/gone.rs b/gone.rs\n",
            "deleted file mode 100644\n",
            "--- a/gone.rs\n",
            "+++ /dev/null\n",
            "@@ -1,3 +0,0 @@\n",
            "-a\n",
            "diff --git a/new.rs b/new.rs\n",
            "new file mode 100644\n",
            "--- /dev/null\n",
            "+++ b/new.rs\n",
            "@@ -0,0 +1,3 @@\n",
            "+++ looks like a header\n",
            "+b\n",
            "+c\n",
            "diff --git a/with space.rs b/with space.rs\n",
            "--- a/with space.rs\t\n",
            "+++ b/with space.rs\t\n",
            "@@ -0,0 +1 @@\n",
            "+x\n",
        );
        let added = added_lines(diff).unwrap();
        assert_eq!(
            added.keys().collect::<Vec<_>>(),
            ["a.rs", "new.rs", "with space.rs"]
        );
        assert_eq!(added["a.rs"], [(4, 5), (12, 12)]);
        assert_eq!(added["new.rs"], [(1, 3)]);
        assert_eq!(added["with space.rs"], [(1, 1)]);

        let ranges = &added["a.rs"];
        let cases = [
            (1, 3, false),
            (1, 4, true),
            (5, 9, true),
            (6, 11, false),
            (9, 12, true),
            (13, 40, false),
        ];
        for (start, end, expected) in cases {
            assert_eq!(overlaps(ranges, start, end), expected, "{start}-{end}");
        }
        assert!(added_lines("+++ \"b/quoted\\tname.rs\"\n@@ -0,0 +1 @@\n+x\n").is_err());
    }

    const DECLARATIONS: &str = "use std::fmt;
use std::io::{
    Read,
    Write,
};

pub struct A {
    pub a: u32,
    pub b: u32,
}

pub struct B {
    pub c: u32,
}

keys! {
    one,
    two,
}

pub fn f(x: u32) -> u32 {
    let y = x + 1;
    y * 2
}

pub const TABLE: [u32; 3] = [
    1,
    2,
    3,
];

pub fn g(x: u32) -> u32 {
    match x {
        0 => 1,
        _ => 2,
    }
}
";

    #[test]
    fn the_declaration_filter_keeps_code_and_drops_field_lists() {
        let cases = [
            ("inside one struct's fields", 8, 9, true),
            ("two structs and a use list", 2, 14, true),
            ("inside an item-level macro call", 17, 18, true),
            ("inside a function body", 22, 23, false),
            ("inside a const array", 27, 29, false),
            ("covering a match", 33, 36, false),
            ("a struct and the function after it", 12, 23, false),
        ];
        for (name, start, end, expected) in cases {
            assert_eq!(
                declaration_only("x.rs", DECLARATIONS, start, end),
                Ok(expected),
                "{name}"
            );
        }
        let error = declaration_only("x.rs", "fn broken( {", 1, 1).unwrap_err();
        assert!(error.contains("does not parse"), "{error}");
    }

    #[test]
    fn tracked_rust_files_exclude_the_base_globs_for_the_worktree_and_for_a_commit() {
        let repo = Repo::new("clones-listing");
        repo.init();
        for path in [
            "a.rs",
            "crates/x/src/lib.rs",
            "crates/x/src/keys/k.rs",
            "crates/y/src/keys/deep/k.rs",
            "vendor/v.rs",
            "notes.md",
        ] {
            repo.write(path, "fn f() {}\n");
        }
        repo.commit("base");
        let sha = repo.sha();
        repo.write("untracked.rs", "fn f() {}\n");
        repo.remove("a.rs");
        let exclude = vec!["crates/*/src/keys/**".to_owned()];

        let expected = ["a.rs", "crates/x/src/lib.rs", "vendor/v.rs"];
        let at_commit = listed_paths(&repo.0, &Head::Commit(sha), &exclude).unwrap();
        assert_eq!(at_commit, expected);
        let in_tree = listed_paths(&repo.0, &Head::Worktree, &exclude).unwrap();
        assert_eq!(in_tree, ["crates/x/src/lib.rs", "vendor/v.rs"]);
        let unfiltered = listed_paths(&repo.0, &Head::Worktree, &[]).unwrap();
        assert!(unfiltered.contains(&"crates/x/src/keys/k.rs".to_owned()));
    }

    mod jscpd {
        use super::*;
        use crate::parity::Selection;
        use crate::source::fixture::{self, At, Edit, Row, Verdict, del, put, row};

        fn routine(name: &str, input: &str, acc: &str) -> String {
            let mut text =
                format!("pub fn {name}({input}: &[u32]) -> u32 {{\n    let mut {acc} = 0u32;\n");
            for step in 0..14 {
                text += &format!(
                    "    if {input}.len() > {step} && {input}[{step}] % 3 == {} {{\n        {acc} = {acc}.wrapping_mul(31).wrapping_add({input}[{step}] ^ {});\n    }}\n",
                    step % 3,
                    step * 7 + 1
                );
            }
            text + &format!("    {acc}\n}}\n")
        }

        fn original() -> String {
            routine("original", "input", "acc")
        }

        fn renamed() -> String {
            routine("copied", "values", "total")
        }

        fn record(name: &str, field: &str) -> String {
            let fields: String = (0..16)
                .map(|index| format!("    pub {field}{index}: u32,\n"))
                .collect();
            format!("pub struct {name} {{\n{fields}}}\n")
        }

        const LINTS: &str = "[clones]\nmin_tokens = 70\n";

        fn lints() -> Edit {
            put("quality/lints.toml", LINTS)
        }

        fn clones(
            repo: &Path,
            base: &str,
            head: &Head,
            _selection: Selection,
        ) -> Result<String, String> {
            check(repo, base, head)
        }

        fn judge(row: &Row) -> Option<String> {
            fixture::judge("clones", &BTreeMap::new(), clones, row)
        }

        #[test]
        fn a_renamed_copy_fails_naming_both_copies() {
            let repo = Repo::new("clones-renamed");
            repo.init();
            repo.write("quality/lints.toml", LINTS);
            repo.write("a.rs", &original());
            repo.commit("base");
            let base = repo.sha();
            let unchanged = check(&repo.0, &base, &Head::Worktree).unwrap();
            assert!(unchanged.starts_with("no new clone; "), "{unchanged}");

            repo.write("b.rs", &renamed());
            repo.commit("head");
            let error = check(&repo.0, &base, &Head::Commit(repo.sha())).unwrap_err();
            assert!(
                error.contains("a.rs:1-46 <-> b.rs:1-46 (") && error.contains(" tokens, renamed)"),
                "{error}"
            );
            assert!(error.contains("1 new clone"), "{error}");
        }

        #[test]
        fn the_check_judges_clones_against_the_base() {
            const COPY: Verdict = Verdict::Fail(&["a.rs:", "e.rs:", " <-> "]);
            const CLEAN: Verdict = Verdict::Pass(&["no new clone"]);
            let third = || routine("third", "items", "sum");
            let with_a = || vec![lints(), put("a.rs", &original())];
            let three = || {
                vec![
                    lints(),
                    put("a.rs", &original()),
                    put("b.rs", &renamed()),
                    put("c.rs", &third()),
                ]
            };
            let copy = |name: &'static str, extra: Edit| {
                row(name, with_a(), vec![extra, put("e.rs", &renamed())], COPY).forced()
            };
            let excluding = "[clones]\nmin_tokens = 70\nexclude = [\"crates/skip/**\"]\n";
            let rows = vec![
                row(
                    "editing one copy of an existing clone",
                    three(),
                    vec![put("b.rs", &(renamed() + "pub const EDITED: u32 = 1;\n"))],
                    CLEAN,
                ),
                row(
                    "a new 16-field struct",
                    vec![lints(), put("t.rs", &record("Old", "old"))],
                    vec![put("s.rs", &record("New", "fresh"))],
                    CLEAN,
                ),
                row(
                    "a moved file",
                    three(),
                    vec![del("b.rs"), put("d.rs", &renamed())],
                    CLEAN,
                ),
                row(
                    "30 lines inserted above existing clones",
                    three(),
                    vec![put("b.rs", &("// pad\n".repeat(30) + &renamed()))],
                    CLEAN,
                ),
                row(
                    "the renamed copy",
                    with_a(),
                    vec![put("e.rs", &renamed())],
                    COPY,
                ),
                copy(
                    "a head .gitignore listing the copy",
                    put(".gitignore", "e.rs\n"),
                ),
                copy(
                    "a head .jscpd.json ignoring the copy",
                    put(".jscpd.json", "{\"ignore\": [\"**/e.rs\"]}"),
                ),
                copy(
                    "a head package.json with a jscpd key ignoring the copy",
                    put("package.json", "{\"jscpd\": {\"ignore\": [\"**/e.rs\"]}}"),
                ),
                row(
                    "the copy in a file over 1 MB",
                    with_a(),
                    vec![put("e.rs", &(renamed() + &"// pad\n".repeat(160_000)))],
                    COPY,
                ),
                row(
                    "a copy under a path the base excludes",
                    vec![
                        put("quality/lints.toml", excluding),
                        put("a.rs", &original()),
                    ],
                    vec![put("crates/skip/e.rs", &renamed())],
                    CLEAN,
                ),
                row(
                    "min_tokens raised at the head",
                    with_a(),
                    vec![
                        put("quality/lints.toml", "[clones]\nmin_tokens = 5000\n"),
                        put("e.rs", &renamed()),
                    ],
                    COPY,
                ),
                row(
                    "a base without [clones]",
                    vec![put("a.rs", &original())],
                    vec![put("e.rs", &renamed())],
                    Verdict::Pass(&["not in force yet at", "; 1 new clones"]),
                ),
                row(
                    "an untracked copy in the working tree",
                    with_a(),
                    vec![put("e.rs", &renamed())],
                    CLEAN,
                )
                .at(At::Worktree),
                row(
                    "a commit head exported while HEAD stays at the base",
                    with_a(),
                    vec![put("e.rs", &renamed())],
                    COPY,
                )
                .at(At::DetachedAtBase),
            ];
            judge_rows_in_parallel(&rows, judge);
        }
    }
}
