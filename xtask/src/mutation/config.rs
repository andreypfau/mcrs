use crate::metadata::Workspace;
use crate::mutation::tool::{self, Mode, Spec};
use crate::scope::{self, Found, Head};
use regex::RegexSet;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub(crate) const CONFIG_FILE: &str = ".cargo/mutants.toml";

const LISTING_DIR: &str = "target/mutation/config";
const TEST_TOOL: &str = "nextest";
const PROFILE: &str = "mutants";
const TEST_ARGS: [&str; 2] = ["--profile=mutants", "--no-tests=pass"];
const MATRIX_TEST_ARGS: [&str; 2] = ["--profile=mutants-matrix", "--no-tests=pass"];
const REGEX_METACHARACTERS: &str = "\\.+*?()|[]{}^$";
const POSITION: &str = r":\d+:\d+: ";
const WRITE_AS: &str = "\"<pattern>\", # <reason>";

#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    test_tool: String,
    profile: String,
    additional_cargo_test_args: Vec<String>,
    gitignore: bool,
    exclude_re: Vec<String>,
}

impl Config {
    fn shared() -> Config {
        Config {
            test_tool: TEST_TOOL.to_owned(),
            profile: PROFILE.to_owned(),
            additional_cargo_test_args: TEST_ARGS.map(str::to_owned).into(),
            gitignore: true,
            exclude_re: Vec::new(),
        }
    }

    fn validate(&self, revision: &str) -> Result<(), String> {
        let shared = Config::shared();
        let wrong = |key: &str, wanted: &dyn std::fmt::Debug| {
            Err(format!(
                "{CONFIG_FILE} {revision}: {key} must be {wanted:?}"
            ))
        };
        if self.test_tool != shared.test_tool {
            return wrong("test_tool", &shared.test_tool);
        }
        if self.profile != shared.profile {
            return wrong("profile", &shared.profile);
        }
        if self.additional_cargo_test_args != shared.additional_cargo_test_args {
            return wrong(
                "additional_cargo_test_args",
                &shared.additional_cargo_test_args,
            );
        }
        if self.gitignore != shared.gitignore {
            return wrong("gitignore", &shared.gitignore);
        }
        RegexSet::new(&self.exclude_re).map_err(|e| {
            format!("{CONFIG_FILE} {revision}: exclude_re holds a pattern that is not a regular expression: {e}")
        })?;
        Ok(())
    }

    pub(crate) fn exclusions(&self) -> BTreeSet<&str> {
        self.exclude_re.iter().map(String::as_str).collect()
    }

    pub(crate) fn shares_keys_with(&self, other: &Config) -> bool {
        self.test_tool == other.test_tool
            && self.profile == other.profile
            && self.additional_cargo_test_args == other.additional_cargo_test_args
            && self.gitignore == other.gitignore
    }
}

pub(crate) fn parse_shape(text: &str) -> Result<Config, String> {
    toml::from_str(text).map_err(|e| e.message().to_owned())
}

pub(crate) fn parse(text: &str, revision: &str) -> Result<Config, String> {
    let config = parse_shape(text)
        .map_err(|message| format!("{CONFIG_FILE} {revision} is malformed: {message}"))?;
    config.validate(revision)?;
    Ok(config)
}

fn listing_text(config: &Config) -> String {
    format!(
        "test_tool = {:?}\nprofile = {:?}\nadditional_cargo_test_args = {:?}\ngitignore = {}\nexclude_re = []\n",
        config.test_tool, config.profile, config.additional_cargo_test_args, config.gitignore
    )
}

pub(crate) fn write_listing(repo: &Path, config: &Config) -> Result<PathBuf, String> {
    let dir = repo.join(LISTING_DIR);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    let path = dir.join("listing.toml");
    std::fs::write(&path, listing_text(config))
        .map_err(|e| format!("could not write {}: {e}", path.display()))?;
    Ok(path)
}

