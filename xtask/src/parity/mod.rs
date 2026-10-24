mod claims;
mod route;
mod rules;

use crate::scope::{self, Head};
use crate::source;
use route::Routes;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use syn::{Item, Visibility};

pub const TABLE: &str = "tools/fixtures.json";
pub const LOADER_SOURCE: &str = "tools/mcrs_minecraft_fixtures/src/lib.rs";

pub enum Selection<'a> {
    Every,
    Changed(&'a [String]),
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Warning {
    path: String,
    line: usize,
    test: String,
}

impl Warning {
    fn message(&self) -> String {
        format!(
            "{}: claims vanilla behaviour against a literal and loads no fixture",
            self.test
        )
    }
}

struct Report {
    files: usize,
    violations: BTreeSet<String>,
    warnings: BTreeSet<Warning>,
}

fn crate_of(path: &str) -> &str {
    match path.split_once('/') {
        Some(("crates", rest)) => rest.split_once('/').map_or("mcrs", |(name, _)| name),
        Some(("xtask", _)) => "xtask",
        _ => "mcrs",
    }
}

fn counts(warnings: &BTreeSet<Warning>) -> BTreeMap<&str, usize> {
    let mut counts = BTreeMap::new();
    for warning in warnings {
        *counts.entry(crate_of(&warning.path)).or_default() += 1;
    }
    counts
}

fn annotations(warnings: &BTreeSet<Warning>, changed: &[String]) -> Vec<String> {
    warnings
        .iter()
        .filter(|warning| changed.contains(&warning.path))
        .map(|warning| {
            format!(
                "::warning file={},line={}::{}",
                crate::escape_property(&warning.path),
                warning.line,
                crate::escape_data(&warning.message())
            )
        })
        .collect()
}

fn report_lines(report: &Report, annotate: Option<&[String]>) -> Vec<String> {
    let mut lines: Vec<String> = report
        .violations
        .iter()
        .map(|line| printable(line))
        .collect();
    for warning in &report.warnings {
        lines.push(printable(&format!(
            "parity: warning: {}:{}: {}",
            warning.path,
            warning.line,
            warning.message()
        )));
    }
    for (krate, count) in counts(&report.warnings) {
        lines.push(printable(&format!(
            "parity: {krate}: {count} vanilla claims"
        )));
    }
    if let Some(changed) = annotate {
        lines.extend(annotations(&report.warnings, changed));
    }
    lines
}

fn widens(path: &str) -> bool {
    path == TABLE || path == LOADER_SOURCE || path.starts_with("xtask/")
}

fn selected(repo: &Path, head: &Head, selection: &Selection) -> Result<Vec<String>, String> {
    match selection {
        Selection::Changed(paths) if !paths.iter().any(|path| widens(path)) => {
            let mut rust: Vec<String> = paths
                .iter()
                .filter(|path| path.ends_with(".rs"))
                .cloned()
                .collect();
            rust.sort();
            rust.dedup();
            Ok(rust)
        }
        _ => source::rust_files(repo, head),
    }
}

fn read_head(repo: &Path, head: &Head, path: &str) -> Result<Option<String>, String> {
    let Head::Worktree = head else {
        return scope::read_at(repo, head.rev(), path);
    };
    match std::fs::read(repo.join(path)) {
        Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("could not read {path}: {e}")),
    }
}

fn routes(repo: &Path, base: &str, head: &Head) -> Result<Routes, String> {
    let at_base = scope::read_at(repo, base, TABLE)?;
    let at_head = read_head(repo, head, TABLE)?;
    Routes::parse(&[
        ("the base", at_base.as_deref()),
        ("the head", at_head.as_deref()),
    ])
}

fn loader_functions(repo: &Path, head: &Head) -> Result<BTreeSet<String>, String> {
    let Some(text) = read_head(repo, head, LOADER_SOURCE)? else {
        return Ok(BTreeSet::new());
    };
    let file = source::parse(LOADER_SOURCE, &text)?;
    Ok(file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(function) if matches!(function.vis, Visibility::Public(_)) => {
                Some(function.sig.ident.to_string())
            }
            _ => None,
        })
        .collect())
}

