use crate::checks::Outcome;
use crate::scope::{self, Head};
use crate::source::{self, Files, Level, LintAttr, MANIFEST, PackageMap, Tree, Trees, verdict};
use regex::Regex;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const FIX: &str = "use #[expect(<lint>, reason = \"...\")] at the item where the lint fires, with a reason that says why the lint is wrong there, or fix the code; a crate's count may not rise";
const CRATE_WIDE: [&str; 3] = [
    "clippy::too_many_lines",
    "clippy::excessive_nesting",
    "clippy::too_many_arguments",
];
const LINT_GROUPS: [&str; 23] = [
    "clippy::all",
    "clippy::cargo",
    "clippy::complexity",
    "clippy::correctness",
    "clippy::nursery",
    "clippy::pedantic",
    "clippy::perf",
    "clippy::restriction",
    "clippy::style",
    "clippy::suspicious",
    "deprecated_safe",
    "future_incompatible",
    "keyword_idents",
    "let_underscore",
    "nonstandard_style",
    "refining_impl_trait",
    "rust_2018_compatibility",
    "rust_2018_idioms",
    "rust_2021_compatibility",
    "rust_2024_compatibility",
    "unknown_or_malformed_diagnostic_attributes",
    "unused",
    "warnings",
];

fn is_lint_group(lint: &str) -> bool {
    LINT_GROUPS.contains(&lint) || lint == "rustdoc::all"
}

fn cites_issue(reason: &str) -> bool {
    reason
        .as_bytes()
        .windows(2)
        .any(|pair| pair[0] == b'#' && pair[1].is_ascii_digit())
}

#[derive(Default)]
struct Scan {
    allows: BTreeSet<String>,
    expects: BTreeSet<String>,
    sites: Vec<Site>,
    unparsed: Vec<String>,
}

struct Site {
    package: String,
    path: String,
    line: usize,
    lint: String,
}

impl Scan {
    fn add(&mut self, path: &str, package: &str, attr: &LintAttr) {
        let at = format!("{path}:{}", attr.line);
        match attr.level {
            Level::Allow => {
                if attr.lints.is_empty() {
                    self.allows.insert(format!("{at}: allow"));
                }
                for lint in &attr.lints {
                    self.allows.insert(format!("{at}: allow {lint}"));
                }
            }
            Level::Expect => {
                match attr.reason.as_deref().map(str::trim) {
                    None | Some("") => {
                        self.expects
                            .insert(format!("{at}: expect without a reason"));
                    }
                    Some(reason) if cites_issue(reason) => {
                        self.expects
                            .insert(format!("{at}: expect reason cites an issue"));
                    }
                    Some(_) => {}
                }
                for lint in &attr.lints {
                    if attr.inner && CRATE_WIDE.contains(&lint.as_str()) {
                        self.expects
                            .insert(format!("{at}: crate-wide expect of {lint}"));
                    }
                    if is_lint_group(lint) {
                        self.expects
                            .insert(format!("{at}: expect of the lint group {lint}"));
                    }
                }
            }
            Level::Warn | Level::Deny | Level::Forbid => return,
        }
        for lint in &attr.lints {
            self.sites.push(Site {
                package: package.to_owned(),
                path: path.to_owned(),
                line: attr.line,
                lint: lint.clone(),
            });
        }
    }
}

fn scan_sources(files: &Files, packages: &PackageMap) -> Scan {
    let mut scan = Scan::default();
    for (path, text) in files {
        match source::parse(path, text) {
            Ok(parsed) => {
                let package = packages.owner(path);
                for attr in source::lint_attributes(&parsed) {
                    scan.add(path, package, &attr);
                }
            }
            Err(problem) => scan.unparsed.push(problem),
        }
    }
    scan
}