fn toml_string(text: &str) -> String {
    let mut quoted = String::from("\"");
    for c in text.chars() {
        match c {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            c if c.is_control() => quoted.push_str(&format!("\\u{:04X}", c as u32)),
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

fn text_with_test_args(config: &Config, test_args: &[impl std::fmt::Debug]) -> String {
    let exclusions: Vec<String> = config.exclude_re.iter().map(|p| toml_string(p)).collect();
    format!(
        "test_tool = {:?}\nprofile = {:?}\nadditional_cargo_test_args = {:?}\ngitignore = {}\nexclude_re = [{}]\n",
        config.test_tool,
        config.profile,
        test_args,
        config.gitignore,
        exclusions.join(", ")
    )
}

fn matrix_text(config: &Config) -> String {
    text_with_test_args(config, &MATRIX_TEST_ARGS)
}

fn gate_text(config: &Config) -> String {
    text_with_test_args(config, &config.additional_cargo_test_args)
}

pub(crate) fn write_gate(repo: &Path, config: &Config) -> Result<PathBuf, String> {
    let dir = repo.join(LISTING_DIR);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    let path = dir.join("gate.toml");
    std::fs::write(&path, gate_text(config))
        .map_err(|e| format!("could not write {}: {e}", path.display()))?;
    Ok(path)
}

pub(crate) fn write_matrix(repo: &Path, config: &Config) -> Result<PathBuf, String> {
    let dir = repo.join(LISTING_DIR);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    let path = dir.join("matrix.toml");
    std::fs::write(&path, matrix_text(config))
        .map_err(|e| format!("could not write {}: {e}", path.display()))?;
    Ok(path)
}

pub(crate) fn base_config(repo: &Path, base: &str, head: &Head) -> Result<Config, String> {
    let short = scope::short(base);
    Ok(
        match scope::configuration_at(repo, base, head, CONFIG_FILE)? {
            None => Config::shared(),
            Some(Found::AtBase(text)) => parse(&text, &format!("at the base {short}"))?,
            Some(Found::Restored { last, .. }) => {
                parse(&last, &format!("before its removal from the base {short}"))?
            }
        },
    )
}

#[derive(Debug, Deserialize)]
pub(crate) struct Mutant {
    pub name: String,
    pub package: String,
    pub file: String,
    pub function: Option<Function>,
    pub span: Span,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Function {
    pub function_name: String,
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub(crate) struct Span {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub(crate) struct Position {
    pub line: u32,
}

pub(crate) fn list(repo: &Path, config: &Path, package: &str) -> Result<Vec<Mutant>, String> {
    list_with(repo, config, package, None, &[])
}

pub(crate) fn list_exit_hint(code: Option<i32>) -> &'static str {
    match code {
        Some(5) => {
            " (exit 5: the diff does not match the tree, so the tree is not the commit the diff was built from)"
        }
        Some(6) => " (exit 6: the diff cannot be parsed)",
        _ => "",
    }
}

pub(crate) fn list_with(
    tree: &Path,
    config: &Path,
    package: &str,
    in_diff: Option<&Path>,
    files: &[String],
) -> Result<Vec<Mutant>, String> {
    let spec = Spec {
        tree,
        config,
        package,
        mode: Mode::List,
        shard: None,
        in_diff,
        files,
    };
    let out = tool::output(tool::command(&spec))?;
    if !out.status.success() {
        let said = String::from_utf8_lossy(&out.stderr);
        let said = said.lines().rev().find(|line| !line.trim().is_empty());
        return Err(format!(
            "cargo mutants could not list the mutants of {package}: {}{}; fix the source or manifest it names",
            said.unwrap_or("no message").trim(),
            list_exit_hint(out.status.code())
        ));
    }
    if in_diff.is_some() && out.stdout.iter().all(u8::is_ascii_whitespace) {
        return Ok(Vec::new());
    }
    let mutants: Vec<Mutant> = serde_json::from_slice(&out.stdout).map_err(|e| {
        format!("cargo mutants printed a listing of {package} that cannot be read: {e}")
    })?;
    if let Some(other) = mutants.iter().find(|mutant| mutant.package != package) {
        return Err(format!(
            "cargo mutants listed {} for {package}, but it belongs to {}",
            other.name, other.package
        ));
    }
    Ok(mutants)
}

pub(crate) fn list_named(
    tree: &Path,
    config: &Path,
    package: &str,
    in_diff: Option<&Path>,
    names: &[String],
) -> Result<Vec<String>, String> {
    let spec = Spec {
        tree,
        config,
        package,
        mode: Mode::ListNamed { names },
        shard: None,
        in_diff,
        files: &[],
    };
    let out = tool::output(tool::command(&spec))?;
    if !out.status.success() {
        let said = String::from_utf8_lossy(&out.stderr);
        let said = said.lines().rev().find(|line| !line.trim().is_empty());
        return Err(format!(
            "cargo mutants could not list the survivors of {package}: {}{}",
            said.unwrap_or("no message").trim(),
            list_exit_hint(out.status.code())
        ));
    }
    if out.stdout.iter().all(u8::is_ascii_whitespace) {
        return Ok(Vec::new());
    }
    let listed: Vec<Mutant> = serde_json::from_slice(&out.stdout).map_err(|e| {
        format!(
            "cargo mutants printed a listing of the survivors of {package} that cannot be read: {e}"
        )
    })?;
    Ok(listed.into_iter().map(|mutant| mutant.name).collect())
}

#[derive(Debug, PartialEq)]
pub(crate) struct Entry {
    pub line: usize,
    pub pattern: String,
    pub file: String,
}

struct Scanned {
    line: usize,
    pattern: String,
}

#[derive(Deserialize)]
struct BasicString {
    value: String,
}

fn at_line(line: usize, message: impl std::fmt::Display) -> String {
    format!("{CONFIG_FILE}:{line}: {message}")
}

fn basic_string_end(text: &str) -> Option<usize> {
    let mut chars = text.char_indices().skip(1);
    while let Some((index, c)) = chars.next() {
        match c {
            '\\' => {
                chars.next();
            }
            '"' => return Some(index),
            _ => {}
        }
    }
    None
}

fn scan_entry(line: usize, text: &str, problems: &mut Vec<String>) -> Option<Scanned> {
    if !text.starts_with('"') {
        problems.push(at_line(
            line,
            format!("an exclusion is a double-quoted string, a comma and a reason; write it as {WRITE_AS}"),
        ));
        return None;
    }
    let Some(end) = basic_string_end(text) else {
        problems.push(at_line(
            line,
            format!(
                "the string is not closed on this line; an exclusion sits on one line as {WRITE_AS}"
            ),
        ));
        return None;
    };
    let pattern = match toml::from_str::<BasicString>(&format!("value = {}", &text[..=end])) {
        Ok(parsed) => parsed.value,
        Err(e) => {
            problems.push(at_line(
                line,
                format!("the string is not valid TOML: {}", e.message()),
            ));
            return None;
        }
    };
    let rest = text[end + 1..].trim();
    let Some(after_comma) = rest.strip_prefix(',') else {
        problems.push(at_line(
            line,
            format!("an exclusion ends with a comma and a reason; write it as {WRITE_AS}"),
        ));
        return None;
    };
    let after_comma = after_comma.trim();
    let reason = after_comma.strip_prefix('#').map(str::trim);
    if reason.is_none_or(str::is_empty) {
        problems.push(at_line(
            line,
            format!("the exclusion has no reason; add one after the comma as {WRITE_AS}"),
        ));
    }
    Some(Scanned { line, pattern })
}

fn scan(text: &str, problems: &mut Vec<String>) -> (usize, Vec<Scanned>) {
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(index, line)| (index + 1, line));
    let opening = lines.find_map(|(number, line)| {
        let rest = line.trim().strip_prefix("exclude_re")?.trim_start();
        rest.strip_prefix('=')
            .map(|rest| (number, rest.trim().to_owned()))
    });
    let Some((key_line, opening)) = opening else {
        problems.push(format!("{CONFIG_FILE}: exclude_re is missing"));
        return (0, Vec::new());
    };
    let mut entries = Vec::new();
    match opening.as_str() {
        "[]" => {}
        "[" => {
            let mut closed = false;
            for (number, line) in lines {
                let line = line.trim();
                if line == "]" {
                    closed = true;
                    break;
                }
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                entries.extend(scan_entry(number, line, problems));
            }
            if !closed {
                problems.push(at_line(key_line, "exclude_re is never closed; end it with ] on its own line"));
            }
        }
        _ => problems.push(at_line(
            key_line,
            format!("exclude_re opens with [ alone so that each entry sits on its own line as {WRITE_AS}"),
        )),
    }
    (key_line, entries)
}

fn shape(pattern: &str) -> Result<String, String> {
    let body = pattern
        .strip_prefix('^')
        .ok_or("the pattern is not anchored; start it with ^ and end it with $")?;
    let body = body
        .strip_suffix('$')
        .filter(|rest| rest.chars().rev().take_while(|&c| c == '\\').count() % 2 == 0)
        .ok_or("the pattern is not anchored; start it with ^ and end it with $")?;
    let mut file = String::new();
    let mut chars = body.char_indices();
    let rest = loop {
        match chars.next() {
            None => {
                return Err(format!(
                    "the pattern names no position; follow the file with {POSITION}"
                ));
            }
            Some((_, '\\')) => match chars.next() {
                Some((_, c)) if REGEX_METACHARACTERS.contains(c) => file.push(c),
                _ => {
                    return Err(
                        "a backslash in the file part may only escape a regex metacharacter"
                            .to_owned(),
                    );
                }
            },
            Some((index, ':')) => break &body[index..],
            Some((_, c)) if REGEX_METACHARACTERS.contains(c) => {
                return Err(format!(
                    "the file part holds the regex metacharacter {c}; write it as \\{c}"
                ));
            }
            Some((_, c)) => file.push(c),
        }
    };
    let change = rest.strip_prefix(POSITION).ok_or_else(|| {
        format!("the line and column must be written exactly as {POSITION} so the entry survives edits above it")
    })?;
    if change.is_empty() {
        return Err(
            "name the change after the position, such as replace == with != in Palette::index"
                .to_owned(),
        );
    }
    Ok(file)
}

pub(crate) fn format_rules(
    text: &str,
    parsed: &Config,
    tracked: &BTreeSet<String>,
) -> Result<Vec<Entry>, Vec<String>> {
    let mut problems = Vec::new();
    let (key_line, scanned) = scan(text, &mut problems);
    let mut entries = Vec::new();
    for found in &scanned {
        match shape(&found.pattern) {
            Ok(file) if tracked.contains(&file) => entries.push(Entry {
                line: found.line,
                pattern: found.pattern.clone(),
                file,
            }),
            Ok(file) => problems.push(at_line(
                found.line,
                format!("{file} is not a tracked file at the head; name a file that exists"),
            )),
            Err(why) => problems.push(at_line(found.line, why)),
        }
    }
    let patterns: Vec<&String> = scanned.iter().map(|found| &found.pattern).collect();
    if problems.is_empty() && patterns.iter().copied().ne(parsed.exclude_re.iter()) {
        problems.push(at_line(
            key_line,
            "the entries read line by line differ from the entries TOML parses; write each entry as a plain basic string on its own line",
        ));
    }
    if problems.is_empty() {
        Ok(entries)
    } else {
        Err(problems)
    }
}

fn judge_entries(entries: &[Entry], mutants: &[Mutant]) -> Result<(), Vec<String>> {
    let patterns: Vec<&str> = entries.iter().map(|entry| entry.pattern.as_str()).collect();
    let set = RegexSet::new(&patterns).map_err(|e| vec![format!("{CONFIG_FILE}: {e}")])?;
    let mut matched: Vec<BTreeSet<(&str, &str)>> = vec![BTreeSet::new(); entries.len()];
    for mutant in mutants {
        let function = mutant
            .function
            .as_ref()
            .map_or("(outside any function)", |f| f.function_name.as_str());
        for index in set.matches(&mutant.name) {
            matched[index].insert((mutant.file.as_str(), function));
        }
    }
    let mut problems = Vec::new();
    for (entry, pairs) in entries.iter().zip(&matched) {
        if pairs.is_empty() {
            problems.push(at_line(
                entry.line,
                "the exclusion is stale because its pattern matches no mutant; drop the entry",
            ));
        } else if pairs.len() > 1 {
            let shown: Vec<String> = pairs
                .iter()
                .map(|(file, function)| format!("{function} in {file}"))
                .collect();
            problems.push(at_line(
                entry.line,
                format!(
                    "the pattern matches mutants of more than one function ({}); rewrite it to one function and operator",
                    shown.join(", ")
                ),
            ));
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

pub(crate) fn check(
    repo: &Path,
    base: &str,
    head: &Head,
    workspace: &Workspace,
    packages: &[String],
) -> Result<String, String> {
    tool::require_version(repo)?;
    let short = scope::short(base);
    let floors_note = crate::floors::completeness(repo, base, head, workspace)?;
    let found = scope::configuration_at(repo, base, head, CONFIG_FILE)?;
    let head_text = scope::head_text(repo, head, CONFIG_FILE)?;
    let head_config = head_text
        .as_deref()
        .map(|text| parse(text, "at the head"))
        .transpose()?;
    if head_config.is_none() && found.is_some() {
        return Err(format!("{CONFIG_FILE} is missing at the head; restore it"));
    }
    let base_config = match &found {
        None => None,
        Some(Found::AtBase(text)) => Some(parse(text, &format!("at the base {short}"))?),
        Some(Found::Restored { last, .. }) => Some(parse(
            last,
            &format!("before its removal from the base {short}"),
        )?),
    };
    let mut entries = Vec::new();
    if let (Some(text), Some(parsed), Some(_)) = (&head_text, &head_config, &base_config) {
        let tracked: BTreeSet<String> = scope::tracked(repo, head)?.into_iter().collect();
        entries = format_rules(text, parsed, &tracked).map_err(|problems| problems.join("\n"))?;
    }
    let mut crates: BTreeSet<&str> = packages.iter().map(String::as_str).collect();
    let mut unowned = Vec::new();
    for entry in &entries {
        match workspace.owner(&entry.file) {
            Some(owner) => {
                crates.insert(owner.name.as_str());
            }
            None => unowned.push(at_line(
                entry.line,
                format!(
                    "{} belongs to no workspace crate; name a file of a crate",
                    entry.file
                ),
            )),
        }
    }
    if !unowned.is_empty() {
        return Err(unowned.join("\n"));
    }
    let shared = Config::shared();
    let listing = write_listing(repo, base_config.as_ref().unwrap_or(&shared))?;
    let mut listed = 0;
    let mut problems = Vec::new();
    let mut by_crate: BTreeMap<&str, Vec<Entry>> = BTreeMap::new();
    for entry in entries {
        if let Some(owner) = workspace.owner(&entry.file) {
            by_crate.entry(owner.name.as_str()).or_default().push(entry);
        }
    }
    for package in &crates {
        let mutants = list(repo, &listing, package)?;
        listed += mutants.len();
        if let Some(own) = by_crate.get(package)
            && let Err(found) = judge_entries(own, &mutants)
        {
            problems.extend(found);
        }
    }
    if !problems.is_empty() {
        problems.sort();
        return Err(problems.join("\n"));
    }
    let exclusions = by_crate.values().map(Vec::len).sum::<usize>();
    let listing_note = if base_config.is_none() {
        format!("not in force yet at {short}; {listed} mutants listed")
    } else {
        format!(
            "{listed} mutants listed in {} crates; {exclusions} exclusion{}",
            crates.len(),
            if exclusions == 1 { "" } else { "s" }
        )
    };
    Ok(if floors_note.is_empty() {
        listing_note
    } else {
        format!("{listing_note}; {floors_note}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEYS: &str = "test_tool = \"nextest\"\nprofile = \"mutants\"\nadditional_cargo_test_args = [\"--profile=mutants\", \"--no-tests=pass\"]\ngitignore = true\n";
    const SHARED: &str = concat!(
        "test_tool = \"nextest\"\nprofile = \"mutants\"\nadditional_cargo_test_args = [\"--profile=mutants\", \"--no-tests=pass\"]\ngitignore = true\n",
        "exclude_re = [\n]\n"
    );

    const DOUBLE: &str = r"^src/lib\.rs:\d+:\d+: replace \* with / in double$";

    fn entry_line(pattern: &str, reason: &str) -> String {
        format!("    \"{}\", # {reason}\n", pattern.replace('\\', "\\\\"))
    }

    fn with_entries(body: &str) -> String {
        format!("{KEYS}exclude_re = [\n{body}]\n")
    }

    struct Rule {
        name: &'static str,
        text: String,
        parsed_as: Option<Vec<String>>,
        rejected_naming: Vec<&'static str>,
    }

    fn rule(name: &'static str, text: String, rejected_naming: Vec<&'static str>) -> Rule {
        Rule {
            name,
            text,
            parsed_as: None,
            rejected_naming,
        }
    }

    #[test]
    fn every_exclusion_sits_on_one_line_with_a_reason() {
        let line_six = "mutants.toml:6:";
        let at = |pattern: &str| {
            with_entries(&entry_line(
                pattern,
                "equivalent: the factor keeps the sign",
            ))
        };
        let rules = vec![
            rule("a valid entry", at(DOUBLE), vec![]),
            rule(
                "no reason",
                with_entries(&format!("    \"{}\",\n", DOUBLE.replace('\\', "\\\\"))),
                vec![line_six, "reason"],
            ),
            rule(
                "an empty reason",
                with_entries(&format!("    \"{}\", #\n", DOUBLE.replace('\\', "\\\\"))),
                vec![line_six, "reason"],
            ),
            rule(
                "an entry over two lines",
                with_entries(&format!(
                    "    \"{}\",\n    # equivalent\n",
                    DOUBLE.replace('\\', "\\\\")
                )),
                vec![line_six, "reason"],
            ),
            rule(
                "an unanchored pattern",
                at(r"src/lib\.rs:\d+:\d+: replace \* with / in double"),
                vec![line_six, "anchored"],
            ),
            rule(
                "a pinned line and column",
                at(r"^src/lib\.rs:12:5: replace \* with / in double$"),
                vec![line_six, r"\d+:\d+"],
            ),
            rule(
                "a file part with an unescaped metacharacter",
                at(r"^src/lib.rs:\d+:\d+: replace \* with / in double$"),
                vec![line_six, "metacharacter"],
            ),
            rule(
                "a file that is not tracked",
                at(r"^src/other\.rs:\d+:\d+: replace \* with / in double$"),
                vec![line_six, "src/other.rs", "tracked"],
            ),
            rule(
                "an entry on the line that opens the array",
                format!("{KEYS}exclude_re = [\"a\", # r\n]\n"),
                vec!["mutants.toml:5:", "own line"],
            ),
            Rule {
                parsed_as: Some(vec!["^other$".to_owned()]),
                ..rule(
                    "a scanned list that differs from the parsed one",
                    at(DOUBLE),
                    vec!["mutants.toml:5:", "differ"],
                )
            },
        ];
        let tracked: BTreeSet<String> = ["src/lib.rs".to_owned()].into();
        for rule in rules {
            let mut parsed = parse(&rule.text, "at the head").expect(rule.name);
            if let Some(list) = rule.parsed_as {
                parsed.exclude_re = list;
            }
            let found = format_rules(&rule.text, &parsed, &tracked);
            if rule.rejected_naming.is_empty() {
                let entries =
                    found.unwrap_or_else(|problems| panic!("{}: {problems:?}", rule.name));
                assert_eq!(entries.len(), 1, "{}", rule.name);
                assert_eq!(
                    (entries[0].line, entries[0].file.as_str()),
                    (6, "src/lib.rs")
                );
                continue;
            }
            let problems = found.expect_err(rule.name).join("\n");
            for part in rule.rejected_naming {
                assert!(
                    problems.contains(part),
                    "{}: {part} not in {problems}",
                    rule.name
                );
            }
        }
    }

    #[test]
    fn the_matrix_variant_differs_from_the_base_only_in_the_test_profile() {
        let quote_heavy = r"^src/lib\.rs:\d+:\d+: replace \* with / in double$";
        let base = parse(
            &with_entries(&entry_line(
                quote_heavy,
                "equivalent: the factor keeps the sign",
            )),
            "at the base abc",
        )
        .unwrap();
        let variant = parse_shape(&matrix_text(&base)).unwrap();
        assert_eq!(
            variant.additional_cargo_test_args,
            ["--profile=mutants-matrix", "--no-tests=pass"]
        );
        assert_eq!(variant.exclude_re, [quote_heavy]);
        let rest = |c: &Config| (c.test_tool.clone(), c.profile.clone(), c.gitignore);
        assert_eq!(rest(&variant), rest(&base));
    }

    #[test]
    fn the_mutation_configuration_holds_only_the_shared_keys() {
        let with = |extra: &str| format!("{SHARED}{extra}\n");
        let swap = |from: &str, to: &str| SHARED.replace(from, to);
        let rows: Vec<(&str, String, Vec<&str>)> = vec![
            ("the shared keys", SHARED.to_owned(), vec![]),
            ("cap_lints", with("cap_lints = true"), vec!["cap_lints"]),
            (
                "exclude_globs",
                with("exclude_globs = []"),
                vec!["exclude_globs"],
            ),
            ("examine_re", with("examine_re = []"), vec!["examine_re"]),
            (
                "test_package",
                with("test_package = []"),
                vec!["test_package"],
            ),
            (
                "timeout_multiplier",
                with("timeout_multiplier = 2.0"),
                vec!["timeout_multiplier"],
            ),
            (
                "another cargo profile",
                swap("profile = \"mutants\"", "profile = \"dev\""),
                vec!["profile", "\"mutants\""],
            ),
            (
                "another test tool",
                swap("nextest", "cargo"),
                vec!["test_tool", "\"nextest\""],
            ),
            (
                "no empty-selection rule",
                swap(", \"--no-tests=pass\"", ""),
                vec!["additional_cargo_test_args", "--no-tests=pass"],
            ),
            (
                "gitignore off",
                swap("gitignore = true", "gitignore = false"),
                vec!["gitignore", "true"],
            ),
            (
                "an exclusion that is not a regular expression",
                swap("exclude_re = [\n]", "exclude_re = [\"(\"]"),
                vec!["exclude_re"],
            ),
            (
                "no exclusion key",
                swap("exclude_re = [\n]\n", ""),
                vec!["exclude_re"],
            ),
            (
                "an unparsable file",
                "test_tool = = \n".to_owned(),
                vec![CONFIG_FILE, "at the base abc"],
            ),
        ];
        for (name, text, rejected_naming) in rows {
            let parsed = parse(&text, "at the base abc");
            if rejected_naming.is_empty() {
                assert!(parsed.is_ok(), "{name}: {parsed:?}");
                continue;
            }
            let message = parsed.expect_err(name);
            for part in rejected_naming {
                assert!(message.contains(part), "{name}: {part} not in {message}");
            }
        }
    }

    mod cargo_mutants {
        use super::super::*;
        use crate::scope::tests::Repo;

        fn crate_with_a_bad_tree_copy(label: &str) -> Repo {
            let repo = Repo::new(label);
            repo.init();
            repo.write(
                "Cargo.toml",
                "[package]\nname = \"listed\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
            );
            repo.write(
                "src/lib.rs",
                "pub fn double(x: i32) -> i32 {\n    x * 2\n}\n",
            );
            repo.write(CONFIG_FILE, &format!("{}bogus_key = 1\n", super::SHARED));
            repo.commit("base");
            repo
        }

        #[test]
        fn the_listing_reads_only_the_given_config() {
            let repo = crate_with_a_bad_tree_copy("listing-config");
            let from_the_tree = std::process::Command::new("cargo")
                .args(["mutants", "--list", "-p", "listed"])
                .current_dir(&repo.0)
                .output()
                .unwrap();
            assert!(
                !from_the_tree.status.success(),
                "cargo-mutants accepted the tree's own configuration with an unknown key"
            );
            let config = parse(super::SHARED, "at the base").unwrap();
            let path = write_listing(&repo.0, &config).unwrap();
            assert!(path.starts_with(repo.0.join("target/mutation")), "{path:?}");
            let mutants = list(&repo.0, &path, "listed").unwrap();
            assert!(!mutants.is_empty());
            assert!(mutants.iter().all(|m| m.package == "listed"), "{mutants:?}");
        }

        fn listed_crate() -> Workspace {
            Workspace {
                packages: vec![crate::metadata::Package {
                    name: "listed".to_owned(),
                    dir: String::new(),
                    edition: "2024".to_owned(),
                    dependencies: Vec::new(),
                    targets: Vec::new(),
                }],
            }
        }

        const TWO_FUNCTIONS: &str = "pub fn double(x: i32) -> i32 {\n    x * 2\n}\n\npub fn triple(x: i32) -> i32 {\n    x * 3\n}\n";
        const ONE_FUNCTION: &str = "pub fn double(x: i32) -> i32 {\n    x * 2\n}\n";

        #[test]
        fn the_check_counts_mutants_with_and_without_the_base_configuration() {
            let repo = Repo::new("check-force");
            repo.init();
            repo.write(
                "Cargo.toml",
                "[package]\nname = \"listed\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
            );
            repo.write("src/lib.rs", ONE_FUNCTION);
            repo.commit("without configuration");
            let before = repo.sha();
            repo.write(CONFIG_FILE, super::SHARED);
            repo.commit("with configuration");
            let after = repo.sha();
            let packages = ["listed".to_owned()];
            let short = &before[..9];
            let workspace = listed_crate();
            let not_in_force =
                check(&repo.0, &before, &Head::Worktree, &workspace, &packages).unwrap();
            assert!(
                not_in_force.starts_with(&format!("not in force yet at {short}; "))
                    && not_in_force.ends_with(" mutants listed"),
                "{not_in_force}"
            );
            let in_force = check(&repo.0, &after, &Head::Worktree, &workspace, &packages).unwrap();
            assert!(
                in_force.ends_with(" mutants listed in 1 crates; 0 exclusions")
                    && !in_force.starts_with("0 "),
                "{in_force}"
            );
        }

        #[test]
        fn stale_and_widened_exclusions_fail() {
            let triple = r"^src/lib\.rs:\d+:\d+: replace \* with / in triple$";
            let both = r"^src/lib\.rs:\d+:\d+: replace \* with / in \w+$";
            struct Case {
                name: &'static str,
                source: &'static str,
                configuration: String,
                passes: bool,
                parts: Vec<&'static str>,
            }
            let case = |name, source, configuration, passes, parts| Case {
                name,
                source,
                configuration,
                passes,
                parts,
            };
            let cases = vec![
                case(
                    "an entry naming a function that exists",
                    TWO_FUNCTIONS,
                    super::with_entries(&super::entry_line(super::DOUBLE, "equivalent")),
                    true,
                    vec!["1 exclusion"],
                ),
                case(
                    "an entry naming a function the head deleted",
                    ONE_FUNCTION,
                    super::with_entries(&super::entry_line(triple, "equivalent")),
                    false,
                    vec!["stale", "mutants.toml:6:"],
                ),
                case(
                    "a pattern that matches both functions",
                    TWO_FUNCTIONS,
                    super::with_entries(&super::entry_line(both, "equivalent")),
                    false,
                    vec!["more than one function", "double", "triple"],
                ),
                case(
                    "a stale entry removed together with its function",
                    ONE_FUNCTION,
                    super::with_entries(""),
                    true,
                    vec!["0 exclusions"],
                ),
            ];
            for Case {
                name,
                source,
                configuration,
                passes,
                parts,
            } in cases
            {
                let repo = Repo::new("exclusions");
                repo.init();
                repo.write(
                    "Cargo.toml",
                    "[package]\nname = \"listed\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
                );
                repo.write("src/lib.rs", TWO_FUNCTIONS);
                repo.write(CONFIG_FILE, super::SHARED);
                repo.commit("base");
                let base = repo.sha();
                repo.write("src/lib.rs", source);
                repo.write(CONFIG_FILE, &configuration);
                let found = check(
                    &repo.0,
                    &base,
                    &Head::Worktree,
                    &listed_crate(),
                    &["listed".to_owned()],
                );
                let (detail, expected) = match (passes, found) {
                    (true, Ok(detail)) | (false, Err(detail)) => (detail, true),
                    (_, Ok(detail)) | (_, Err(detail)) => (detail, false),
                };
                assert!(expected, "{name}: unexpected outcome: {detail}");
                for part in parts {
                    assert!(detail.contains(part), "{name}: {part} not in {detail}");
                }
            }
        }
    }
}