fn scan(repo: &Path, base: &str, head: &Head, selection: &Selection) -> Result<Report, String> {
    let routes = routes(repo, base, head)?;
    let loader_functions = loader_functions(repo, head)?;
    let paths = selected(repo, head, selection)?;
    let files = source::read_files(repo, head, &paths)?;
    let mut violations = BTreeSet::new();
    let mut warnings = BTreeSet::new();
    for (path, text) in &files {
        let parsed = match source::parse(path, text) {
            Ok(parsed) => parsed,
            Err(problem) => {
                violations.insert(problem);
                continue;
            }
        };
        let literals = source::string_literals(&parsed);
        for line in route::violations(&routes, path, &literals) {
            violations.insert(format!("{path}:{line}: {}", route::OUTSIDE_THE_LOADER));
        }
        if rules::is_parity_file(path) {
            for (line, rule) in rules::check(&parsed, &loader_functions) {
                violations.insert(format!("{path}:{line}: {rule}"));
            }
        }
        for claim in claims::warnings(path, &parsed, &loader_functions) {
            warnings.insert(Warning {
                path: path.clone(),
                line: claim.line,
                test: claim.test,
            });
        }
    }
    Ok(Report {
        files: files.len(),
        violations,
        warnings,
    })
}

fn printable(line: &str) -> String {
    let line = crate::one_line(line);
    if line.starts_with("::") || line.starts_with("##[") {
        format!("./{line}")
    } else {
        line
    }
}