impl Tree<'_> {
    fn scan(&self) -> Result<Scan, String> {
        let files = self.read_rust(|_| true)?;
        Ok(scan_sources(&files, &self.packages))
    }

    fn unopted(&self) -> Result<BTreeSet<String>, String> {
        let Some(root) = self.manifests.get(MANIFEST) else {
            return Ok(BTreeSet::new());
        };
        let root = parse_manifest(root).map_err(|e| format!("{MANIFEST} does not parse: {e}"))?;
        let workspace = root.workspace.unwrap_or_default();
        let mut found = BTreeSet::new();
        for (path, text) in &self.manifests {
            let member = match path.strip_suffix("/Cargo.toml") {
                None => !self.packages.owner(path).is_empty(),
                Some(dir) => {
                    any_glob_matches(&workspace.members, dir)?
                        && !any_glob_matches(&workspace.exclude, dir)?
                }
            };
            if !member {
                continue;
            }
            let manifest =
                parse_manifest(text).map_err(|e| format!("{path} does not parse: {e}"))?;
            if !inherits_workspace(manifest.lints.as_ref()) {
                found.insert(format!("{path}: [lints] must be exactly workspace = true"));
            }
        }
        Ok(found)
    }

    fn configuration(&self) -> Result<BTreeSet<String>, String> {
        let mut found = BTreeSet::new();
        for path in &self.paths {
            let name = source::file_name(path);
            if matches!(name, "clippy.toml" | ".clippy.toml") && path != "clippy.toml" {
                found.insert(format!(
                    "{path}: clippy configuration outside the repository root"
                ));
            }
        }
        let configs: Vec<String> = self
            .paths
            .iter()
            .filter(|path| is_cargo_config(path))
            .cloned()
            .collect();
        for (path, text) in source::read_files(self.repo, &self.head, &configs)? {
            let config: CargoConfig = toml::from_str(&text)
                .map_err(|e| format!("{path} does not parse: {}", e.message()))?;
            config.bypasses(&path, &mut found);
        }
        Ok(found)
    }
}

#[derive(Deserialize)]
struct Manifest {
    lints: Option<toml::Table>,
    workspace: Option<WorkspaceTable>,
}

#[derive(Deserialize, Default)]
struct WorkspaceTable {
    #[serde(default)]
    members: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
}

fn parse_manifest(text: &str) -> Result<Manifest, String> {
    toml::from_str(text).map_err(|e| e.message().to_owned())
}

fn inherits_workspace(lints: Option<&toml::Table>) -> bool {
    lints.is_some_and(|table| {
        table.len() == 1 && table.get("workspace") == Some(&toml::Value::Boolean(true))
    })
}

fn segment_matches(pattern: &str, text: &str) -> bool {
    let wildcard: Vec<String> = pattern.split('*').map(regex::escape).collect();
    Regex::new(&format!("(?s)^{}$", wildcard.join(".*")))
        .is_ok_and(|matcher| matcher.is_match(text))
}

fn glob_matches(pattern: &str, dir: &str) -> Result<bool, String> {
    let patterns: Vec<&str> = pattern.trim_start_matches("./").split('/').collect();
    if patterns
        .iter()
        .any(|segment| segment.contains("**") || segment.contains(['?', '[']))
    {
        return Err(format!(
            "the workspace member pattern `{pattern}` uses glob syntax beyond `*`, which the suppressions check cannot match"
        ));
    }
    let segments: Vec<&str> = dir.split('/').collect();
    Ok(patterns.len() == segments.len()
        && patterns
            .iter()
            .zip(&segments)
            .all(|(pattern, segment)| segment_matches(pattern, segment)))
}

