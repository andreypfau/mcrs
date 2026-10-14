use crate::floors::{self, FLOORS_FILE};
use crate::mutation::config::{self, CONFIG_FILE as MUTANTS_FILE};
use crate::scope::{self, Found, Head};
use crate::size::{self, LIMITS_FILE};
use crate::unused_deps::{self, EXEMPTIONS_FILE};
use regex::Regex;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const ATTRIBUTES_FILE: &str = ".gitattributes";
const OTHERS_SHOWN: usize = 10;
const ROOT_MANIFEST: &str = "Cargo.toml";
const CLIPPY_FILE: &str = "clippy.toml";
const CHECK_CFG: &str = "check-cfg";
const DEFAULT_TOO_MANY_LINES: u64 = 100;
const DEFAULT_TOO_MANY_ARGUMENTS: u64 = 7;
const NESTING_DISABLED: u64 = u64::MAX;
pub(crate) const LINTS_FILE: &str = "quality/lints.toml";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NoKeys {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Clones {
    pub(crate) min_tokens: u32,
    #[serde(default)]
    pub(crate) exclude: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Lints {
    pub(crate) suppressions: Option<NoKeys>,
    pub(crate) bypasses: Option<NoKeys>,
    pub(crate) clones: Option<Clones>,
}

fn parse_lints(text: &str) -> Result<Lints, String> {
    toml::from_str(text).map_err(|e| e.message().to_owned())
}

pub(crate) fn lints_at(repo: &Path, base: &str, head: &Head) -> Result<Option<Lints>, String> {
    let Some(found) = scope::configuration_at(repo, base, head, LINTS_FILE)? else {
        return Ok(None);
    };
    let reference = match found {
        Found::AtBase(text) => text,
        Found::Restored { restored, last } => {
            parse_lints(&restored)
                .map_err(|e| format!("the restored {LINTS_FILE} is malformed: {e}"))?;
            last
        }
    };
    parse_lints(&reference).map(Some).map_err(|e| {
        format!(
            "{LINTS_FILE} at the base {} is malformed: {e}; fix it on the base branch",
            scope::short(base)
        )
    })
}

#[derive(Deserialize)]
pub(crate) struct Manifest {
    pub(crate) lints: Option<toml::Table>,
    pub(crate) workspace: Option<WorkspaceTable>,
}

#[derive(Deserialize, Default)]
pub(crate) struct WorkspaceTable {
    #[serde(default)]
    pub(crate) members: Vec<String>,
    #[serde(default)]
    pub(crate) exclude: Vec<String>,
    lints: Option<LintTable>,
}

type LintTable = BTreeMap<String, BTreeMap<String, Setting>>;

#[derive(Deserialize)]
#[serde(untagged)]
enum Setting {
    Level(String),
    Table(BTreeMap<String, toml::Value>),
}

impl Setting {
    fn level(&self) -> Option<&str> {
        match self {
            Setting::Level(level) => Some(level),
            Setting::Table(table) => table.get("level").and_then(toml::Value::as_str),
        }
    }

    fn extras(&self) -> BTreeMap<&str, &toml::Value> {
        match self {
            Setting::Level(_) => BTreeMap::new(),
            Setting::Table(table) => table
                .iter()
                .filter(|(key, _)| key.as_str() != "level")
                .map(|(key, value)| (key.as_str(), value))
                .collect(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct ClippyConfig {
    too_many_lines_threshold: Option<u64>,
    too_many_arguments_threshold: Option<u64>,
    excessive_nesting_threshold: Option<u64>,
    disallowed_types: Option<Vec<Disallowed>>,
    disallowed_macros: Option<Vec<Disallowed>>,
    disallowed_methods: Option<Vec<Disallowed>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Disallowed {
    Path(String),
    Entry(DisallowedEntry),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DisallowedEntry {
    path: String,
    #[serde(rename = "reason")]
    _reason: Option<String>,
}

impl Disallowed {
    fn path(&self) -> &str {
        match self {
            Disallowed::Path(path) => path,
            Disallowed::Entry(entry) => &entry.path,
        }
    }
}

fn merge(directions: impl IntoIterator<Item = Direction>) -> Direction {
    directions
        .into_iter()
        .fold(Direction::Unchanged, |sum, next| {
            if sum == Direction::Loosening || next == Direction::Loosening {
                Direction::Loosening
            } else if sum == Direction::Tightening || next == Direction::Tightening {
                Direction::Tightening
            } else {
                Direction::Unchanged
            }
        })
}

fn by_order<T: Ord>(before: T, after: T) -> Direction {
    match after.cmp(&before) {
        std::cmp::Ordering::Greater => Direction::Loosening,
        std::cmp::Ordering::Equal => Direction::Unchanged,
        std::cmp::Ordering::Less => Direction::Tightening,
    }
}

fn by_set<T: Ord>(before: &BTreeSet<T>, after: &BTreeSet<T>) -> Direction {
    if !after.is_subset(before) {
        Direction::Loosening
    } else if after.len() < before.len() {
        Direction::Tightening
    } else {
        Direction::Unchanged
    }
}

pub(crate) fn parse_manifest(text: &str) -> Result<Manifest, String> {
    toml::from_str(text).map_err(|e| e.message().to_owned())
}

pub(crate) fn inherits_workspace(lints: Option<&toml::Table>) -> bool {
    lints.is_some_and(|table| {
        table.len() == 1 && table.get("workspace") == Some(&toml::Value::Boolean(true))
    })
}

fn package_lints_direction(base: Option<&toml::Table>, head: Option<&toml::Table>) -> Direction {
    if inherits_workspace(head) {
        if inherits_workspace(base) {
            Direction::Unchanged
        } else {
            Direction::Tightening
        }
    } else if base == head {
        Direction::Unchanged
    } else {
        Direction::Loosening
    }
}

fn level_direction(before: Option<&str>, after: Option<&str>) -> Direction {
    if before == after {
        return Direction::Unchanged;
    }
    let rank = |level: Option<&str>| match level {
        Some("allow") => Some(0),
        Some("warn" | "deny" | "forbid") => Some(1),
        _ => None,
    };
    match (after, rank(after)) {
        (Some("deny" | "forbid"), _) | (_, None) => Direction::Loosening,
        (_, Some(after)) => by_order(after, rank(before).unwrap_or(1)),
    }
}

fn check_cfg_entries(value: Option<&toml::Value>) -> Option<BTreeSet<&str>> {
    match value {
        None => Some(BTreeSet::new()),
        Some(toml::Value::Array(items)) => items.iter().map(toml::Value::as_str).collect(),
        Some(_) => None,
    }
}

fn setting_direction(base: &Setting, head: &Setting) -> Direction {
    let (mut before, mut after) = (base.extras(), head.extras());
    let (cfgs_before, cfgs_after) = (before.remove(CHECK_CFG), after.remove(CHECK_CFG));
    let cfgs = match (
        check_cfg_entries(cfgs_before),
        check_cfg_entries(cfgs_after),
    ) {
        (Some(before), Some(after)) => by_set(&before, &after),
        _ if cfgs_before == cfgs_after => Direction::Unchanged,
        _ => Direction::Loosening,
    };
    let others = if before == after {
        Direction::Unchanged
    } else {
        Direction::Loosening
    };
    merge([level_direction(base.level(), head.level()), others, cfgs])
}

fn added_direction(head: &Setting) -> Direction {
    if head.level() == Some("warn") && head.extras().is_empty() {
        Direction::Tightening
    } else {
        Direction::Loosening
    }
}

fn flatten(table: &LintTable) -> BTreeMap<String, &Setting> {
    table
        .iter()
        .flat_map(|(tool, lints)| {
            lints
                .iter()
                .map(move |(lint, setting)| (format!("{tool}.{lint}"), setting))
        })
        .collect()
}

fn lint_table_changes(
    base: Option<&LintTable>,
    head: Option<&LintTable>,
) -> Vec<(Direction, String)> {
    let Some(head) = head else {
        return if base.is_some() {
            vec![(
                Direction::Loosening,
                "the workspace lint table was removed".to_owned(),
            )]
        } else {
            Vec::new()
        };
    };
    let (before, after) = (base.map(flatten).unwrap_or_default(), flatten(head));
    let names: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    names
        .into_iter()
        .map(|name| match (before.get(name), after.get(name)) {
            (Some(base), Some(head)) => (setting_direction(base, head), format!("{name} changed")),
            (Some(_), None) => (Direction::Loosening, format!("{name} removed")),
            (None, Some(head)) => (added_direction(head), format!("{name} added")),
            (None, None) => (Direction::Unchanged, String::new()),
        })
        .collect()
}

fn workspace_lints(manifest: &Manifest) -> Option<&LintTable> {
    manifest.workspace.as_ref()?.lints.as_ref()
}

fn root_changes(sides: &Sides) -> Result<Vec<(Direction, String)>, String> {
    let Some(base) = sides.base else {
        return Ok(Vec::new());
    };
    let base = parse_manifest(base)
        .map_err(|e| format!("{ROOT_MANIFEST} at the base is malformed: {e}"))?;
    let Some(head) = sides.head.and_then(|text| parse_manifest(text).ok()) else {
        return Ok(vec![(
            Direction::Loosening,
            "the manifest is missing or cannot be parsed".to_owned(),
        )]);
    };
    let mut changes = lint_table_changes(workspace_lints(&base), workspace_lints(&head));
    let what = if inherits_workspace(base.lints.as_ref()) {
        "the root package no longer inherits the workspace lints"
    } else {
        "the root package's own lints changed"
    };
    changes.push((
        package_lints_direction(base.lints.as_ref(), head.lints.as_ref()),
        what.to_owned(),
    ));
    Ok(changes)
}

fn root_manifest_direction(sides: &Sides) -> Result<Direction, String> {
    Ok(merge(
        root_changes(sides)?
            .into_iter()
            .map(|(direction, _)| direction),
    ))
}

fn root_loosenings(sides: &Sides) -> Result<Vec<String>, String> {
    Ok(root_changes(sides)?
        .into_iter()
        .filter(|(direction, _)| *direction == Direction::Loosening)
        .map(|(_, what)| what)
        .collect())
}

struct MemberSet {
    include: Vec<String>,
    exclude: Vec<String>,
}

struct Members {
    sets: Vec<MemberSet>,
    tracked: Vec<String>,
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
            "the workspace member pattern `{pattern}` uses glob syntax beyond `*`, which gate-config cannot match"
        ));
    }
    let segments: Vec<&str> = dir.split('/').collect();
    Ok(patterns.len() == segments.len()
        && patterns
            .iter()
            .zip(&segments)
            .all(|(pattern, segment)| segment_matches(pattern, segment)))
}

pub(crate) fn any_glob_matches(patterns: &[String], dir: &str) -> Result<bool, String> {
    for pattern in patterns {
        if glob_matches(pattern, dir)? {
            return Ok(true);
        }
    }
    Ok(false)
}

impl Members {
    fn read(repo: &Path, base: &str, head: &Head) -> Result<Members, String> {
        let texts = [
            scope::read_at(repo, base, ROOT_MANIFEST)?,
            scope::head_text(repo, head, ROOT_MANIFEST)?,
        ];
        let sets = texts
            .iter()
            .flatten()
            .filter_map(|text| parse_manifest(text).ok())
            .filter_map(|manifest| manifest.workspace)
            .map(|workspace| MemberSet {
                include: workspace.members,
                exclude: workspace.exclude,
            })
            .collect();
        Ok(Members {
            sets,
            tracked: scope::tracked(repo, head)?,
        })
    }

    fn contains(&self, dir: &str) -> Result<bool, String> {
        for set in &self.sets {
            if any_glob_matches(&set.include, dir)? && !any_glob_matches(&set.exclude, dir)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn holds_files(&self, dir: &str) -> bool {
        let prefix = format!("{dir}/");
        self.tracked.iter().any(|path| path.starts_with(&prefix))
    }
}

fn member_dir(path: &str) -> Option<&str> {
    path.strip_suffix("/Cargo.toml")
}

fn member_manifest_direction(sides: &Sides) -> Result<Direction, String> {
    let (Some(members), Some(dir)) = (sides.members, member_dir(sides.path)) else {
        return Err(format!(
            "the members of the head are needed to judge {}",
            sides.path
        ));
    };
    if !members.contains(dir)? {
        return Ok(Direction::Unchanged);
    }
    let Some(head) = sides.head else {
        return Ok(if members.holds_files(dir) {
            Direction::Loosening
        } else {
            Direction::Unchanged
        });
    };
    let Ok(head) = parse_manifest(head) else {
        return Ok(Direction::Loosening);
    };
    let Some(base) = sides.base else {
        return Ok(if inherits_workspace(head.lints.as_ref()) {
            Direction::Unchanged
        } else {
            Direction::Loosening
        });
    };
    let base = parse_manifest(base)
        .map_err(|e| format!("{} at the base is malformed: {e}", sides.path))?;
    Ok(package_lints_direction(
        base.lints.as_ref(),
        head.lints.as_ref(),
    ))
}

fn lints_only_change(base: &str, head: &str, root: bool) -> bool {
    let (Ok(mut before), Ok(mut after)) = (
        toml::from_str::<toml::Table>(base),
        toml::from_str::<toml::Table>(head),
    ) else {
        return false;
    };
    let strip = |table: &mut toml::Table| {
        let mut taken = vec![table.remove("lints")];
        if root {
            if let Some(toml::Value::Table(package)) = table.get_mut("package") {
                package.remove("version");
            }
            if let Some(toml::Value::Table(workspace)) = table.get_mut("workspace") {
                taken.push(workspace.remove("lints"));
                if let Some(toml::Value::Table(package)) = workspace.get_mut("package") {
                    package.remove("version");
                }
            }
        }
        taken
    };
    let (lints_before, lints_after) = (strip(&mut before), strip(&mut after));
    before == after && lints_before != lints_after
}

fn is_configuration_only(sides: &Sides) -> Result<bool, String> {
    let (Some(base), Some(head)) = (sides.base, sides.head) else {
        return Ok(false);
    };
    if sides.path == ROOT_MANIFEST {
        return Ok(scope::manifest_version_only(base, head).is_some()
            || lints_only_change(base, head, true));
    }
    let (Some(members), Some(dir)) = (sides.members, member_dir(sides.path)) else {
        return Ok(false);
    };
    Ok(members.contains(dir)? && lints_only_change(base, head, false))
}

fn effective_nesting(threshold: Option<u64>) -> Option<u64> {
    threshold.map(|n| if n == 0 { NESTING_DISABLED } else { n })
}

fn threshold_direction(base: Option<u64>, head: Option<u64>, default: u64) -> Direction {
    match (base, head) {
        (Some(_), None) => Direction::Loosening,
        (_, None) => Direction::Unchanged,
        (before, Some(after)) => by_order(before.unwrap_or(default), after),
    }
}

fn paths_direction(base: Option<&[Disallowed]>, head: Option<&[Disallowed]>) -> Direction {
    let paths = |list: &[Disallowed]| -> BTreeSet<String> {
        list.iter().map(|entry| entry.path().to_owned()).collect()
    };
    match (base, head) {
        (Some(_), None) => Direction::Loosening,
        (_, None) => Direction::Unchanged,
        (before, Some(after)) => {
            let (before, after) = (paths(before.unwrap_or_default()), paths(after));
            if !before.is_subset(&after) {
                Direction::Loosening
            } else if after.len() > before.len() {
                Direction::Tightening
            } else {
                Direction::Unchanged
            }
        }
    }
}

fn clippy_direction(sides: &Sides) -> Result<Direction, String> {
    let Some(base) = sides.base else {
        return Ok(Direction::Unchanged);
    };
    let base: ClippyConfig = toml::from_str(base)
        .map_err(|e| format!("{CLIPPY_FILE} at the base is malformed: {}", e.message()))?;
    let Some(head) = sides
        .head
        .and_then(|text| toml::from_str::<ClippyConfig>(text).ok())
    else {
        return Ok(Direction::Loosening);
    };
    Ok(merge([
        threshold_direction(
            base.too_many_lines_threshold,
            head.too_many_lines_threshold,
            DEFAULT_TOO_MANY_LINES,
        ),
        threshold_direction(
            base.too_many_arguments_threshold,
            head.too_many_arguments_threshold,
            DEFAULT_TOO_MANY_ARGUMENTS,
        ),
        threshold_direction(
            effective_nesting(base.excessive_nesting_threshold),
            effective_nesting(head.excessive_nesting_threshold),
            NESTING_DISABLED,
        ),
        paths_direction(
            base.disallowed_types.as_deref(),
            head.disallowed_types.as_deref(),
        ),
        paths_direction(
            base.disallowed_macros.as_deref(),
            head.disallowed_macros.as_deref(),
        ),
        paths_direction(
            base.disallowed_methods.as_deref(),
            head.disallowed_methods.as_deref(),
        ),
    ]))
}

fn presence_direction(before: bool, after: bool) -> Direction {
    match (before, after) {
        (true, false) => Direction::Loosening,
        (false, true) => Direction::Tightening,
        _ => Direction::Unchanged,
    }
}

fn clones_direction(base: Option<&Clones>, head: Option<&Clones>) -> Direction {
    match (base, head) {
        (Some(before), Some(after)) => {
            let globs =
                |clones: &Clones| -> BTreeSet<String> { clones.exclude.iter().cloned().collect() };
            merge([
                by_order(before.min_tokens, after.min_tokens),
                by_set(&globs(before), &globs(after)),
            ])
        }
        (before, after) => presence_direction(before.is_some(), after.is_some()),
    }
}

fn lints_file_direction(sides: &Sides) -> Result<Direction, String> {
    let head = sides
        .head
        .map(|text| {
            parse_lints(text).map_err(|e| format!("{LINTS_FILE} at the head is malformed: {e}"))
        })
        .transpose()?;
    let Some(base) = sides.lints else {
        return Ok(Direction::Unchanged);
    };
    let Some(head) = head else {
        return Ok(Direction::Loosening);
    };
    Ok(merge([
        presence_direction(base.suppressions.is_some(), head.suppressions.is_some()),
        presence_direction(base.bypasses.is_some(), head.bypasses.is_some()),
        clones_direction(base.clones.as_ref(), head.clones.as_ref()),
    ]))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Direction {
    Loosening,
    Unchanged,
    Tightening,
}

struct Sides<'a> {
    path: &'a str,
    base: Option<&'a str>,
    head: Option<&'a str>,
    packages: Option<&'a BTreeSet<String>>,
    members: Option<&'a Members>,
    lints: Option<&'a Lints>,
}

#[derive(Clone, Copy)]
enum Matcher {
    File(&'static str),
    Attributes,
    RootManifest,
    MemberManifest,
    Quality,
}

impl Matcher {
    fn matches(self, path: &str) -> bool {
        match self {
            Matcher::File(file) => path == file,
            Matcher::Attributes => path.rsplit('/').next() == Some(ATTRIBUTES_FILE),
            Matcher::RootManifest => path == ROOT_MANIFEST,
            Matcher::MemberManifest => member_dir(path).is_some(),
            Matcher::Quality => path.starts_with("quality/"),
        }
    }
}

type Judge = fn(&Sides) -> Result<Direction, String>;

const DIRECTIONS: [(Matcher, Judge); 10] = [
    (Matcher::File(LIMITS_FILE), limits_direction),
    (Matcher::Attributes, attributes_direction),
    (Matcher::File(EXEMPTIONS_FILE), exemptions_direction),
    (Matcher::File(MUTANTS_FILE), mutants_direction),
    (Matcher::File(FLOORS_FILE), floors_direction),
    (Matcher::RootManifest, root_manifest_direction),
    (Matcher::MemberManifest, member_manifest_direction),
    (Matcher::File(CLIPPY_FILE), clippy_direction),
    (Matcher::File(LINTS_FILE), lints_file_direction),
    (Matcher::Quality, quality_direction),
];

fn is_manifest(path: &str) -> bool {
    Matcher::RootManifest.matches(path) || Matcher::MemberManifest.matches(path)
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

fn mutants_direction(sides: &Sides) -> Result<Direction, String> {
    let Some(base) = sides.base else {
        return Ok(Direction::Unchanged);
    };
    let base = config::parse_shape(base)
        .map_err(|e| format!("{MUTANTS_FILE} at the base is malformed: {e}"))?;
    let Some(head) = sides.head.and_then(|text| config::parse_shape(text).ok()) else {
        return Ok(Direction::Loosening);
    };
    if !head.shares_keys_with(&base) {
        return Ok(Direction::Loosening);
    }
    let (before, after) = (base.exclusions(), head.exclusions());
    Ok(if !after.is_subset(&before) {
        Direction::Loosening
    } else if after.len() < before.len() {
        Direction::Tightening
    } else {
        Direction::Unchanged
    })
}

fn floors_direction(sides: &Sides) -> Result<Direction, String> {
    let Some(base) = sides.base else {
        return Ok(Direction::Unchanged);
    };
    let base =
        floors::parse(base).map_err(|e| format!("{FLOORS_FILE} at the base is malformed: {e}"))?;
    let Some(head) = sides.head.and_then(|text| floors::parse(text).ok()) else {
        return Ok(Direction::Loosening);
    };
    let packages = sides
        .packages
        .ok_or_else(|| format!("the packages of the head are needed to judge {FLOORS_FILE}"))?;
    let judged = floors::judge(&base, &head, packages);
    Ok(if !judged.problems.is_empty() {
        Direction::Loosening
    } else if !judged.tightened.is_empty() {
        Direction::Tightening
    } else {
        Direction::Unchanged
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
    let packages = if changed.iter().any(|path| path == FLOORS_FILE) {
        Some(floors::head_packages(repo, head)?)
    } else {
        None
    };
    let members = if changed
        .iter()
        .any(|path| Matcher::MemberManifest.matches(path))
    {
        Some(Members::read(repo, base, head)?)
    } else {
        None
    };
    let lints = if changed.iter().any(|path| path == LINTS_FILE) {
        lints_at(repo, base, head)?
    } else {
        None
    };
    let mut named = Vec::new();
    for path in &changed {
        if let Some((_, judge)) = DIRECTIONS.iter().find(|(matcher, _)| matcher.matches(path)) {
            let before = if path == LIMITS_FILE {
                Some(enforced.reference.clone())
            } else {
                scope::read_at(repo, base, path)?
            };
            let after = scope::head_text(repo, head, path)?;
            let sides = Sides {
                path,
                base: before.as_deref(),
                head: after.as_deref(),
                packages: packages.as_ref(),
                members: members.as_ref(),
                lints: lints.as_ref(),
            };
            match judge(&sides)? {
                Direction::Loosening => {
                    loosening.insert(path.as_str());
                    if Matcher::RootManifest.matches(path) {
                        named.extend(root_loosenings(&sides)?);
                    }
                }
                Direction::Tightening => tightened = true,
                Direction::Unchanged => {}
            }
            if is_manifest(path) && !is_configuration_only(&sides)? {
                others.insert(path.as_str());
            }
        } else if path != "Cargo.lock" {
            others.insert(path.as_str());
        }
    }
    let attribute_files: Vec<&str> = changed
        .iter()
        .map(String::as_str)
        .filter(|path| Matcher::Attributes.matches(path))
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
    let named = if named.is_empty() {
        String::new()
    } else {
        format!(" ({})", named.join("; "))
    };
    Err(format!(
        "gate configuration is loosened in {}{named} beside other changes ({}{tail}); ship the loosened gate configuration in a pull request of its own; only the version commit may sit beside it",
        loosening.into_iter().collect::<Vec<_>>().join(", "),
        shown.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::floors::{
        FLOORS_FILE,
        tests::{measured, unmeasured},
    };
    use crate::scope::{
        resolve_base,
        tests::{Repo, judge_rows_in_parallel},
    };
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
        LintsRemoved,
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
    const MUTANTS: &str = crate::mutation::config::CONFIG_FILE;

    fn mutants_file(changed_key: &str, entries: &[(&str, &str)]) -> String {
        let mut text = String::from(
            "test_tool = \"nextest\"\nprofile = \"mutants\"\nadditional_cargo_test_args = [\"--profile=mutants\", \"--no-tests=pass\"]\n",
        );
        text.push_str(if changed_key.is_empty() {
            "gitignore = true\n"
        } else {
            changed_key
        });
        text.push_str("exclude_re = [\n");
        for (pattern, reason) in entries {
            text.push_str(&format!("    \"{pattern}\", # {reason}\n"));
        }
        text.push_str("]\n");
        text
    }

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

    fn at(name: &str, floor: &str, counts: (u64, u64, u64)) -> String {
        measured(name, floor, counts).text()
    }

    fn floor_edits(entries: &[String]) -> Vec<Edit> {
        vec![put(FLOORS_FILE, &entries.concat())]
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

    #[expect(
        clippy::too_many_lines,
        reason = "a table of gate-configuration changes with their expected outcome, one row per rule"
    )]
    fn rows() -> Vec<Row> {
        let raise = || put(LIMITS_FILE, LIMIT_200);
        let loosened = Expect::Pass("gate configuration loosened alone");
        let shipped_alone = "pull request of its own";
        let one = || pairs(&[("c", "b")], "needed");
        let two = || pairs(&[("c", "b"), ("a", "b")], "needed");
        let exemptions = |base: String| vec![put(EXEMPTIONS, &base)];
        let mut all = vec![
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
            Row {
                base_edits: vec![put(MUTANTS, &mutants_file("", &[("^a$", "equivalent")]))],
                ..row(
                    "an added mutation exclusion beside a crate file fails",
                    vec![
                        put(
                            MUTANTS,
                            &mutants_file("", &[("^a$", "equivalent"), ("^b$", "equivalent")]),
                        ),
                        crate_change(),
                    ],
                    Expect::Fail(shipped_alone),
                )
            },
            Row {
                base_edits: vec![put(MUTANTS, &mutants_file("", &[("^a$", "equivalent")]))],
                ..row(
                    "an added mutation exclusion alone passes",
                    vec![put(
                        MUTANTS,
                        &mutants_file("", &[("^a$", "equivalent"), ("^b$", "equivalent")]),
                    )],
                    Expect::Pass("gate configuration loosened alone"),
                )
            },
            Row {
                base_edits: vec![put(MUTANTS, &mutants_file("", &[("^a$", "equivalent")]))],
                ..row(
                    "a widened mutation exclusion pattern beside a crate file fails",
                    vec![
                        put(MUTANTS, &mutants_file("", &[("^a.*$", "equivalent")])),
                        crate_change(),
                    ],
                    Expect::Fail(shipped_alone),
                )
            },
            Row {
                base_edits: vec![put(
                    MUTANTS,
                    &mutants_file("", &[("^a$", "equivalent"), ("^b$", "equivalent")]),
                )],
                ..row(
                    "a removed mutation exclusion beside a crate file passes",
                    vec![
                        put(MUTANTS, &mutants_file("", &[("^a$", "equivalent")])),
                        crate_change(),
                    ],
                    Expect::Pass("gate configuration tightened"),
                )
            },
            Row {
                base_edits: vec![put(MUTANTS, &mutants_file("", &[("^a$", "equivalent")]))],
                ..row(
                    "a reworded mutation exclusion reason beside a crate file passes",
                    vec![
                        put(
                            MUTANTS,
                            &mutants_file("", &[("^a$", "the sign cannot change")]),
                        ),
                        crate_change(),
                    ],
                    Expect::Pass("no gate change"),
                )
            },
            Row {
                base_edits: vec![put(MUTANTS, &mutants_file("", &[]))],
                ..row(
                    "a changed mutation configuration key beside a crate file fails",
                    vec![
                        put(MUTANTS, &mutants_file("gitignore = false\n", &[])),
                        crate_change(),
                    ],
                    Expect::Fail(shipped_alone),
                )
            },
            Row {
                base_edits: vec![put(MUTANTS, &mutants_file("", &[]))],
                ..row(
                    "a deleted mutation configuration beside a crate file fails",
                    vec![del(MUTANTS), crate_change()],
                    Expect::Fail(shipped_alone),
                )
            },
            Row {
                base_edits: vec![put(MUTANTS, &mutants_file("", &[]))],
                ..row(
                    "an unparsable mutation configuration beside a crate file fails",
                    vec![put(MUTANTS, "not toml ["), crate_change()],
                    Expect::Fail(shipped_alone),
                )
            },
            row(
                "a mutation configuration absent from the base is an introduction beside a crate file",
                vec![
                    put(MUTANTS, &mutants_file("", &[("^a$", "equivalent")])),
                    crate_change(),
                ],
                Expect::Pass("no gate change"),
            ),
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
            Row {
                base_edits: floor_edits(&[unmeasured("mcrs")]),
                ..row(
                    "a floor enrolment beside a crate file passes",
                    floor_edits(&[at("mcrs", "0.83", (83, 0, 17))])
                        .into_iter()
                        .chain([crate_change()])
                        .collect(),
                    Expect::Pass("gate configuration tightened"),
                )
            },
            Row {
                base_edits: floor_edits(&[at("mcrs", "0.83", (83, 0, 17))]),
                ..row(
                    "a floor raise beside a crate file passes",
                    floor_edits(&[at("mcrs", "0.84", (84, 0, 16))])
                        .into_iter()
                        .chain([crate_change()])
                        .collect(),
                    Expect::Pass("gate configuration tightened"),
                )
            },
            Row {
                base_edits: floor_edits(&[at("mcrs", "0.83", (83, 0, 17))]),
                ..row(
                    "a lowered floor beside a crate file fails",
                    floor_edits(&[at("mcrs", "0.80", (80, 0, 20))])
                        .into_iter()
                        .chain([crate_change()])
                        .collect(),
                    Expect::Fail(shipped_alone),
                )
            },
            Row {
                base_edits: floor_edits(&[at("mcrs", "0.83", (83, 0, 17))]),
                ..row(
                    "a lowered floor alone passes the gate configuration",
                    floor_edits(&[at("mcrs", "0.80", (80, 0, 20))]),
                    Expect::Pass("gate configuration loosened alone"),
                )
            },
            Row {
                base_edits: floor_edits(&[at("mcrs", "0.83", (83, 0, 17))]),
                ..row(
                    "a dropped provenance field beside a crate file fails",
                    floor_edits(
                        &[at("mcrs", "0.83", (83, 0, 17)).replace("commit = \"c1\"\n", "")],
                    )
                    .into_iter()
                    .chain([crate_change()])
                    .collect(),
                    Expect::Fail(shipped_alone),
                )
            },
            Row {
                base_edits: floor_edits(&[at("mcrs", "0.83", (83, 0, 17))]),
                ..row(
                    "a changed provenance with the same floor beside a crate file fails",
                    floor_edits(&[
                        at("mcrs", "0.83", (83, 0, 17)).replace("2026-11-02", "2026-12-01")
                    ])
                    .into_iter()
                    .chain([crate_change()])
                    .collect(),
                    Expect::Fail(shipped_alone),
                )
            },
            Row {
                base_edits: floor_edits(&[at("mcrs", "0.83", (83, 0, 17))]),
                ..row(
                    "a number turned into unmeasured beside a crate file fails",
                    floor_edits(&[unmeasured("mcrs")])
                        .into_iter()
                        .chain([crate_change()])
                        .collect(),
                    Expect::Fail(shipped_alone),
                )
            },
            Row {
                base_edits: floor_edits(&[unmeasured("mcrs"), at("gone", "0.83", (83, 0, 17))]),
                ..row(
                    "a removed numeric entry of a package gone from the head passes",
                    floor_edits(&[unmeasured("mcrs")])
                        .into_iter()
                        .chain([crate_change()])
                        .collect(),
                    Expect::Pass("no gate change"),
                )
            },
            Row {
                base_edits: floor_edits(&[at("mcrs", "0.83", (83, 0, 17))]),
                ..row(
                    "a removed numeric entry of a live package beside a crate file fails",
                    floor_edits(&[unmeasured("other")])
                        .into_iter()
                        .chain([crate_change()])
                        .collect(),
                    Expect::Fail(shipped_alone),
                )
            },
            Row {
                base_edits: floor_edits(&[unmeasured("mcrs")]),
                ..row(
                    "an added unmeasured entry beside a crate file passes",
                    floor_edits(&[unmeasured("mcrs"), unmeasured("other")])
                        .into_iter()
                        .chain([crate_change()])
                        .collect(),
                    Expect::Pass("no gate change"),
                )
            },
            row(
                "a floors file absent from the base is an introduction beside a crate file",
                floor_edits(&[at("mcrs", "0.83", (83, 0, 17))])
                    .into_iter()
                    .chain([crate_change()])
                    .collect(),
                Expect::Pass("no gate change"),
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
        ];
        all.extend(lint_table_rows());
        all.extend(root_manifest_rows());
        all.extend(member_rows());
        all.extend(clippy_rows());
        all.extend(lints_file_rows());
        all
    }

    const INHERITED: &str = "[lints]\nworkspace = true\n";
    const MEMBER_MANIFEST: &str = "crates/c/Cargo.toml";
    const MEMBER_SOURCE: &str = "crates/c/src/lib.rs";
    const CLIPPY: &str = CLIPPY_FILE;
    const LINTS_ALL: &str =
        "[suppressions]\n\n[bypasses]\n\n[clones]\nmin_tokens = 70\nexclude = [\"crates/a/**\"]\n";
    const LISTS: &str = "disallowed-types = [\"std::collections::HashMap\"]\n\
        disallowed-macros = [\"std::println\"]\n\
        disallowed-methods = [{ path = \"std::env::var\", reason = \"configuration comes through the plugin\" }]\n";

    fn row_on(name: &'static str, base_edits: Vec<Edit>, edits: Vec<Edit>, gate: Expect) -> Row {
        Row {
            base_edits,
            ..row(name, edits, gate)
        }
    }

    fn root_with(version: &str, tables: &str) -> Edit {
        put(ROOT_MANIFEST, &format!("{}\n{tables}", manifest(version)))
    }

    fn clippy_lints(body: &str) -> String {
        format!("[workspace.lints.clippy]\n{body}")
    }

    fn member(tables: &str) -> String {
        format!("[package]\nname = \"c\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n{tables}")
    }

    fn existing_member(tables: &str) -> Vec<Edit> {
        vec![
            put(MEMBER_MANIFEST, &member(tables)),
            put(MEMBER_SOURCE, "fn c() {}\n"),
        ]
    }

    fn lint_table_rows() -> Vec<Row> {
        let two = "too_many_lines = \"warn\"\ndbg_macro = \"warn\"\n";
        let one = "dbg_macro = \"warn\"\n";
        let on = |body: &str| vec![root_with("0.5.1", &clippy_lints(body))];
        let head = |body: &str| root_with("0.5.1", &clippy_lints(body));
        let cfgs = |entries: &str| {
            root_with(
                "0.5.1",
                &format!(
                    "[workspace.lints.rust]\nunexpected_cfgs = {{ level = \"warn\", check-cfg = [{entries}] }}\n"
                ),
            )
        };
        let tightened = Expect::Pass("gate configuration tightened");
        let ordinary = Expect::Pass("no gate change");
        vec![
            row_on(
                "a lint removed from the workspace table beside a crate file fails",
                on(two),
                vec![head(one), crate_change()],
                Expect::Fail("Cargo.toml (clippy.too_many_lines removed)"),
            ),
            row_on(
                "a lint removed from the workspace table alone passes",
                on(two),
                vec![head(one)],
                Expect::Pass("gate configuration loosened alone"),
            ),
            row_on(
                "a lint added at warn beside a crate file passes",
                on(one),
                vec![head(two), crate_change()],
                tightened,
            ),
            row_on(
                "a lint lowered to allow beside a crate file fails",
                on("too_many_lines = \"warn\"\n"),
                vec![head("too_many_lines = \"allow\"\n"), crate_change()],
                Expect::Fail("clippy.too_many_lines changed"),
            ),
            row_on(
                "a lint raised from allow to warn beside a crate file passes",
                on("too_many_lines = \"allow\"\n"),
                vec![head("too_many_lines = \"warn\"\n"), crate_change()],
                Expect::Pass("gate configuration tightened"),
            ),
            row_on(
                "a lint raised to deny beside a crate file fails",
                on("too_many_lines = \"warn\"\n"),
                vec![head("too_many_lines = \"deny\"\n"), crate_change()],
                Expect::Fail("clippy.too_many_lines changed"),
            ),
            row_on(
                "a priority added to a lint beside a crate file fails",
                on("too_many_lines = \"warn\"\n"),
                vec![
                    head("too_many_lines = { level = \"warn\", priority = -1 }\n"),
                    crate_change(),
                ],
                Expect::Fail("clippy.too_many_lines changed"),
            ),
            row_on(
                "a level written as a table is no change",
                on("too_many_lines = \"warn\"\n"),
                vec![
                    head("too_many_lines = { level = \"warn\" }\n"),
                    crate_change(),
                ],
                ordinary,
            ),
            row_on(
                "a lint added at allow beside a crate file fails",
                on(two),
                vec![head(&format!("{two}todo = \"allow\"\n")), crate_change()],
                Expect::Fail("clippy.todo added"),
            ),
            row_on(
                "a check-cfg entry added beside a crate file fails",
                vec![cfgs("'cfg(a)'")],
                vec![cfgs("'cfg(a)', 'cfg(b)'"), crate_change()],
                Expect::Fail("rust.unexpected_cfgs changed"),
            ),
            row_on(
                "a check-cfg entry removed beside a crate file passes",
                vec![cfgs("'cfg(a)', 'cfg(b)'")],
                vec![cfgs("'cfg(a)'"), crate_change()],
                Expect::Pass("gate configuration tightened"),
            ),
        ]
    }

    fn root_manifest_rows() -> Vec<Row> {
        let two = "too_many_lines = \"warn\"\ndbg_macro = \"warn\"\n";
        let one = "dbg_macro = \"warn\"\n";
        let on = |body: &str| vec![root_with("0.5.1", &clippy_lints(body))];
        vec![
            row_on(
                "the workspace lint table removed beside a crate file fails",
                on(two),
                vec![put(ROOT_MANIFEST, &manifest("0.5.1")), crate_change()],
                Expect::Fail("Cargo.toml (the workspace lint table was removed)"),
            ),
            row_on(
                "a root package that stops inheriting the table beside a crate file fails",
                vec![root_with("0.5.1", INHERITED)],
                vec![put(ROOT_MANIFEST, &manifest("0.5.1")), crate_change()],
                Expect::Fail("the root package no longer inherits the workspace lints"),
            ),
            row_on(
                "a root package that loosens its own lints beside a crate file fails",
                vec![root_with("0.5.1", "[lints.clippy]\ndbg_macro = \"warn\"\n")],
                vec![
                    root_with("0.5.1", "[lints.clippy]\ndbg_macro = \"allow\"\n"),
                    crate_change(),
                ],
                Expect::Fail("the root package's own lints changed"),
            ),
            row_on(
                "a root package that starts inheriting the table beside a crate file passes",
                Vec::new(),
                vec![root_with("0.5.1", INHERITED), crate_change()],
                Expect::Pass("gate configuration tightened"),
            ),
            row_on(
                "a lint removed together with a dependency change fails",
                vec![root_with(
                    "0.5.1",
                    &format!(
                        "[workspace.dependencies]\nx = \"1\"\n\n{}",
                        clippy_lints(two)
                    ),
                )],
                vec![root_with(
                    "0.5.1",
                    &format!(
                        "[workspace.dependencies]\nx = \"2\"\n\n{}",
                        clippy_lints(one)
                    ),
                )],
                Expect::Fail("Cargo.toml (clippy.too_many_lines removed)"),
            ),
            row_on(
                "a lint removed together with the version lines passes",
                on(two),
                vec![
                    root_with("0.5.2", &clippy_lints(one)),
                    put("Cargo.lock", &lock("0.5.2")),
                ],
                Expect::Pass("gate configuration loosened alone"),
            ),
        ]
    }

    fn member_rows() -> Vec<Row> {
        let manifest_only = |tables: &str| vec![put(MEMBER_MANIFEST, &member(tables))];
        let new_crate = |tables: &str| {
            vec![
                put(MEMBER_MANIFEST, &member(tables)),
                put(MEMBER_SOURCE, "fn c() {}\n"),
            ]
        };
        vec![
            row(
                "a new crate without lints beside its sources fails",
                new_crate(""),
                Expect::Fail(MEMBER_MANIFEST),
            ),
            row(
                "a new crate that inherits the lints passes",
                new_crate(INHERITED),
                Expect::Pass("no gate change"),
            ),
            row_on(
                "lints removed from a crate beside a crate file fails",
                existing_member(INHERITED),
                [manifest_only(""), vec![crate_change()]].concat(),
                Expect::Fail(MEMBER_MANIFEST),
            ),
            row_on(
                "lints replaced by a local override beside a crate file fails",
                existing_member(INHERITED),
                [
                    manifest_only("[lints.clippy]\ntoo_many_lines = \"allow\"\n"),
                    vec![crate_change()],
                ]
                .concat(),
                Expect::Fail(MEMBER_MANIFEST),
            ),
            row_on(
                "a manifest deleted while its sources stay fails",
                existing_member(INHERITED),
                vec![del(MEMBER_MANIFEST), crate_change()],
                Expect::Fail(MEMBER_MANIFEST),
            ),
            row_on(
                "a whole crate deleted passes",
                existing_member(INHERITED),
                vec![del(MEMBER_MANIFEST), del(MEMBER_SOURCE), crate_change()],
                Expect::Pass("no gate change"),
            ),
            row_on(
                "lints added to a crate beside a crate file passes",
                existing_member(""),
                [manifest_only(INHERITED), vec![crate_change()]].concat(),
                Expect::Pass("gate configuration tightened"),
            ),
            row_on(
                "lints removed from a crate alone passes",
                existing_member(INHERITED),
                manifest_only(""),
                Expect::Pass("gate configuration loosened alone"),
            ),
            row_on(
                "lints removed beside a dependency change fails",
                existing_member(INHERITED),
                manifest_only("[dependencies]\nx = \"1\"\n"),
                Expect::Fail(MEMBER_MANIFEST),
            ),
            row(
                "a manifest outside the workspace members is an ordinary path",
                vec![put("tools/z/Cargo.toml", &member("")), crate_change()],
                Expect::Pass("no gate change"),
            ),
        ]
    }

    fn clippy_rows() -> Vec<Row> {
        let full = |nesting: u32| {
            format!(
                "too-many-lines-threshold = 100\ntoo-many-arguments-threshold = 7\nexcessive-nesting-threshold = {nesting}\n{LISTS}"
            )
        };
        let on = |text: String| vec![put(CLIPPY, &text)];
        let beside = |text: String| vec![put(CLIPPY, &text), crate_change()];
        let tightened = || Expect::Pass("gate configuration tightened");
        vec![
            row(
                "a clippy configuration absent from the base is an introduction beside a crate file",
                beside(full(6)),
                Expect::Pass("no gate change"),
            ),
            row_on(
                "a raised nesting threshold beside a crate file fails",
                on(full(6)),
                beside(full(7)),
                Expect::Fail(CLIPPY),
            ),
            row_on(
                "a lowered nesting threshold beside a crate file passes",
                on(full(6)),
                beside(full(5)),
                tightened(),
            ),
            row_on(
                "a nesting threshold added to a base of lists beside a crate file passes",
                on(LISTS.to_owned()),
                beside(format!("excessive-nesting-threshold = 6\n{LISTS}")),
                tightened(),
            ),
            row_on(
                "a threshold added above the default beside a crate file fails",
                on(LISTS.to_owned()),
                beside(format!("too-many-lines-threshold = 1000\n{LISTS}")),
                Expect::Fail(CLIPPY),
            ),
            row_on(
                "a removed disallowed path beside a crate file fails",
                on(full(6)),
                beside(full(6).replace(
                    "disallowed-methods = [{ path = \"std::env::var\", reason = \"configuration comes through the plugin\" }]",
                    "disallowed-methods = []",
                )),
                Expect::Fail(CLIPPY),
            ),
            row_on(
                "an added disallowed path beside a crate file passes",
                on(full(6)),
                beside(full(6).replace(
                    "disallowed-macros = [\"std::println\"]",
                    "disallowed-macros = [\"std::println\", \"std::dbg\"]",
                )),
                tightened(),
            ),
            row_on(
                "a reworded reason beside a crate file passes",
                on(full(6)),
                beside(full(6).replace("configuration comes through the plugin", "see the plugin")),
                Expect::Pass("no gate change"),
            ),
            row_on(
                "a key outside the known six beside a crate file fails",
                on(full(6)),
                beside(format!("avoid-breaking-exported-api = false\n{}", full(6))),
                Expect::Fail(CLIPPY),
            ),
            row_on(
                "a removed threshold key beside a crate file fails",
                on(full(6)),
                beside(full(6).replace("too-many-lines-threshold = 100\n", "")),
                Expect::Fail(CLIPPY),
            ),
            row_on(
                "a deleted clippy configuration beside a crate file fails",
                on(full(6)),
                vec![del(CLIPPY), crate_change()],
                Expect::Fail(CLIPPY),
            ),
            row_on(
                "an unparsable clippy configuration beside a crate file fails",
                on(full(6)),
                beside("not toml [".to_owned()),
                Expect::Fail(CLIPPY),
            ),
        ]
    }

    fn lints_file_rows() -> Vec<Row> {
        let on = |text: &str| vec![put(LINTS_FILE, text)];
        let beside = |text: &str| vec![put(LINTS_FILE, text), crate_change()];
        let clones = |min_tokens: u32, exclude: &str| {
            format!(
                "[suppressions]\n\n[bypasses]\n\n[clones]\nmin_tokens = {min_tokens}\nexclude = [{exclude}]\n"
            )
        };
        let globs = "\"crates/a/**\"";
        vec![
            row(
                "a lints file absent from the base is an introduction beside a crate file",
                beside(LINTS_ALL),
                Expect::Pass("no gate change"),
            ),
            row_on(
                "a removed clones table beside a crate file fails",
                on(LINTS_ALL),
                beside("[suppressions]\n\n[bypasses]\n"),
                Expect::Fail(LINTS_FILE),
            ),
            row_on(
                "a raised min_tokens beside a crate file fails",
                on(LINTS_ALL),
                beside(&clones(80, globs)),
                Expect::Fail(LINTS_FILE),
            ),
            row_on(
                "a lowered min_tokens beside a crate file passes",
                on(LINTS_ALL),
                beside(&clones(60, globs)),
                Expect::Pass("gate configuration tightened"),
            ),
            row_on(
                "an added exclude glob beside a crate file fails",
                on(LINTS_ALL),
                beside(&clones(70, &format!("{globs}, \"crates/b/**\""))),
                Expect::Fail(LINTS_FILE),
            ),
            row_on(
                "a removed exclude glob beside a crate file passes",
                on(LINTS_ALL),
                beside(&clones(70, "")),
                Expect::Pass("gate configuration tightened"),
            ),
            row_on(
                "an added suppressions table beside a crate file passes",
                on("[bypasses]\n"),
                beside("[suppressions]\n\n[bypasses]\n"),
                Expect::Pass("gate configuration tightened"),
            ),
            row_on(
                "a malformed lints file introduced beside a crate file fails",
                Vec::new(),
                beside("[suppresion]\n"),
                Expect::Fail("at the head is malformed"),
            ),
            row_on(
                "a malformed lints file changed on its own fails",
                on(LINTS_ALL),
                on("[suppresion]\n"),
                Expect::Fail("at the head is malformed"),
            ),
            row_on(
                "an unknown key beside a crate file fails",
                on(LINTS_ALL),
                beside(&format!("{LINTS_ALL}\n[extra]\nx = 1\n")),
                Expect::Fail(LINTS_FILE),
            ),
            row_on(
                "a deleted lints file beside a crate file fails",
                on(LINTS_ALL),
                vec![del(LINTS_FILE), crate_change()],
                Expect::Fail(LINTS_FILE),
            ),
            Row {
                base: Base::LintsRemoved,
                ..row(
                    "a lints file restored with a raised min_tokens beside a crate file fails",
                    beside(&clones(80, globs)),
                    Expect::Fail(LINTS_FILE),
                )
            },
            Row {
                base: Base::LintsRemoved,
                ..row(
                    "a lints file restored as it was beside a crate file passes",
                    beside(LINTS_ALL),
                    Expect::Pass("no gate change"),
                )
            },
            row_on(
                "a clones removal beside the version lines alone passes",
                on(LINTS_ALL),
                vec![
                    put(LINTS_FILE, "[suppressions]\n\n[bypasses]\n"),
                    put(ROOT_MANIFEST, &manifest("0.5.2")),
                    put("Cargo.lock", &lock("0.5.2")),
                ],
                Expect::Pass("gate configuration loosened alone"),
            ),
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
        if matches!(row.base, Base::LintsRemoved) {
            repo.write(LINTS_FILE, LINTS_ALL);
            repo.commit("carry the lints file");
            repo.remove(LINTS_FILE);
        }
        apply(&repo, &row.base_edits);
        repo.commit("base");
        let base = repo.sha();
        apply(&repo, &row.edits);
        repo.commit("head");
        let head = Head::Commit(repo.sha());
        (repo, base, head)
    }

    fn verify(
        name: &str,
        what: &str,
        found: Result<String, String>,
        expect: &Expect,
    ) -> Option<String> {
        match (expect, found) {
            (Expect::Pass(part), Ok(detail)) if detail.contains(part) => None,
            (Expect::Pass(_), Ok(detail)) => Some(format!("{name} ({what}): {detail}")),
            (Expect::Fail(part), Err(message)) if message.contains(part) => None,
            (Expect::Fail(_), Err(message)) => Some(format!("{name} ({what}): {message}")),
            (_, other) => Some(format!("{name} ({what}): {other:?}")),
        }
    }

    #[test]
    fn gate_config_requires_loosened_configuration_to_ship_alone() {
        judge_rows_in_parallel(&rows(), |row| {
            let (repo, base, head) = build(row);
            let gate = verify(
                row.name,
                "gate-config",
                check(&repo.0, &base, &head),
                &row.gate,
            );
            let size = row.size.as_ref().and_then(|expect| {
                verify(row.name, "size", size::check(&repo.0, &base, &head), expect)
            });
            let problems: Vec<String> = [gate, size].into_iter().flatten().collect();
            (!problems.is_empty()).then(|| problems.join("\n"))
        });
    }

    #[test]
    fn the_lints_file_switches_each_check_from_the_base() {
        enum Past<'a> {
            Never,
            Removed,
            OnTheBase(&'a str),
        }
        type Switches = (bool, bool, Option<u32>);
        type Row<'a> = (
            &'a str,
            Past<'a>,
            Option<&'a str>,
            Result<Option<Switches>, &'a str>,
        );
        let every = Some((true, true, Some(70)));
        let rows: Vec<Row> = vec![
            (
                "a base whose history never carried the file has nothing in force",
                Past::Never,
                Some(LINTS_ALL),
                Ok(None),
            ),
            (
                "a base with the suppressions table only",
                Past::OnTheBase("[suppressions]\n"),
                Some(LINTS_ALL),
                Ok(Some((true, false, None))),
            ),
            (
                "a base with all three tables",
                Past::OnTheBase(LINTS_ALL),
                Some(LINTS_ALL),
                Ok(every),
            ),
            (
                "a head that deletes the file does not change what the base has in force",
                Past::OnTheBase(LINTS_ALL),
                None,
                Ok(every),
            ),
            (
                "a base whose history removed the file and a head without it",
                Past::Removed,
                None,
                Err("quality/lints.toml was removed from the base; restore it"),
            ),
            (
                "a head that restores the file leaves the last version before the removal in force",
                Past::Removed,
                Some("[suppressions]\n"),
                Ok(every),
            ),
        ];
        judge_rows_in_parallel(&rows, |(name, past, head_file, expected)| {
            let repo = Repo::new("lints-history");
            repo.init();
            repo.write(CRATE_FILE, "fn a() {}\n");
            match past {
                Past::Never => {}
                Past::Removed => {
                    repo.write(LINTS_FILE, LINTS_ALL);
                    repo.commit("carry the lints file");
                    repo.remove(LINTS_FILE);
                }
                Past::OnTheBase(text) => repo.write(LINTS_FILE, text),
            }
            repo.commit("base");
            let base = repo.sha();
            match head_file {
                Some(text) => repo.write(LINTS_FILE, text),
                None if repo.0.join(LINTS_FILE).exists() => repo.remove(LINTS_FILE),
                None => {}
            }
            repo.write(CRATE_FILE, "fn changed() {}\n");
            repo.commit("head");
            let found = lints_at(&repo.0, &base, &Head::Commit(repo.sha())).map(|lints| {
                lints.map(|lints| {
                    (
                        lints.suppressions.is_some(),
                        lints.bypasses.is_some(),
                        lints.clones.map(|clones| clones.min_tokens),
                    )
                })
            });
            let ok = match (&found, expected) {
                (Ok(found), Ok(expected)) => found == expected,
                (Err(message), Err(part)) => message.contains(part),
                _ => false,
            };
            (!ok).then(|| format!("{name}: expected {expected:?}, found {found:?}"))
        });
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