pub fn check(
    repo: &Path,
    base: &str,
    head: &Head,
    selection: Selection,
    annotate: Option<&[String]>,
) -> Result<String, String> {
    let report = scan(repo, base, head, &selection)?;
    for line in report_lines(&report, annotate) {
        println!("{line}");
    }
    if report.violations.is_empty() {
        return Ok(format!(
            "{} files; {} vanilla claims outside parity files",
            report.files,
            report.warnings.len()
        ));
    }
    Err(format!("{} violations", report.violations.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scope::tests::Repo;

    const DEMO_TABLE: &str = r#"[
        { "name": "demo", "task": "dumpDemo", "outputs": [
            { "file": "demo.bin", "to": "crates/demo/tests/fixtures" } ] },
        { "name": "demo_dumps", "task": "dumpDemos", "outputs": [
            { "pattern": "demo_s*.bin", "to": "crates/demo/tests/fixtures/dumps" } ] }
    ]"#;

    const OUTSIDE: &str =
        "fixture path outside the loader: load the fixture through mcrs_minecraft_fixtures";

    fn with_base(base_files: &[(&str, &str)]) -> (Repo, String) {
        let repo = Repo::new("parity");
        repo.init();
        repo.write("README.md", "base\n");
        for (path, text) in base_files {
            repo.write(path, text);
        }
        repo.commit("base");
        let base = repo.sha();
        (repo, base)
    }

    fn scanned(repo: &Repo, base: &str) -> Result<Report, String> {
        scan(&repo.0, base, &Head::Worktree, &Selection::Every)
    }

    fn files_named(report: &Report) -> BTreeSet<String> {
        report
            .violations
            .iter()
            .map(|line| line.split(':').next().unwrap().to_owned())
            .collect()
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "a table of routes to a fixture, each with whether parity refuses it"
    )]
    fn parity_refuses_every_route_to_a_fixture_but_the_loader() {
        let rows: [(&str, &str, bool); 19] = [
            (
                "crates/demo/tests/it/embed.rs",
                "const A: &[u8] = include_bytes!(\"../fixtures/demo.bin\");\n",
                true,
            ),
            (
                "crates/demo/tests/it/manifest_dir.rs",
                "const P: &str = concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/tests/fixtures/demo.bin\");\n",
                true,
            ),
            (
                "crates/demo/tests/it/join.rs",
                "fn d() -> std::path::PathBuf {\n    std::path::Path::new(\"x\").join(\"tests/fixtures/dumps\")\n}\n",
                true,
            ),
            (
                "crates/demo/tests/it/member.rs",
                "const P: &str = \"tests/fixtures/dumps/demo_s1.bin\";\n",
                true,
            ),
            (
                "crates/other/tests/it/ending.rs",
                "const P: &str = \"fixtures/demo.bin\";\n",
                true,
            ),
            (
                "crates/other/src/from_root.rs",
                "const P: &str = \"crates/demo/tests/fixtures/demo.bin\";\n",
                true,
            ),
            (
                "crates/demo/tests/it/dotted.rs",
                "const P: &str = \"./tests/../tests/fixtures/demo.bin\";\n",
                true,
            ),
            (
                "crates/demo/tests/it/raw.rs",
                "const P: &str = r#\"tests/fixtures/demo.bin\"#;\n",
                true,
            ),
            (
                "crates/demo/tests/it/hand_written.rs",
                "const N: &str = include_str!(\"../fixtures/notes.json\");\n",
                false,
            ),
            (
                "crates/demo/tests/it/outside_pattern.rs",
                "const P: &str = \"tests/fixtures/dumps/other.bin\";\n",
                false,
            ),
            (
                "crates/demo/tests/it/elsewhere.rs",
                "const P: &str = \"tests/elsewhere/demo_s1.bin\";\n",
                false,
            ),
            (
                "crates/demo/tests/it/directory_alone.rs",
                "const P: &str = \"tests/fixtures\";\n",
                true,
            ),
            (
                "crates/demo/tests/it/split_join.rs",
                "fn p() -> std::path::PathBuf {\n    std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(\"tests/fixtures\").join(\"demo.bin\")\n}\n",
                true,
            ),
            (
                "tools/mcrs_minecraft_fixtures/src/owner.rs",
                "const P: &str = \"tests/fixtures/demo.bin\";\n",
                false,
            ),
            (
                "tools/mcrs_minecraft_update/src/owner.rs",
                "const P: &str = \"crates/demo/tests/fixtures/demo.bin\";\n",
                false,
            ),
            (
                "crates/demo/tests/it/documented.rs",
                "/// reads tests/fixtures/demo.bin\n#[doc = \"tests/fixtures/demo.bin\"]\nfn documented() {}\n",
                false,
            ),
            (
                "crates/demo/tests/it/documented_in_a_macro.rs",
                "macro_rules! m { () => { #[doc = \"tests/fixtures/demo.bin\"] fn f() {} } }\n",
                false,
            ),
            (
                "crates/demo/tests/fixtures/dumps/empty.rs",
                "const EMPTY: &str = \"\";\nconst DOT: &str = \".\";\nconst ROOT: &str = \"/\";\n",
                false,
            ),
            (
                "crates/demo/tests/it/in_a_macro.rs",
                "fn read() {\n    let _ = std::fs::read(format!(\"{}\", \"tests/fixtures/demo.bin\"));\n}\n",
                true,
            ),
        ];
        let (repo, base) = with_base(&[]);
        repo.write(TABLE, DEMO_TABLE);
        for (path, text, _) in rows {
            repo.write(path, text);
        }

        let report = scanned(&repo, &base).unwrap();
        let expected: BTreeSet<String> = rows
            .iter()
            .filter(|(_, _, refused)| *refused)
            .map(|(path, _, _)| (*path).to_owned())
            .collect();
        assert_eq!(files_named(&report), expected);
        assert_eq!(report.files, rows.len());
        assert!(
            report
                .violations
                .contains(&format!("crates/demo/tests/it/embed.rs:1: {OUTSIDE}")),
            "{:?}",
            report.violations
        );
        assert!(
            report
                .violations
                .contains(&format!("crates/demo/tests/it/in_a_macro.rs:2: {OUTSIDE}")),
            "{:?}",
            report.violations
        );
    }

    #[test]
    fn an_output_only_the_base_lists_is_still_guarded_and_a_base_without_a_table_adds_nothing() {
        let old = r#"[{ "name": "old", "task": "t", "outputs": [
            { "file": "old.bin", "to": "crates/demo/tests/fixtures" } ] }]"#;
        let (repo, base) = with_base(&[(TABLE, old)]);
        repo.write(TABLE, DEMO_TABLE);
        repo.write(
            "crates/demo/src/old.rs",
            "const P: &str = \"tests/fixtures/old.bin\";\n",
        );
        repo.write(
            "crates/demo/src/new.rs",
            "const P: &str = \"tests/fixtures/demo.bin\";\n",
        );
        let report = scanned(&repo, &base).unwrap();
        assert_eq!(
            files_named(&report),
            BTreeSet::from([
                "crates/demo/src/new.rs".to_owned(),
                "crates/demo/src/old.rs".to_owned()
            ])
        );

        repo.remove(TABLE);
        let report = scanned(&repo, &base).unwrap();
        assert_eq!(
            files_named(&report),
            BTreeSet::from(["crates/demo/src/old.rs".to_owned()])
        );

        let (bare, base) = with_base(&[]);
        bare.write(
            "crates/demo/src/old.rs",
            "const P: &str = \"tests/fixtures/old.bin\";\n",
        );
        let report = scanned(&bare, &base).unwrap();
        assert!(report.violations.is_empty(), "{:?}", report.violations);
    }

    #[test]
    fn a_table_may_gain_fields_but_must_keep_the_ones_the_check_reads() {
        let richer = r#"[{ "name": "demo", "task": "t", "magic": "DEMO", "outputs": [
            { "file": "demo.bin", "to": "crates/demo/tests/fixtures", "hash": "00" } ] }]"#;
        let (repo, base) = with_base(&[]);
        repo.write(TABLE, richer);
        repo.write(
            "crates/demo/src/a.rs",
            "const P: &str = \"tests/fixtures/demo.bin\";\n",
        );
        let report = scanned(&repo, &base).unwrap();
        assert_eq!(report.violations.len(), 1, "{:?}", report.violations);

        let broken = r#"[{ "name": "demo", "task": "t", "outputs": [{ "file": "demo.bin" }] }]"#;
        repo.write(TABLE, broken);
        let at_head = scanned(&repo, &base).err().expect("a table without `to`");
        assert!(
            at_head.contains(TABLE) && at_head.contains("head") && at_head.contains("to"),
            "{at_head}"
        );

        let (repo, base) = with_base(&[(TABLE, broken)]);
        let at_base = scanned(&repo, &base).err().expect("a table without `to`");
        assert!(
            at_base.contains(TABLE) && at_base.contains("base"),
            "{at_base}"
        );

        let neither = r#"[{ "name": "demo", "task": "t", "outputs": [{ "to": "crates/demo" }] }]"#;
        let (repo, base) = with_base(&[]);
        repo.write(TABLE, neither);
        let message = scanned(&repo, &base)
            .err()
            .expect("an output without a name");
        assert!(
            message.contains("demo") && message.contains("file"),
            "{message}"
        );
    }

    #[test]
    fn a_changed_table_loader_or_runner_widens_a_local_run_to_every_file() {
        let (repo, _) = with_base(&[
            ("a.rs", "fn a() {}\n"),
            ("b.rs", "fn b() {}\n"),
            ("notes.md", "notes\n"),
            (TABLE, "[]"),
            (LOADER_SOURCE, "pub fn fixture() {}\n"),
            ("xtask/src/x.rs", "fn x() {}\n"),
            ("xtasks/y.rs", "fn y() {}\n"),
        ]);
        repo.write("untracked.rs", "fn untracked() {}\n");
        let every = [
            "a.rs",
            "b.rs",
            LOADER_SOURCE,
            "untracked.rs",
            "xtask/src/x.rs",
            "xtasks/y.rs",
        ];
        let pick = |changed: &[&str]| {
            let changed: Vec<String> = changed.iter().map(|p| (*p).to_owned()).collect();
            selected(&repo.0, &Head::Worktree, &Selection::Changed(&changed)).unwrap()
        };
        assert_eq!(pick(&["a.rs"]), ["a.rs"]);
        assert_eq!(pick(&["a.rs", "notes.md"]), ["a.rs"]);
        assert_eq!(pick(&["xtasks/y.rs"]), ["xtasks/y.rs"]);
        assert_eq!(pick(&["gone.rs"]), ["gone.rs"]);
        assert_eq!(pick(&[]), Vec::<String>::new());
        for widening in [TABLE, LOADER_SOURCE, "xtask/src/x.rs", "xtask/Cargo.toml"] {
            assert_eq!(pick(&[widening]), every, "{widening}");
        }
        assert_eq!(
            selected(&repo.0, &Head::Worktree, &Selection::Every).unwrap(),
            every
        );
    }

    #[test]
    fn a_file_that_does_not_parse_fails_naming_its_line() {
        let (repo, base) = with_base(&[]);
        repo.write(TABLE, DEMO_TABLE);
        repo.write("crates/demo/src/broken.rs", "fn fine() {}\nfn broken( {\n");
        repo.write(
            "crates/demo/src/route.rs",
            "const P: &str = \"tests/fixtures/demo.bin\";\n",
        );
        let report = scanned(&repo, &base).unwrap();
        assert!(
            report
                .violations
                .iter()
                .any(|line| line.starts_with("crates/demo/src/broken.rs:2: does not parse: ")),
            "{:?}",
            report.violations
        );
        assert!(
            report
                .violations
                .contains(&format!("crates/demo/src/route.rs:1: {OUTSIDE}")),
            "a file that does not parse hid the others: {:?}",
            report.violations
        );
        assert_eq!(check_message(&repo, &base), Err("2 violations".to_owned()));
    }

    fn check_message(repo: &Repo, base: &str) -> Result<String, String> {
        check(&repo.0, base, &Head::Worktree, Selection::Every, None)
    }

    #[test]
    fn a_table_that_cannot_be_read_fails_instead_of_counting_as_absent() {
        let (repo, base) = with_base(&[]);
        repo.write(
            "tools/fixtures.json/inside.txt",
            "a directory, not a file\n",
        );
        let message = scanned(&repo, &base).err().expect("an unreadable table");
        assert!(
            message.starts_with(&format!("could not read {TABLE}: ")),
            "{message}"
        );
    }

    #[test]
    fn a_path_cannot_start_a_workflow_command_on_a_violation_line() {
        let rows = [
            ("::warning::x.rs:1: rule", "./::warning::x.rs:1: rule"),
            ("##[error]x.rs:1: rule", "./##[error]x.rs:1: rule"),
            ("a.rs:1: ::rule", "a.rs:1: ::rule"),
            ("a.rs:1: rule\n::warning::x", "a.rs:1: rule\\n::warning::x"),
            (":x.rs:1: rule", ":x.rs:1: rule"),
            ("#x.rs:1: rule", "#x.rs:1: rule"),
        ];
        for (line, printed) in rows {
            assert_eq!(printable(line), printed, "{line}");
        }
    }

    #[test]
    fn a_clean_tree_reports_its_file_count() {
        let (repo, base) = with_base(&[]);
        repo.write("crates/demo/src/a.rs", "fn a() {}\n");
        repo.write("crates/demo/src/b.rs", "fn b() {}\n");
        assert_eq!(
            check_message(&repo, &base),
            Ok("2 files; 0 vanilla claims outside parity files".to_owned())
        );
    }

    #[test]
    fn parity_files_are_judged_with_the_loader_names_of_the_head() {
        let parity = "use mcrs_minecraft_fixtures::*;\n#[test]\nfn loads() {\n    fixture(\"x\");\n}\n#[test]\nfn loads_nothing() {\n    other();\n}\n#[test]\nfn compares_a_literal() {\n    assert_eq!(fixture(\"x\").text(), \"abc\");\n}\n#[test]\nfn names_a_private_function() {\n    private();\n}\n";
        let (repo, base) = with_base(&[]);
        repo.write("crates/demo/tests/it/x_parity.rs", parity);
        repo.write("crates/demo/tests/it/x_capture.rs", parity);
        let without_loader = scanned(&repo, &base).unwrap();
        assert_eq!(
            without_loader.violations.len(),
            5,
            "{:?}",
            without_loader.violations
        );

        repo.write(
            LOADER_SOURCE,
            "pub fn fixture(name: &str) {}\nfn private() {}\n",
        );
        let report = scanned(&repo, &base).unwrap();
        let expected: BTreeSet<String> = [
            format!(
                "crates/demo/tests/it/x_parity.rs:7: {}",
                rules::LOADS_NO_FIXTURE
            ),
            format!(
                "crates/demo/tests/it/x_parity.rs:12: {}",
                rules::LITERAL_EXPECTED
            ),
            format!(
                "crates/demo/tests/it/x_parity.rs:15: {}",
                rules::LOADS_NO_FIXTURE
            ),
        ]
        .into();
        assert_eq!(report.violations, expected);

        repo.write(LOADER_SOURCE, "pub fn fixture( {\n");
        let message = scanned(&repo, &base)
            .err()
            .expect("the loader does not parse");
        assert!(
            message.starts_with(&format!("{LOADER_SOURCE}:1: does not parse: ")),
            "{message}"
        );
    }

    const CLAIM: &str = "#[test]\nfn vanilla_value() {\n    assert_eq!(got(), 1);\n}\n";

    #[test]
    fn warnings_are_counted_per_crate() {
        let rows = [
            ("crates/a/src/x.rs", "a"),
            ("crates/b/tests/it/y.rs", "b"),
            ("crates/a_b/src/lib.rs", "a_b"),
            ("xtask/src/z.rs", "xtask"),
            ("src/m.rs", "mcrs"),
            ("build.rs", "mcrs"),
            ("crates/loose.rs", "mcrs"),
            ("xtasks/y.rs", "mcrs"),
            ("tools/crates/a/x.rs", "mcrs"),
        ];
        for (path, krate) in rows {
            assert_eq!(crate_of(path), krate, "{path}");
        }

        let (repo, base) = with_base(&[]);
        for path in [
            "crates/a/src/one.rs",
            "crates/b/src/one.rs",
            "crates/b/tests/it/two.rs",
            "xtask/src/one.rs",
            "src/one.rs",
        ] {
            repo.write(path, CLAIM);
        }
        repo.write("crates/c/src/silent.rs", "fn quiet() {}\n");
        let report = scanned(&repo, &base).unwrap();
        assert_eq!(
            counts(&report.warnings),
            BTreeMap::from([("a", 1), ("b", 2), ("mcrs", 1), ("xtask", 1)])
        );
        assert_eq!(
            report_lines(&report, None),
            [
                "parity: warning: crates/a/src/one.rs:2: vanilla_value: claims vanilla behaviour against a literal and loads no fixture",
                "parity: warning: crates/b/src/one.rs:2: vanilla_value: claims vanilla behaviour against a literal and loads no fixture",
                "parity: warning: crates/b/tests/it/two.rs:2: vanilla_value: claims vanilla behaviour against a literal and loads no fixture",
                "parity: warning: src/one.rs:2: vanilla_value: claims vanilla behaviour against a literal and loads no fixture",
                "parity: warning: xtask/src/one.rs:2: vanilla_value: claims vanilla behaviour against a literal and loads no fixture",
                "parity: a: 1 vanilla claims",
                "parity: b: 2 vanilla claims",
                "parity: mcrs: 1 vanilla claims",
                "parity: xtask: 1 vanilla claims",
            ]
        );
        assert_eq!(
            check_message(&repo, &base),
            Ok("6 files; 5 vanilla claims outside parity files".to_owned())
        );
    }

    #[test]
    fn a_claim_never_turns_into_a_violation() {
        let (repo, base) = with_base(&[]);
        repo.write("crates/a/src/one.rs", CLAIM);
        let report = scanned(&repo, &base).unwrap();
        assert!(report.violations.is_empty(), "{:?}", report.violations);
        assert_eq!(report.warnings.len(), 1);

        repo.write("crates/a/src/route.rs", "fn broken( {\n");
        let lines = report_lines(&scanned(&repo, &base).unwrap(), None);
        assert!(lines[0].contains("does not parse"), "{lines:?}");
        assert!(lines[1].starts_with("parity: warning: "), "{lines:?}");
        assert_eq!(check_message(&repo, &base), Err("1 violations".to_owned()));
    }

    #[test]
    fn warnings_in_changed_files_become_annotations() {
        let one = Warning {
            path: "crates/a/src/one.rs".to_owned(),
            line: 3,
            test: "first".to_owned(),
        };
        let two = Warning {
            path: "crates/b/tests/it/two,odd:%.rs".to_owned(),
            line: 9,
            test: "second".to_owned(),
        };
        let multiline = Warning {
            path: "crates/c/line\r\nbreak.rs".to_owned(),
            line: 1,
            test: "third".to_owned(),
        };
        let warnings: BTreeSet<Warning> = [one, two, multiline].into();
        let only = |paths: &[&str]| {
            let changed: Vec<String> = paths.iter().map(|p| (*p).to_owned()).collect();
            annotations(&warnings, &changed)
        };
        assert_eq!(
            only(&["crates/b/tests/it/two,odd:%.rs"]),
            [
                "::warning file=crates/b/tests/it/two%2Codd%3A%25.rs,line=9::second: claims vanilla behaviour against a literal and loads no fixture"
            ]
        );
        assert_eq!(
            only(&["crates/c/line\r\nbreak.rs"]),
            [
                "::warning file=crates/c/line%0D%0Abreak.rs,line=1::third: claims vanilla behaviour against a literal and loads no fixture"
            ]
        );
        assert_eq!(
            only(&["crates/a/src/one.rs", "README.md"]),
            [
                "::warning file=crates/a/src/one.rs,line=3::first: claims vanilla behaviour against a literal and loads no fixture"
            ]
        );
        assert_eq!(only(&["crates/a/src/other.rs"]), Vec::<String>::new());
        assert_eq!(only(&[]), Vec::<String>::new());
        assert_eq!(
            only(&["crates/a/src/one.rs", "crates/b/tests/it/two,odd:%.rs"]).len(),
            2
        );
    }

    #[test]
    fn annotations_follow_the_counts_and_only_when_asked_for() {
        let (repo, base) = with_base(&[]);
        repo.write("crates/a/src/one.rs", CLAIM);
        repo.write("crates/a/src/two.rs", CLAIM);
        let report = scanned(&repo, &base).unwrap();
        let changed = ["crates/a/src/two.rs".to_owned()];

        let plain = report_lines(&report, None);
        assert_eq!(plain.len(), 3, "{plain:?}");
        assert!(
            plain.iter().all(|line| !line.starts_with("::")),
            "{plain:?}"
        );

        let annotated = report_lines(&report, Some(&changed));
        assert_eq!(annotated[..3], plain[..]);
        assert_eq!(
            annotated[3..],
            [
                "::warning file=crates/a/src/two.rs,line=2::vanilla_value: claims vanilla behaviour against a literal and loads no fixture"
            ]
        );
        assert_eq!(report_lines(&report, Some(&[])), plain);
    }
}