fn any_glob_matches(patterns: &[String], dir: &str) -> Result<bool, String> {
    for pattern in patterns {
        if glob_matches(pattern, dir)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn is_cargo_config(path: &str) -> bool {
    ["config.toml", "config"].iter().any(|name| {
        path.strip_suffix(name)
            .is_some_and(|dir| dir == ".cargo/" || dir.ends_with("/.cargo/"))
    })
}

#[derive(Deserialize)]
struct CargoConfig {
    build: Option<Flags>,
    target: Option<BTreeMap<String, Flags>>,
    env: Option<BTreeMap<String, EnvValue>>,
}

#[derive(Deserialize)]
struct Flags {
    rustflags: Option<Rustflags>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Rustflags {
    Line(String),
    List(Vec<String>),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum EnvValue {
    Plain(String),
    Table { value: String },
}

fn flag_tokens(line: &str) -> Vec<&str> {
    line.split(|c: char| c.is_whitespace() || c == '\x1f')
        .filter(|token| !token.is_empty())
        .collect()
}

fn lint_bypass(tokens: &[&str]) -> Option<String> {
    let crate_attr = |name: &str| name.replace('_', "-").starts_with("crate-attr");
    tokens.iter().enumerate().find_map(|(at, token)| {
        let after_z = at > 0 && tokens[at - 1] == "-Z" && crate_attr(token);
        let bypass = token.starts_with("-A")
            || ["--allow", "--cap-lints"].iter().any(|flag| {
                token == flag
                    || token
                        .strip_prefix(flag)
                        .is_some_and(|rest| rest.starts_with('='))
            })
            || token.strip_prefix("-Z").is_some_and(crate_attr);
        if after_z {
            Some(format!("-Z {token}"))
        } else {
            bypass.then(|| (*token).to_owned())
        }
    })
}

fn note_bypass(found: &mut BTreeSet<String>, path: &str, key: &str, tokens: &[&str]) {
    if let Some(flag) = lint_bypass(tokens) {
        found.insert(format!("{path}: {key} holds {flag}"));
    }
}

impl CargoConfig {
    fn bypasses(&self, path: &str, found: &mut BTreeSet<String>) {
        let tables = self
            .build
            .iter()
            .map(|flags| ("build".to_owned(), flags))
            .chain(
                self.target
                    .iter()
                    .flatten()
                    .map(|(name, flags)| (format!("target.{name}"), flags)),
            );
        for (key, flags) in tables {
            let key = format!("{key}.rustflags");
            match &flags.rustflags {
                Some(Rustflags::Line(line)) => note_bypass(found, path, &key, &flag_tokens(line)),
                Some(Rustflags::List(list)) => {
                    let tokens: Vec<&str> = list.iter().map(String::as_str).collect();
                    note_bypass(found, path, &key, &tokens);
                }
                None => {}
            }
        }
        for (name, value) in self.env.iter().flatten() {
            let (EnvValue::Plain(text) | EnvValue::Table { value: text }) = value;
            if matches!(name.as_str(), "RUSTFLAGS" | "CARGO_ENCODED_RUSTFLAGS") {
                note_bypass(found, path, &format!("env.{name}"), &flag_tokens(text));
            }
            if name == "CLIPPY_CONF_DIR" {
                found.insert(format!("{path}: env.CLIPPY_CONF_DIR is set"));
            }
        }
    }
}

fn counts(sites: &[Site]) -> BTreeMap<&str, usize> {
    let mut counts = BTreeMap::new();
    for site in sites {
        *counts.entry(site.package.as_str()).or_default() += 1;
    }
    counts
}

fn added_sites(package: &str, now: &[Site], before: &[Site]) -> Vec<String> {
    let mut unmatched: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for site in before.iter().filter(|site| site.package == package) {
        *unmatched
            .entry((site.path.as_str(), site.lint.as_str()))
            .or_default() += 1;
    }
    let mut added = Vec::new();
    for site in now.iter().filter(|site| site.package == package) {
        match unmatched.get_mut(&(site.path.as_str(), site.lint.as_str())) {
            Some(left) if *left > 0 => *left -= 1,
            _ => added.push(format!("{}:{} {}", site.path, site.line, site.lint)),
        }
    }
    added
}

fn over_base(now: &Scan, before: &Scan, existed: &PackageMap) -> BTreeSet<String> {
    let (now_counts, before_counts) = (counts(&now.sites), counts(&before.sites));
    let mut removed: usize = before_counts
        .iter()
        .map(|(package, was)| was.saturating_sub(now_counts.get(package).copied().unwrap_or(0)))
        .sum();
    let mut over = BTreeSet::new();
    for (package, &count) in &now_counts {
        let excess = if existed.contains(package) {
            let was = before_counts.get(package).copied().unwrap_or(0);
            (count > was).then(|| format!("{was} at the base"))
        } else if count <= removed {
            removed -= count;
            None
        } else {
            Some(format!("{removed} removed elsewhere in this change"))
        };
        if let Some(base) = excess {
            over.insert(format!(
                "{package}: {count} suppressed lint paths, {base}; added {}",
                added_sites(package, &now.sites, &before.sites).join(", ")
            ));
        }
    }
    over
}

pub(crate) fn check(repo: &Path, base: &str, head: &Head) -> Outcome {
    let trees = Trees::read(repo, base, head, |lints| lints.suppressions.is_some())?;
    let now = trees.head.scan()?;
    if !now.unparsed.is_empty() {
        return Err(now.unparsed.join("; "));
    }
    let before = trees.base.scan()?;
    let over = over_base(&now, &before, &trees.base.packages);
    let manifests = trees.head.unopted()?;
    if !trees.in_force {
        return Ok(format!(
            "not in force yet at {}; {} allow sites, {} expect findings, {} manifests not opted in, {} packages over their base count",
            scope::short(base),
            now.allows.len(),
            now.expects.len(),
            manifests.len(),
            over.len()
        ));
    }
    let mut findings = BTreeSet::new();
    findings.extend(now.allows.iter().cloned());
    findings.extend(now.expects.iter().cloned());
    findings.extend(manifests);
    findings.extend(trees.head.configuration()?);
    findings.extend(over);
    verdict(&findings, FIX, || {
        let packages: BTreeSet<&str> = now.sites.iter().map(|site| site.package.as_str()).collect();
        format!(
            "no allow; {} expect lint paths in {} packages",
            now.sites.len(),
            packages.len()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scope::tests::judge_rows_in_parallel;
    use crate::source::fixture::{self, At, Row, Verdict, del, files, put, row};
    use std::collections::BTreeMap;
    use std::process::Command;

    fn scan_one(source: &str) -> Scan {
        let tree = files(&[("lib.rs", source)]);
        let packages = PackageMap::new(&files(&[("Cargo.toml", "[package]\nname = \"p\"\n")]))
            .expect("a manifest");
        scan_sources(&tree, &packages)
    }

    #[test]
    fn every_form_of_allow_is_a_suppression() {
        let rows: [(&str, &str, &[(usize, &str)]); 12] = [
            (
                "outer",
                "#[allow(dead_code)]\nfn f() {}\n",
                &[(1, "dead_code")],
            ),
            ("inner", "#![allow(unused)]\nfn f() {}\n", &[(1, "unused")]),
            (
                "several lints",
                "#[allow(dead_code, clippy::too_many_lines)]\nfn f() {}\n",
                &[(1, "dead_code"), (1, "clippy::too_many_lines")],
            ),
            (
                "behind cfg_attr",
                "#[cfg_attr(test, allow(dead_code))]\nfn f() {}\n",
                &[(1, "dead_code")],
            ),
            (
                "behind nested cfg_attr",
                "#[cfg_attr(unix, cfg_attr(test, allow(dead_code)))]\nfn f() {}\n",
                &[(1, "dead_code")],
            ),
            (
                "under a cfg that is never active",
                "#[cfg(any())]\n#[allow(dead_code)]\nfn f() {}\n",
                &[(2, "dead_code")],
            ),
            (
                "on a statement",
                "fn f() {\n    #[allow(unused_variables)]\n    let x = 1;\n}\n",
                &[(2, "unused_variables")],
            ),
            (
                "in a macro_rules body",
                "macro_rules! m {\n    () => {\n        #[allow(dead_code)]\n        fn f() {}\n    };\n}\n",
                &[(3, "dead_code")],
            ),
            (
                "in a quote body",
                "fn f() {\n    let _ = quote! {\n        #[allow(unused_imports)]\n        use x;\n    };\n}\n",
                &[(3, "unused_imports")],
            ),
            (
                "inner attribute in a macro body",
                "fn f() {\n    m! {\n        #![allow(unused)]\n    }\n}\n",
                &[(3, "unused")],
            ),
            (
                "raising levels",
                "#![warn(clippy::dbg_macro)]\n#[deny(missing_docs)]\n#[forbid(unsafe_code)]\nfn f() {}\n",
                &[],
            ),
            (
                "a reasoned expectation",
                "#[expect(dead_code, reason = \"kept for the decoder\")]\nfn f() {}\n",
                &[],
            ),
        ];
        for (name, source, expected) in rows {
            let found: Vec<String> = scan_one(source).allows.into_iter().collect();
            let mut wanted: Vec<String> = expected
                .iter()
                .map(|(line, lint)| format!("lib.rs:{line}: allow {lint}"))
                .collect();
            wanted.sort();
            assert_eq!(found, wanted, "{name}");
        }
    }

    #[test]
    fn an_expectation_needs_a_reason_that_cites_no_issue() {
        let rows: [(&str, &str, &[&str], usize); 13] = [
            (
                "no reason",
                "#[expect(dead_code)]\nfn f() {}\n",
                &["lib.rs:1: expect without a reason"],
                1,
            ),
            (
                "empty reason",
                "#[expect(dead_code, reason = \"\")]\nfn f() {}\n",
                &["lib.rs:1: expect without a reason"],
                1,
            ),
            (
                "blank reason",
                "#[expect(dead_code, reason = \"   \")]\nfn f() {}\n",
                &["lib.rs:1: expect without a reason"],
                1,
            ),
            (
                "reason citing an issue",
                "#[expect(dead_code, reason = \"see #123\")]\nfn f() {}\n",
                &["lib.rs:1: expect reason cites an issue"],
                1,
            ),
            (
                "reason with a number sign and no digit",
                "#[expect(dead_code, reason = \"C# interop\")]\nfn f() {}\n",
                &[],
                1,
            ),
            (
                "proper reason",
                "#[expect(dead_code, reason = \"kept for the decoder\")]\nfn f() {}\n",
                &[],
                1,
            ),
            (
                "crate-wide complexity expectation",
                "#![expect(clippy::too_many_lines, reason = \"x\")]\n",
                &["lib.rs:1: crate-wide expect of clippy::too_many_lines"],
                1,
            ),
            (
                "the same on an item",
                "#[expect(clippy::too_many_lines, reason = \"x\")]\nfn f() {}\n",
                &[],
                1,
            ),
            (
                "clippy group",
                "#[expect(clippy::pedantic, reason = \"x\")]\nfn f() {}\n",
                &["lib.rs:1: expect of the lint group clippy::pedantic"],
                1,
            ),
            (
                "warnings",
                "#[expect(warnings, reason = \"x\")]\nfn f() {}\n",
                &["lib.rs:1: expect of the lint group warnings"],
                1,
            ),
            (
                "rustc group",
                "#[expect(unused, reason = \"x\")]\nfn f() {}\n",
                &["lib.rs:1: expect of the lint group unused"],
                1,
            ),
            (
                "group behind cfg_attr",
                "#[cfg_attr(test, expect(nonstandard_style))]\nfn f() {}\n",
                &[
                    "lib.rs:1: expect of the lint group nonstandard_style",
                    "lib.rs:1: expect without a reason",
                ],
                1,
            ),
            (
                "two lints in one inner expectation",
                "#![expect(clippy::disallowed_types, clippy::disallowed_macros, reason = \"x\")]\n",
                &[],
                2,
            ),
        ];
        for (name, source, expected, paths) in rows {
            let scan = scan_one(source);
            let found: Vec<&str> = scan.expects.iter().map(String::as_str).collect();
            assert_eq!(found, expected, "{name}");
            assert_eq!(scan.sites.len(), paths, "{name}");
        }
    }

    fn groups_in(output: &str, heading: &str) -> BTreeSet<String> {
        output
            .lines()
            .skip_while(|line| !line.starts_with(heading))
            .skip_while(|line| !line.contains("----"))
            .skip(1)
            .take_while(|line| !line.trim().is_empty())
            .filter_map(|line| line.split_whitespace().next())
            .map(|name| name.replace('-', "_"))
            .collect()
    }

    fn help_of(program: &str) -> String {
        let output = Command::new(program)
            .args(["-W", "help"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .unwrap_or_else(|e| panic!("could not run {program}: {e}"));
        assert!(output.status.success(), "{program} -W help failed");
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    #[test]
    fn the_lint_groups_match_the_pinned_toolchain() {
        let mut printed = groups_in(&help_of("rustc"), "Lint groups provided by rustc");
        printed.extend(groups_in(
            &help_of("clippy-driver"),
            "Lint groups loaded by this crate",
        ));
        let listed: BTreeSet<String> = LINT_GROUPS.iter().map(|g| (*g).to_owned()).collect();
        assert!(printed.contains("warnings") && printed.contains("clippy::all"));
        assert_eq!(listed, printed);
    }

    const ROOT: &str = "[package]\nname = \"root\"\nversion = \"0.1.0\"\n\n[lints]\nworkspace = true\n\n[workspace]\nmembers = [\"crates/*\"]\n";
    const OPTED_IN: &str = "[lints]\nworkspace = true\n";
    const LINTS_ON: &str = "[suppressions]\n";
    const LINTS_FILE: &str = "quality/lints.toml";
    const A_ROOT: &str = "crates/a/src/lib.rs";
    const B_ROOT: &str = "crates/b/src/lib.rs";
    const B_MANIFEST: &str = "crates/b/Cargo.toml";
    const CONFIG: &str = ".cargo/config.toml";
    const PLAIN: &str = "fn a() {}\n";
    const ONE_EXPECT: &str =
        "#[expect(dead_code, reason = \"kept for the decoder\")]\nfn one() {}\n";
    const TWO_EXPECTS: &str = "#[expect(dead_code, reason = \"kept for the decoder\")]\nfn one() {}\n#[expect(unused_variables, reason = \"kept for the decoder\")]\nfn two() {}\n";
    const AN_ALLOW: &str = "#[allow(dead_code)]\nfn one() {}\n";

    fn manifest(name: &str, lints: &str) -> String {
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n\n{lints}")
    }

    fn standard() -> BTreeMap<&'static str, String> {
        BTreeMap::from([
            ("Cargo.toml", ROOT.to_owned()),
            ("src/main.rs", "fn main() {}\n".to_owned()),
            ("crates/a/Cargo.toml", manifest("a", OPTED_IN)),
            (A_ROOT, PLAIN.to_owned()),
            (B_MANIFEST, manifest("b", OPTED_IN)),
            (B_ROOT, "fn b() {}\n".to_owned()),
            (LINTS_FILE, LINTS_ON.to_owned()),
        ])
    }

    fn judge(row: &Row) -> Option<String> {
        fixture::judge("suppressions", &standard(), check, row)
    }

    fn config_rows() -> Vec<Row> {
        let cases: [(&str, &str, Verdict); 9] = [
            (
                "-A as its own flag",
                "[build]\nrustflags = [\"-A\", \"dead_code\"]\n",
                Verdict::Fail(&[".cargo/config.toml: build.rustflags holds -A"]),
            ),
            (
                "--cap-lints",
                "[build]\nrustflags = [\"--cap-lints\", \"warn\"]\n",
                Verdict::Fail(&["build.rustflags holds --cap-lints"]),
            ),
            (
                "-Awarnings",
                "[build]\nrustflags = [\"-Awarnings\"]\n",
                Verdict::Fail(&["build.rustflags holds -Awarnings"]),
            ),
            (
                "-Zcrate-attr",
                "[build]\nrustflags = [\"-Zcrate-attr=allow(dead_code)\"]\n",
                Verdict::Fail(&["build.rustflags holds -Zcrate-attr=allow(dead_code)"]),
            ),
            (
                "-Z crate-attr as two flags",
                "[build]\nrustflags = [\"-Z\", \"crate-attr=allow(dead_code)\"]\n",
                Verdict::Fail(&["build.rustflags holds -Z crate-attr=allow(dead_code)"]),
            ),
            (
                "--allow under a cfg target",
                "[target.'cfg(unix)']\nrustflags = [\"--allow=dead_code\"]\n",
                Verdict::Fail(&["target.cfg(unix).rustflags holds --allow=dead_code"]),
            ),
            (
                "RUSTFLAGS in the environment",
                "[env]\nRUSTFLAGS = \"-D warnings -Aunused\"\n",
                Verdict::Fail(&["env.RUSTFLAGS holds -Aunused"]),
            ),
            (
                "encoded flags as a table",
                "[env]\nCARGO_ENCODED_RUSTFLAGS = { value = \"-Adead_code\" }\n",
                Verdict::Fail(&["env.CARGO_ENCODED_RUSTFLAGS holds -Adead_code"]),
            ),
            (
                "flags that only set cfgs and unstable options",
                "[build]\nrustflags = [\"--cfg\", \"web_sys_unstable_apis\", \"-Zshare-generics=y\", \"-Dwarnings\"]\n",
                Verdict::Pass(&["no allow"]),
            ),
        ];
        cases
            .into_iter()
            .map(|(name, text, verdict)| row(name, vec![], vec![put(CONFIG, text)], verdict))
            .collect()
    }

    fn count_rows() -> Vec<Row> {
        use Verdict::{Fail, Pass};
        vec![
            row(
                "an expect added to a package fails and names the site",
                vec![put(A_ROOT, ONE_EXPECT)],
                vec![put(A_ROOT, TWO_EXPECTS)],
                Fail(&[
                    "a: 2 suppressed lint paths, 1 at the base",
                    "crates/a/src/lib.rs:3 unused_variables",
                ]),
            ),
            row(
                "a falling count passes",
                vec![put(A_ROOT, TWO_EXPECTS)],
                vec![put(A_ROOT, ONE_EXPECT)],
                Pass(&["no allow; 1 expect lint paths in 1 packages"]),
            ),
            row(
                "an expect moved into a new package passes",
                vec![put(A_ROOT, ONE_EXPECT)],
                vec![
                    put(A_ROOT, PLAIN),
                    ("crates/c/Cargo.toml", Some(manifest("c", OPTED_IN))),
                    put("crates/c/src/lib.rs", ONE_EXPECT),
                ],
                Pass(&["no allow; 1 expect lint paths in 1 packages"]),
            ),
            row(
                "a new package holding one more than was removed fails",
                vec![put(A_ROOT, ONE_EXPECT)],
                vec![
                    put(A_ROOT, PLAIN),
                    ("crates/c/Cargo.toml", Some(manifest("c", OPTED_IN))),
                    put("crates/c/src/lib.rs", TWO_EXPECTS),
                ],
                Fail(&[
                    "c: 2 suppressed lint paths",
                    "crates/c/src/lib.rs:3 unused_variables",
                ]),
            ),
            row(
                "a member without a lints table fails",
                vec![],
                vec![put(B_MANIFEST, &manifest("b", ""))],
                Fail(&["crates/b/Cargo.toml: [lints] must be exactly workspace = true"]),
            ),
            row(
                "a member with a lints table of its own fails",
                vec![],
                vec![put(
                    B_MANIFEST,
                    &manifest("b", "[lints.clippy]\ntoo_many_lines = \"allow\"\n"),
                )],
                Fail(&["crates/b/Cargo.toml: [lints] must be exactly workspace = true"]),
            ),
            row(
                "the root package must opt in too",
                vec![],
                vec![put(
                    "Cargo.toml",
                    &ROOT.replace("[lints]\nworkspace = true\n\n", ""),
                )],
                Fail(&["Cargo.toml: [lints] must be exactly workspace = true"]),
            ),
        ]
    }

    fn file_rows() -> Vec<Row> {
        use Verdict::{Fail, Pass};
        vec![
            row(
                "clippy configuration in a package fails",
                vec![],
                vec![put("crates/a/clippy.toml", "")],
                Fail(&["crates/a/clippy.toml: clippy configuration outside the repository root"]),
            ),
            row(
                "a hidden clippy configuration at the root fails",
                vec![],
                vec![put(".clippy.toml", "")],
                Fail(&[".clippy.toml: clippy configuration outside the repository root"]),
            ),
            row(
                "the root clippy configuration is fine",
                vec![],
                vec![put("clippy.toml", "")],
                Pass(&["no allow"]),
            ),
            row(
                "CLIPPY_CONF_DIR in the environment fails",
                vec![],
                vec![put(CONFIG, "[env]\nCLIPPY_CONF_DIR = \"tools\"\n")],
                Fail(&[".cargo/config.toml: env.CLIPPY_CONF_DIR is set"]),
            ),
            row(
                "an allow in a committed file fails",
                vec![],
                vec![
                    put("crates/a/src/extra.rs", AN_ALLOW),
                    put(A_ROOT, "mod extra;\n"),
                ],
                Fail(&["crates/a/src/extra.rs:1: allow dead_code"]),
            ),
            row(
                "an allow in an untracked file passes for the working tree",
                vec![],
                vec![put("crates/a/src/extra.rs", AN_ALLOW)],
                Pass(&["no allow"]),
            )
            .at(At::Worktree),
            row(
                "an allow in a modified tracked file fails for the working tree",
                vec![],
                vec![put(A_ROOT, AN_ALLOW)],
                Fail(&["crates/a/src/lib.rs:1: allow dead_code"]),
            )
            .at(At::Worktree),
            row(
                "an allow in the head commit fails while the checkout sits at the base",
                vec![],
                vec![put(A_ROOT, AN_ALLOW)],
                Fail(&["crates/a/src/lib.rs:1: allow dead_code"]),
            )
            .at(At::DetachedAtBase),
            row(
                "a file that does not parse fails naming its path",
                vec![],
                vec![put(A_ROOT, "fn broken( {\n")],
                Fail(&["crates/a/src/lib.rs:", "does not parse"]),
            ),
        ]
    }

    fn base_rows() -> Vec<Row> {
        use Verdict::Pass;
        vec![
            row(
                "without a lints file the check is not in force and counts the allows",
                vec![del(LINTS_FILE), put(A_ROOT, AN_ALLOW)],
                vec![],
                Pass(&[
                    "not in force yet at ",
                    "; 1 allow sites, 0 expect findings, 0 manifests not opted in, 0 packages over their base count",
                ]),
            ),
            row(
                "a lints file without the suppressions table is not in force",
                vec![put(LINTS_FILE, "[bypasses]\n")],
                vec![put(A_ROOT, AN_ALLOW), put(B_MANIFEST, &manifest("b", ""))],
                Pass(&[
                    "not in force yet at ",
                    "; 1 allow sites, 0 expect findings, 1 manifests not opted in, 1 packages over their base count",
                ]),
            ),
            row(
                "not in force, a rising count is reported and passes",
                vec![del(LINTS_FILE), put(A_ROOT, ONE_EXPECT)],
                vec![put(A_ROOT, TWO_EXPECTS)],
                Pass(&[
                    "; 0 allow sites, 0 expect findings, 0 manifests not opted in, 1 packages over their base count",
                ]),
            ),
        ]
    }

    fn rows() -> Vec<Row> {
        let mut rows = count_rows();
        rows.extend(file_rows());
        rows.extend(base_rows());
        rows.extend(config_rows());
        rows
    }

    #[test]
    fn the_check_judges_suppressions_against_the_base() {
        judge_rows_in_parallel(&rows(), judge);
    }
}
