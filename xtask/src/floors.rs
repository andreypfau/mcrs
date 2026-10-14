use crate::metadata::Workspace;
use crate::mutation::outcomes;
use crate::scope::{self, Found, Head};
use serde::Deserialize;
use serde::de::{self, Deserializer, Visitor};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

pub(crate) const FLOORS_FILE: &str = "quality/floors.toml";
pub(crate) const FLOOR_SCOPE: &str = "crate and direct dependents";
const UNMEASURED: &str = "unmeasured";

pub(crate) struct Floors {
    crates: BTreeMap<String, Floor>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum Floor {
    Unmeasured,
    Measured(Measured),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct Measured {
    hundredths: u64,
    provenance: Provenance,
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct Provenance {
    cargo_mutants: String,
    nextest: String,
    toolchain: String,
    measured: String,
    commit: String,
    caught: u64,
    missed: u64,
    timeout: u64,
    unviable: u64,
}

pub(crate) struct Judgement {
    pub(crate) problems: Vec<String>,
    pub(crate) tightened: BTreeSet<String>,
}

enum RawFloor {
    Unmeasured,
    Hundredths(u64),
}

impl<'de> Deserialize<'de> for RawFloor {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Reader;

        const SHAPE: &str = "\"unmeasured\" or a number between 0 and 1 with at most two decimals";

        impl Visitor<'_> for Reader {
            type Value = RawFloor;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str(SHAPE)
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<RawFloor, E> {
                if value == UNMEASURED {
                    Ok(RawFloor::Unmeasured)
                } else {
                    Err(E::custom(format!("the floor {value:?} is not {SHAPE}")))
                }
            }

            fn visit_f64<E: de::Error>(self, value: f64) -> Result<RawFloor, E> {
                if !(0.0..=1.0).contains(&value) {
                    return Err(E::custom(format!(
                        "the floor {value} is not between 0 and 1"
                    )));
                }
                let hundredths = (value * 100.0).round();
                if hundredths / 100.0 != value {
                    return Err(E::custom(format!(
                        "the floor {value} has more than two decimals"
                    )));
                }
                Ok(RawFloor::Hundredths(hundredths as u64))
            }

            fn visit_i64<E: de::Error>(self, value: i64) -> Result<RawFloor, E> {
                match value {
                    0 | 1 => Ok(RawFloor::Hundredths(value as u64 * 100)),
                    _ => Err(E::custom(format!(
                        "the floor {value} is not between 0 and 1"
                    ))),
                }
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<RawFloor, E> {
                match value {
                    0 | 1 => Ok(RawFloor::Hundredths(value * 100)),
                    _ => Err(E::custom(format!(
                        "the floor {value} is not between 0 and 1"
                    ))),
                }
            }
        }

        deserializer.deserialize_any(Reader)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    floor: RawFloor,
    scope: Option<String>,
    cargo_mutants: Option<String>,
    nextest: Option<String>,
    toolchain: Option<String>,
    measured: Option<String>,
    commit: Option<String>,
    caught: Option<u64>,
    missed: Option<u64>,
    timeout: Option<u64>,
    unviable: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    #[serde(default, rename = "crate")]
    crates: BTreeMap<String, RawEntry>,
}

pub(crate) fn show(hundredths: u64) -> String {
    format!("{}.{:02}", hundredths / 100, hundredths % 100)
}

pub(crate) fn is_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 10
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            4 | 7 => *byte == b'-',
            _ => byte.is_ascii_digit(),
        })
}

fn text_field(name: &str, field: &str, value: Option<String>) -> Result<String, String> {
    match value {
        None => Err(format!(
            "{name}: {field} is missing; a number carries every provenance field"
        )),
        Some(value) if value.trim().is_empty() => Err(format!(
            "{name}: {field} is empty; a number carries every provenance field"
        )),
        Some(value) => Ok(value),
    }
}

fn count_field(name: &str, field: &str, value: Option<u64>) -> Result<u64, String> {
    value.ok_or_else(|| {
        format!("{name}: {field} is missing; a number carries every provenance field")
    })
}

impl RawEntry {
    fn first_provenance_field(&self) -> Option<&'static str> {
        [
            ("scope", self.scope.is_some()),
            ("cargo_mutants", self.cargo_mutants.is_some()),
            ("nextest", self.nextest.is_some()),
            ("toolchain", self.toolchain.is_some()),
            ("measured", self.measured.is_some()),
            ("commit", self.commit.is_some()),
            ("caught", self.caught.is_some()),
            ("missed", self.missed.is_some()),
            ("timeout", self.timeout.is_some()),
            ("unviable", self.unviable.is_some()),
        ]
        .into_iter()
        .find_map(|(field, given)| given.then_some(field))
    }

    fn into_floor(self, name: &str) -> Result<Floor, String> {
        let hundredths = match self.floor {
            RawFloor::Unmeasured => {
                return match self.first_provenance_field() {
                    Some(field) => Err(format!(
                        "{name}: {field} belongs to a numeric floor; remove it, or record the floor"
                    )),
                    None => Ok(Floor::Unmeasured),
                };
            }
            RawFloor::Hundredths(hundredths) => hundredths,
        };
        let scope = text_field(name, "scope", self.scope)?;
        if scope != FLOOR_SCOPE {
            return Err(format!(
                "{name}: scope must be {FLOOR_SCOPE:?}, not {scope:?}"
            ));
        }
        let provenance = Provenance {
            cargo_mutants: text_field(name, "cargo_mutants", self.cargo_mutants)?,
            nextest: text_field(name, "nextest", self.nextest)?,
            toolchain: text_field(name, "toolchain", self.toolchain)?,
            measured: text_field(name, "measured", self.measured)?,
            commit: text_field(name, "commit", self.commit)?,
            caught: count_field(name, "caught", self.caught)?,
            missed: count_field(name, "missed", self.missed)?,
            timeout: count_field(name, "timeout", self.timeout)?,
            unviable: count_field(name, "unviable", self.unviable)?,
        };
        if !is_date(&provenance.measured) {
            return Err(format!(
                "{name}: measured must be a date written YYYY-MM-DD, not {:?}",
                provenance.measured
            ));
        }
        let Some(scored) =
            outcomes::score(provenance.caught, provenance.timeout, provenance.missed)
        else {
            return Err(format!(
                "{name}: no mutant was counted, so the crate can only be unmeasured"
            ));
        };
        if scored != hundredths {
            return Err(format!(
                "{name}: the floor {} disagrees with its counts, which score {}; the floor must be caught plus timeout over caught, timeout and missed, rounded down to two decimals",
                show(hundredths),
                show(scored)
            ));
        }
        Ok(Floor::Measured(Measured {
            hundredths,
            provenance,
        }))
    }
}

fn describe(text: &str, error: &toml::de::Error) -> String {
    match error.span() {
        Some(span) => {
            let before = &text.as_bytes()[..span.start.min(text.len())];
            let line = before.iter().filter(|byte| **byte == b'\n').count() + 1;
            format!("line {line}: {}", error.message())
        }
        None => error.message().to_owned(),
    }
}

pub(crate) fn parse(text: &str) -> Result<Floors, String> {
    let raw: RawFile = toml::from_str(text).map_err(|e| describe(text, &e))?;
    let mut crates = BTreeMap::new();
    let mut problems = Vec::new();
    for (name, entry) in raw.crates {
        match entry.into_floor(&name) {
            Ok(floor) => {
                crates.insert(name, floor);
            }
            Err(problem) => problems.push(problem),
        }
    }
    if problems.is_empty() {
        Ok(Floors { crates })
    } else {
        Err(problems.join("\n"))
    }
}

impl Floors {
    pub(crate) fn hundredths(&self, name: &str) -> Option<u64> {
        match self.crates.get(name)? {
            Floor::Measured(measured) => Some(measured.hundredths),
            Floor::Unmeasured => None,
        }
    }

    pub(crate) fn measured(&self) -> impl Iterator<Item = (&str, u64)> {
        self.crates.iter().filter_map(|(name, floor)| match floor {
            Floor::Measured(measured) => Some((name.as_str(), measured.hundredths)),
            Floor::Unmeasured => None,
        })
    }

    fn summary(&self) -> String {
        let floored = self
            .crates
            .values()
            .filter(|floor| matches!(floor, Floor::Measured(_)))
            .count();
        format!("{} crates, {floored} floored", self.crates.len())
    }
}

pub(crate) fn judge(base: &Floors, head: &Floors, packages: &BTreeSet<String>) -> Judgement {
    let mut problems = Vec::new();
    let mut tightened = BTreeSet::new();
    for (name, before) in &base.crates {
        match (before, head.crates.get(name)) {
            (Floor::Unmeasured, None | Some(Floor::Unmeasured)) => {}
            (Floor::Unmeasured, Some(Floor::Measured(_))) => {
                tightened.insert(name.clone());
            }
            (Floor::Measured(_), None) if packages.contains(name) => problems.push(format!(
                "{name}: the entry of a package still in the workspace was removed; restore it with its floor"
            )),
            (Floor::Measured(_), None) => {}
            (Floor::Measured(before), Some(Floor::Unmeasured)) => problems.push(format!(
                "{name}: the floor {} was turned into unmeasured; a floor never falls, so keep the number",
                show(before.hundredths)
            )),
            (Floor::Measured(before), Some(Floor::Measured(after))) => {
                match after.hundredths.cmp(&before.hundredths) {
                    std::cmp::Ordering::Less => problems.push(format!(
                        "{name}: the floor fell from {} to {}; a floor never falls, so keep the tests that hold it or add tests that kill more mutants",
                        show(before.hundredths),
                        show(after.hundredths)
                    )),
                    std::cmp::Ordering::Greater => {
                        tightened.insert(name.clone());
                    }
                    std::cmp::Ordering::Equal if after.provenance != before.provenance => {
                        problems.push(format!(
                            "{name}: the provenance changed without a raise; keep the recorded measurement until the floor rises"
                        ))
                    }
                    std::cmp::Ordering::Equal => {}
                }
            }
        }
    }
    for (name, after) in &head.crates {
        if !base.crates.contains_key(name) && matches!(after, Floor::Measured(_)) {
            tightened.insert(name.clone());
        }
    }
    Judgement {
        problems,
        tightened,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Measurement {
    pub(crate) package: String,
    pub(crate) cargo_mutants: String,
    pub(crate) nextest: String,
    pub(crate) toolchain: String,
    pub(crate) measured: String,
    pub(crate) commit: String,
    pub(crate) caught: u64,
    pub(crate) missed: u64,
    pub(crate) timeout: u64,
    pub(crate) unviable: u64,
}

impl Measurement {
    pub(crate) fn hundredths(&self) -> Option<u64> {
        outcomes::score(self.caught, self.timeout, self.missed)
    }
}

impl Measurement {
    fn table(&self, hundredths: u64) -> Result<String, String> {
        let quoted = |value: &str| {
            serde_json::to_string(value).map_err(|e| format!("{}: {e}", self.package))
        };
        Ok(format!(
            "[crate.{}]\nfloor = {}\nscope = {}\ncargo_mutants = {}\nnextest = {}\ntoolchain = {}\nmeasured = {}\ncommit = {}\ncaught = {}\nmissed = {}\ntimeout = {}\nunviable = {}\n",
            self.package,
            show(hundredths),
            quoted(FLOOR_SCOPE)?,
            quoted(&self.cargo_mutants)?,
            quoted(&self.nextest)?,
            quoted(&self.toolchain)?,
            quoted(&self.measured)?,
            quoted(&self.commit)?,
            self.caught,
            self.missed,
            self.timeout,
            self.unviable
        ))
    }
}

pub(crate) fn raise(text: &str, measurements: &[Measurement]) -> Result<String, String> {
    let before = parse(text)?;
    let mut lines: Vec<String> = text.split_inclusive('\n').map(str::to_owned).collect();
    let mut raised = BTreeSet::new();
    for measurement in measurements {
        let name = &measurement.package;
        let Some(hundredths) = measurement.hundredths() else {
            return Err(format!("{name}: no mutant was counted, so it has no score"));
        };
        let Some(old) = before.hundredths(name) else {
            return Err(format!(
                "{name}: the floor is unmeasured or has no table in {FLOORS_FILE}, and a raise never enrols a crate"
            ));
        };
        if hundredths <= old {
            return Err(format!(
                "{name}: the score {} does not exceed the floor {}",
                show(hundredths),
                show(old)
            ));
        }
        if !raised.insert(name.clone()) {
            return Err(format!("{name}: raised twice"));
        }
        let header = format!("[crate.{name}]");
        let Some(start) = lines.iter().position(|line| line.trim_end() == header) else {
            return Err(format!("{name}: {FLOORS_FILE} has no {header} table"));
        };
        let end = lines[start + 1..]
            .iter()
            .position(|line| line.starts_with('['))
            .map_or(lines.len(), |offset| start + 1 + offset);
        let blank_after = lines[start..end]
            .iter()
            .rev()
            .take_while(|line| line.trim().is_empty())
            .count();
        let mut table: Vec<String> = measurement
            .table(hundredths)?
            .split_inclusive('\n')
            .map(str::to_owned)
            .collect();
        let had_newline = lines[end - blank_after - 1].ends_with('\n');
        if !had_newline
            && blank_after == 0
            && let Some(last) = table.last_mut()
        {
            last.pop();
        }
        table.extend(std::iter::repeat_n("\n".to_owned(), blank_after));
        lines.splice(start..end, table);
    }
    let rewritten = lines.concat();
    let after = parse(&rewritten)?;
    let packages: BTreeSet<String> = after.crates.keys().cloned().collect();
    let judged = judge(&before, &after, &packages);
    if !judged.problems.is_empty() {
        return Err(judged.problems.join("\n"));
    }
    if judged.tightened != raised {
        return Err(format!(
            "the rewrite tightened {:?}, not the raised {raised:?}",
            judged.tightened
        ));
    }
    Ok(rewritten)
}

pub(crate) fn completeness_problems(floors: &Floors, workspace: &Workspace) -> Vec<String> {
    let packages: BTreeSet<&str> = workspace.packages.iter().map(|p| p.name.as_str()).collect();
    let mut problems = Vec::new();
    for name in &packages {
        if !floors.crates.contains_key(*name) {
            problems.push(format!(
                "{name}: the workspace package has no entry in {FLOORS_FILE}; add [crate.{name}] with floor = \"unmeasured\""
            ));
        }
    }
    for name in floors.crates.keys() {
        if !packages.contains(name.as_str()) {
            problems.push(format!(
                "{name}: {FLOORS_FILE} has an entry for a package that is not in the workspace; remove it"
            ));
        }
    }
    problems
}

pub(crate) fn completeness(
    repo: &Path,
    base: &str,
    head: &Head,
    workspace: &Workspace,
) -> Result<String, String> {
    let short = scope::short(base);
    let in_force = scope::configuration_at(repo, base, head, FLOORS_FILE)?.is_some();
    let Some(text) = scope::head_text(repo, head, FLOORS_FILE)? else {
        return if in_force {
            Err(format!("{FLOORS_FILE} is missing at the head; restore it"))
        } else {
            Ok(String::new())
        };
    };
    let floors =
        parse(&text).map_err(|e| format!("{FLOORS_FILE} at the head is malformed: {e}"))?;
    let problems = completeness_problems(&floors, workspace);
    match (problems.is_empty(), in_force) {
        (true, true) => Ok(format!("{} floor entries complete", floors.crates.len())),
        (true, false) => Ok(format!(
            "floors not in force yet at {short}; {} entries complete",
            floors.crates.len()
        )),
        (false, true) => Err(problems.join("\n")),
        (false, false) => Ok(format!(
            "floors not in force yet at {short}; {} entries incomplete: {}",
            problems.len(),
            problems.join("; ")
        )),
    }
}

#[derive(Deserialize)]
struct Manifest {
    package: Option<NamedPackage>,
    workspace: Option<WorkspaceTable>,
}

#[derive(Deserialize)]
struct NamedPackage {
    name: String,
}

#[derive(Deserialize)]
struct WorkspaceTable {
    #[serde(default)]
    members: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
}

fn manifest_at(repo: &Path, head: &Head, path: &str) -> Result<Manifest, String> {
    let text = scope::head_text(repo, head, path)?
        .ok_or_else(|| format!("{path} is missing at the head"))?;
    toml::from_str(&text).map_err(|e| format!("{path} at the head is malformed: {}", e.message()))
}

fn wildcard(pattern: &[u8], text: &[u8]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some((b'*', rest)) => (0..=text.len()).any(|skip| wildcard(rest, &text[skip..])),
        Some((b'?', rest)) => !text.is_empty() && wildcard(rest, &text[1..]),
        Some((byte, rest)) => text.first() == Some(byte) && wildcard(rest, &text[1..]),
    }
}

// chisle: only `*` and `?` inside one path component are expanded; `**` and character classes
// fail loudly. A path dependency that no member pattern names is not followed. Lifted when the
// workspace needs either, because the completeness check against cargo metadata disagrees.
fn is_member(pattern: &str, dir: &str) -> Result<bool, String> {
    if pattern.contains("**") || pattern.contains(['[', '{']) {
        return Err(format!(
            "the workspace member pattern {pattern} uses syntax the floors check cannot expand; list the members with * or ? only"
        ));
    }
    let pattern = pattern.trim_start_matches("./").trim_end_matches('/');
    let (patterns, parts) = (pattern.split('/'), dir.split('/'));
    if patterns.clone().count() != parts.clone().count() {
        return Ok(false);
    }
    Ok(patterns
        .zip(parts)
        .all(|(pattern, part)| wildcard(pattern.as_bytes(), part.as_bytes())))
}

fn is_excluded(exclude: &str, dir: &str) -> bool {
    let exclude = exclude.trim_start_matches("./").trim_end_matches('/');
    dir == exclude
        || dir
            .strip_prefix(exclude)
            .is_some_and(|rest| rest.starts_with('/'))
}

pub(crate) fn head_packages(repo: &Path, head: &Head) -> Result<BTreeSet<String>, String> {
    let root = manifest_at(repo, head, "Cargo.toml")?;
    let mut names = BTreeSet::new();
    if let Some(package) = root.package {
        names.insert(package.name);
    }
    let Some(workspace) = root.workspace else {
        return Ok(names);
    };
    let tracked = scope::tracked(repo, head)?;
    for dir in tracked
        .iter()
        .filter_map(|path| path.strip_suffix("/Cargo.toml"))
    {
        if workspace.exclude.iter().any(|ex| is_excluded(ex, dir)) {
            continue;
        }
        let mut member = false;
        for pattern in &workspace.members {
            member |= is_member(pattern, dir)?;
        }
        if !member {
            continue;
        }
        let path = format!("{dir}/Cargo.toml");
        let package = manifest_at(repo, head, &path)?.package.ok_or_else(|| {
            format!("{path} declares no package; every workspace member needs one")
        })?;
        names.insert(package.name);
    }
    Ok(names)
}

pub(crate) fn pair(
    repo: &Path,
    base: &str,
    head: &Head,
) -> Result<(Option<Floors>, Option<Floors>), String> {
    let short = scope::short(base);
    let base_floors = match scope::configuration_at(repo, base, head, FLOORS_FILE)? {
        None => None,
        Some(Found::AtBase(text)) => Some(
            parse(&text)
                .map_err(|e| format!("{FLOORS_FILE} at the base {short} is malformed: {e}"))?,
        ),
        Some(Found::Restored { last, .. }) => Some(parse(&last).map_err(|e| {
            format!("{FLOORS_FILE} before its removal from the base {short} is malformed: {e}")
        })?),
    };
    let head_floors = scope::head_text(repo, head, FLOORS_FILE)?
        .map(|text| {
            parse(&text).map_err(|e| format!("{FLOORS_FILE} at the head is malformed: {e}"))
        })
        .transpose()?;
    Ok((base_floors, head_floors))
}

pub(crate) fn check(repo: &Path, base: &str, head: &Head) -> Result<String, String> {
    let short = scope::short(base);
    let found = scope::configuration_at(repo, base, head, FLOORS_FILE)?;
    let head_floors = scope::head_text(repo, head, FLOORS_FILE)?
        .map(|text| {
            parse(&text).map_err(|e| format!("{FLOORS_FILE} at the head is malformed: {e}"))
        })
        .transpose()?;
    let (reference, which) = match &found {
        None => {
            let summary = head_floors
                .as_ref()
                .map_or_else(|| "0 crates, 0 floored".to_owned(), Floors::summary);
            return Ok(format!("not in force yet at {short}; {summary}"));
        }
        Some(Found::AtBase(text)) => (text, format!("at the base {short}")),
        Some(Found::Restored { last, .. }) => {
            (last, format!("before its removal from the base {short}"))
        }
    };
    let base_floors = parse(reference).map_err(|e| {
        format!("{FLOORS_FILE} {which} is malformed: {e}; fix it on the base branch")
    })?;
    let head_floors =
        head_floors.ok_or_else(|| format!("{FLOORS_FILE} is missing at the head; restore it"))?;
    let judged = judge(&base_floors, &head_floors, &head_packages(repo, head)?);
    if judged.problems.is_empty() {
        Ok(head_floors.summary())
    } else {
        Err(judged.problems.join("\n"))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::scope::tests::{Repo, judge_rows_in_parallel};

    pub(crate) struct Spec<'a> {
        pub(crate) name: &'a str,
        pub(crate) floor: &'a str,
        pub(crate) counts: (u64, u64, u64),
        pub(crate) commit: &'a str,
        pub(crate) measured: &'a str,
    }

    pub(crate) fn measured<'a>(name: &'a str, floor: &'a str, counts: (u64, u64, u64)) -> Spec<'a> {
        Spec {
            name,
            floor,
            counts,
            commit: "c1",
            measured: "2026-11-02",
        }
    }

    impl Spec<'_> {
        pub(crate) fn text(&self) -> String {
            let (caught, timeout, missed) = self.counts;
            format!(
                "[crate.{}]\nfloor = {}\nscope = \"crate and direct dependents\"\ncargo_mutants = \"27.1.0\"\nnextest = \"0.9.143\"\ntoolchain = \"nightly-2026-09-02\"\nmeasured = \"{}\"\ncommit = \"{}\"\ncaught = {caught}\nmissed = {missed}\ntimeout = {timeout}\nunviable = 9\n\n",
                self.name, self.floor, self.measured, self.commit
            )
        }
    }

    pub(crate) fn unmeasured(name: &str) -> String {
        format!("[crate.{name}]\nfloor = \"unmeasured\"\n\n")
    }

    const EIGHTY_THREE: (u64, u64, u64) = (83, 0, 17);

    fn without(text: String, line: &str) -> String {
        assert!(text.contains(line), "{line:?} not in {text}");
        text.replace(line, "")
    }

    type Wanted<'a> = Result<(), &'a str>;

    fn expect_all(rows: Vec<(&str, Result<(), String>, Wanted)>) {
        let mut wrong = Vec::new();
        for (name, found, expected) in rows {
            match (&found, expected) {
                (Ok(()), Ok(())) => {}
                (Err(message), Err(part)) if message.contains(part) => {}
                _ => wrong.push(format!("{name}: expected {expected:?}, found {found:?}")),
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }

    fn measurement(name: &str, caught: u64, missed: u64) -> Measurement {
        Measurement {
            package: name.to_owned(),
            cargo_mutants: "27.1.0".to_owned(),
            nextest: "0.9.143".to_owned(),
            toolchain: "rustc 1.100.0-nightly (5db7f4be8 2026-09-01)".to_owned(),
            measured: "2026-12-07".to_owned(),
            commit: "feed".to_owned(),
            caught,
            missed,
            timeout: 0,
            unviable: 4,
        }
    }

    #[test]
    fn raised_floors_keep_the_file_format() {
        let before = format!(
            "{}{}{}",
            unmeasured("a"),
            measured("b", "0.80", (80, 0, 20)).text(),
            measured("c", "0.90", (90, 0, 10)).text()
        );
        let raised_b = "[crate.b]\nfloor = 0.83\nscope = \"crate and direct dependents\"\ncargo_mutants = \"27.1.0\"\nnextest = \"0.9.143\"\ntoolchain = \"rustc 1.100.0-nightly (5db7f4be8 2026-09-01)\"\nmeasured = \"2026-12-07\"\ncommit = \"feed\"\ncaught = 83\nmissed = 17\ntimeout = 0\nunviable = 4\n\n";
        let after = raise(&before, &[measurement("b", 83, 17)]).unwrap();
        assert_eq!(
            after,
            format!(
                "{}{raised_b}{}",
                unmeasured("a"),
                measured("c", "0.90", (90, 0, 10)).text()
            )
        );
        let packages: BTreeSet<String> = ["a", "b", "c"].map(str::to_owned).into();
        let after_floors = parse(&after).unwrap();
        let judged = judge(&parse(&before).unwrap(), &after_floors, &packages);
        assert!(judged.problems.is_empty(), "{:?}", judged.problems);
        assert_eq!(judged.tightened, BTreeSet::from(["b".to_owned()]));
        assert_eq!(
            after_floors.hundredths("b"),
            measurement("b", 83, 17).hundredths()
        );

        let last = format!(
            "{}{}",
            unmeasured("a"),
            measured("b", "0.80", (80, 0, 20)).text().trim_end()
        );
        let raised_last = raise(&last, &[measurement("b", 83, 17)]).unwrap();
        assert!(raised_last.ends_with("unviable = 4"), "{raised_last:?}");
        assert!(parse(&raised_last).is_ok());

        let rows: [(&str, Vec<Measurement>, &str); 4] = [
            ("a crate with no table", vec![measurement("z", 90, 10)], "z"),
            (
                "an unmeasured crate is never enrolled",
                vec![measurement("a", 90, 10)],
                "a",
            ),
            (
                "a floor that does not rise",
                vec![measurement("b", 80, 20)],
                "b",
            ),
            ("a floor that falls", vec![measurement("c", 85, 15)], "c"),
        ];
        let mut wrong = Vec::new();
        for (name, raises, part) in rows {
            match raise(&before, &raises) {
                Err(message) if message.contains(part) => {}
                other => wrong.push(format!("{name}: {other:?}")),
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }

    #[test]
    fn a_floor_equals_its_counts_rounded_down() {
        let eighty_three = || measured("a", "0.83", EIGHTY_THREE).text();
        let rows: Vec<(&str, String, Wanted)> = vec![
            ("unmeasured", unmeasured("a"), Ok(())),
            ("a floor that equals its counts", eighty_three(), Ok(())),
            (
                "timeouts count as caught",
                measured("a", "0.83", (82, 1, 17)).text(),
                Ok(()),
            ),
            (
                "a whole number",
                measured("a", "1", (5, 0, 0)).text(),
                Ok(()),
            ),
            (
                "a floor above its counts",
                measured("a", "0.84", EIGHTY_THREE).text(),
                Err("a: the floor 0.84 disagrees with its counts, which score 0.83"),
            ),
            (
                "a floor below its counts",
                measured("a", "0.80", EIGHTY_THREE).text(),
                Err("a: the floor 0.80 disagrees with its counts, which score 0.83"),
            ),
            (
                "more than two decimals",
                measured("a", "0.835", EIGHTY_THREE).text(),
                Err("two decimals"),
            ),
            (
                "above one",
                measured("a", "1.5", EIGHTY_THREE).text(),
                Err("between 0 and 1"),
            ),
            (
                "below zero",
                measured("a", "-0.1", EIGHTY_THREE).text(),
                Err("between 0 and 1"),
            ),
            (
                "a word other than unmeasured",
                measured("a", "\"high\"", EIGHTY_THREE).text(),
                Err("unmeasured"),
            ),
            (
                "a missing commit",
                without(eighty_three(), "commit = \"c1\"\n"),
                Err("a: commit"),
            ),
            (
                "a missing count",
                without(eighty_three(), "missed = 17\n"),
                Err("a: missed"),
            ),
            (
                "an empty toolchain",
                eighty_three().replace("nightly-2026-09-02", ""),
                Err("a: toolchain"),
            ),
            (
                "an empty commit",
                measured("a", "0.83", EIGHTY_THREE)
                    .text()
                    .replace("\"c1\"", "\"\""),
                Err("a: commit"),
            ),
            (
                "all counts zero",
                measured("a", "0.00", (0, 0, 0)).text(),
                Err("a: no mutant was counted"),
            ),
            (
                "a scope that names another",
                eighty_three().replace("crate and direct dependents", "the crate"),
                Err("a: scope"),
            ),
            (
                "a date that is not a date",
                eighty_three().replace("2026-11-02", "yesterday"),
                Err("a: measured"),
            ),
            (
                "an unknown key",
                eighty_three().replace("unviable = 9\n", "unviable = 9\nextra = 1\n"),
                Err("extra"),
            ),
            (
                "provenance on an unmeasured crate",
                "[crate.a]\nfloor = \"unmeasured\"\ncommit = \"c1\"\n".to_owned(),
                Err("a: commit"),
            ),
            (
                "an unknown top-level table",
                format!("{}[other]\nx = 1\n", unmeasured("a")),
                Err("other"),
            ),
        ];
        expect_all(
            rows.into_iter()
                .map(|(name, text, expected)| (name, parse(&text).map(|_| ()), expected))
                .collect(),
        );
    }

    enum Expect {
        Fails(&'static str),
        Tightens,
        Unchanged,
    }

    fn verdict(base: &str, head: &str, packages: &[&str]) -> Result<Result<bool, String>, String> {
        let base = parse(base).map_err(|e| format!("the base did not parse: {e}"))?;
        let head = match parse(head) {
            Ok(head) => head,
            Err(message) => return Ok(Err(message)),
        };
        let packages: BTreeSet<String> = packages.iter().map(|p| (*p).to_owned()).collect();
        let judged = judge(&base, &head, &packages);
        Ok(if judged.problems.is_empty() {
            Ok(!judged.tightened.is_empty())
        } else {
            Err(judged.problems.join("\n"))
        })
    }

    #[test]
    fn floors_never_fall() {
        let at = |floor, counts| measured("a", floor, counts).text();
        let base = || at("0.83", EIGHTY_THREE);
        let live = ["a", "b"];
        let rows: Vec<(&str, String, String, &[&str], Expect)> = vec![
            (
                "a lowered floor",
                base(),
                at("0.80", (80, 0, 20)),
                &live,
                Expect::Fails("a: the floor fell from 0.83 to 0.80"),
            ),
            (
                "a lowered floor beside a raised one",
                format!("{}{}", base(), measured("b", "0.50", (1, 0, 1)).text()),
                format!(
                    "{}{}",
                    at("0.80", (80, 0, 20)),
                    measured("b", "0.90", (9, 0, 1)).text()
                ),
                &live,
                Expect::Fails("a: the floor fell"),
            ),
            (
                "a number turned into unmeasured",
                base(),
                unmeasured("a"),
                &live,
                Expect::Fails("a: the floor 0.83 was turned into unmeasured"),
            ),
            (
                "a dropped commit",
                base(),
                without(base(), "commit = \"c1\"\n"),
                &live,
                Expect::Fails("a: commit"),
            ),
            (
                "a changed measurement date with the same floor",
                base(),
                base().replace("2026-11-02", "2026-12-01"),
                &live,
                Expect::Fails("a: the provenance changed without a raise"),
            ),
            (
                "a changed commit with the same floor",
                base(),
                base().replace("\"c1\"", "\"c2\""),
                &live,
                Expect::Fails("a: the provenance changed without a raise"),
            ),
            (
                "a removed numeric entry of a live package",
                format!("{}{}", base(), unmeasured("b")),
                unmeasured("b"),
                &live,
                Expect::Fails("a: the entry of a package still in the workspace was removed"),
            ),
            (
                "an enrolment",
                unmeasured("a"),
                base(),
                &live,
                Expect::Tightens,
            ),
            (
                "a raise with its new provenance",
                base(),
                at("0.84", (84, 0, 16)).replace("\"c1\"", "\"c2\""),
                &live,
                Expect::Tightens,
            ),
            (
                "a new crate with a floor",
                unmeasured("b"),
                format!("{}{}", base(), unmeasured("b")),
                &live,
                Expect::Tightens,
            ),
            (
                "an added unmeasured entry",
                base(),
                format!("{}{}", base(), unmeasured("b")),
                &live,
                Expect::Unchanged,
            ),
            (
                "a removed unmeasured entry",
                format!("{}{}", base(), unmeasured("b")),
                base(),
                &live,
                Expect::Unchanged,
            ),
            (
                "a removed numeric entry of a package gone from the head",
                format!(
                    "{}{}",
                    base(),
                    at("0.80", (80, 0, 20)).replace("[crate.a]", "[crate.gone]")
                ),
                base(),
                &live,
                Expect::Unchanged,
            ),
            ("no change", base(), base(), &live, Expect::Unchanged),
        ];
        let mut wrong = Vec::new();
        for (name, base, head, packages, expected) in rows {
            let found = verdict(&base, &head, packages);
            let ok = match (&found, &expected) {
                (Ok(Err(message)), Expect::Fails(part)) => message.contains(part),
                (Ok(Ok(true)), Expect::Tightens) => true,
                (Ok(Ok(false)), Expect::Unchanged) => true,
                _ => false,
            };
            if !ok {
                wrong.push(format!("{name}: {found:?}"));
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }

    struct Layout {
        name: &'static str,
        files: Vec<(&'static str, &'static str)>,
        packages: Result<Vec<&'static str>, &'static str>,
    }

    fn manifest(name: &str) -> String {
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n")
    }

    #[test]
    fn the_head_packages_are_read_as_git_data() {
        let workspace = |members: &'static str| -> &'static str {
            Box::leak(
                format!("[package]\nname = \"mcrs\"\n\n[workspace]\nmembers = {members}\n")
                    .into_boxed_str(),
            )
        };
        let leaf = |name: &str| -> &'static str { Box::leak(manifest(name).into_boxed_str()) };
        let layouts = vec![
            Layout {
                name: "a pattern, a literal member and the root package",
                files: vec![
                    ("Cargo.toml", workspace("[\"crates/*\", \"xtask\"]")),
                    ("crates/a/Cargo.toml", leaf("alpha")),
                    ("crates/b/Cargo.toml", leaf("b")),
                    ("crates/c/inner/Cargo.toml", leaf("nested")),
                    ("crates/d/notes.txt", "no manifest\n"),
                    ("xtask/Cargo.toml", leaf("xtask")),
                    ("other/Cargo.toml", leaf("outside")),
                ],
                packages: Ok(vec!["alpha", "b", "mcrs", "xtask"]),
            },
            Layout {
                name: "an excluded member",
                files: vec![
                    (
                        "Cargo.toml",
                        "[package]\nname = \"mcrs\"\n\n[workspace]\nmembers = [\"crates/*\"]\nexclude = [\"crates/b\"]\n",
                    ),
                    ("crates/a/Cargo.toml", leaf("alpha")),
                    ("crates/b/Cargo.toml", leaf("b")),
                ],
                packages: Ok(vec!["alpha", "mcrs"]),
            },
            Layout {
                name: "a virtual root has no package of its own",
                files: vec![
                    ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
                    ("crates/a/Cargo.toml", leaf("alpha")),
                ],
                packages: Ok(vec!["alpha"]),
            },
            Layout {
                name: "a root without a workspace is its own package",
                files: vec![
                    ("Cargo.toml", "[package]\nname = \"solo\"\n"),
                    ("crates/a/Cargo.toml", leaf("alpha")),
                ],
                packages: Ok(vec!["solo"]),
            },
            Layout {
                name: "a question mark and a star inside one component",
                files: vec![
                    ("Cargo.toml", workspace("[\"crates/?\", \"libs/x-*\"]")),
                    ("crates/a/Cargo.toml", leaf("alpha")),
                    ("crates/bb/Cargo.toml", leaf("long")),
                    ("libs/x-one/Cargo.toml", leaf("one")),
                    ("libs/y-one/Cargo.toml", leaf("skipped")),
                ],
                packages: Ok(vec!["alpha", "mcrs", "one"]),
            },
            Layout {
                name: "a member without a package",
                files: vec![
                    ("Cargo.toml", workspace("[\"crates/*\"]")),
                    ("crates/a/Cargo.toml", "[workspace]\n"),
                ],
                packages: Err("crates/a/Cargo.toml declares no package"),
            },
            Layout {
                name: "a pattern the check cannot expand",
                files: vec![
                    ("Cargo.toml", workspace("[\"crates/**\"]")),
                    ("crates/a/Cargo.toml", leaf("alpha")),
                ],
                packages: Err("crates/**"),
            },
        ];
        let mut wrong = Vec::new();
        for layout in layouts {
            let repo = Repo::new("floors-packages");
            repo.init();
            for (path, text) in &layout.files {
                repo.write(path, text);
            }
            repo.commit("head");
            let head = Head::Commit(repo.sha());
            for (path, _) in &layout.files {
                if path.ends_with("Cargo.toml") && *path != "Cargo.toml" {
                    repo.remove(path);
                }
            }
            let found = head_packages(&repo.0, &head);
            let ok = match (&found, &layout.packages) {
                (Ok(found), Ok(expected)) => {
                    found.iter().map(String::as_str).collect::<Vec<_>>() == *expected
                }
                (Err(message), Err(part)) => message.contains(part),
                _ => false,
            };
            if !ok {
                wrong.push(format!("{}: {found:?}", layout.name));
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }

    fn seed(repo: &Repo) {
        repo.write(
            "Cargo.toml",
            "[package]\nname = \"mcrs\"\n\n[workspace]\nmembers = [\"crates/*\"]\n",
        );
        repo.write("crates/a/Cargo.toml", &manifest("a"));
        repo.write("crates/a/src/lib.rs", "pub fn a() {}\n");
    }

    fn pair(a: &str, mcrs: &str) -> String {
        format!("{a}{mcrs}")
    }

    #[test]
    fn the_floors_file_is_in_force_from_its_history() {
        enum Past {
            Never,
            Removed,
            OnTheBase,
        }
        type Row<'a> = (&'a str, Past, Option<String>, Result<&'a str, &'a str>);
        let carried = || {
            pair(
                &measured("a", "0.83", EIGHTY_THREE).text(),
                &unmeasured("mcrs"),
            )
        };
        let lowered = || {
            pair(
                &measured("a", "0.80", (80, 0, 20)).text(),
                &unmeasured("mcrs"),
            )
        };
        let rows: Vec<Row> = vec![
            (
                "a base whose history never carried the file",
                Past::Never,
                Some(lowered()),
                Ok("not in force yet at"),
            ),
            (
                "a head that adds a malformed file before it is in force",
                Past::Never,
                Some("[crate.a]\nfloor = \"high\"\n".to_owned()),
                Err("malformed"),
            ),
            (
                "a base whose history removed the file and a head without it",
                Past::Removed,
                None,
                Err("was removed from the base; restore it"),
            ),
            (
                "a head that restores the file lowered",
                Past::Removed,
                Some(lowered()),
                Err("a: the floor fell from 0.83 to 0.80"),
            ),
            (
                "a head that restores the file as it was",
                Past::Removed,
                Some(carried()),
                Ok("2 crates, 1 floored"),
            ),
            (
                "a lowered floor against the base file",
                Past::OnTheBase,
                Some(lowered()),
                Err("a: the floor fell from 0.83 to 0.80"),
            ),
            (
                "the same file against the base file",
                Past::OnTheBase,
                Some(carried()),
                Ok("2 crates, 1 floored"),
            ),
            (
                "a head that deletes the file",
                Past::OnTheBase,
                None,
                Err("missing at the head"),
            ),
        ];
        judge_rows_in_parallel(&rows, |(name, past, head_file, expected)| {
            let repo = Repo::new("floors-history");
            repo.init();
            seed(&repo);
            match past {
                Past::Never => {}
                Past::Removed => {
                    repo.write(FLOORS_FILE, &carried());
                    repo.commit("carry the floors");
                    repo.remove(FLOORS_FILE);
                }
                Past::OnTheBase => repo.write(FLOORS_FILE, &carried()),
            }
            repo.commit("base");
            let base = repo.sha();
            match head_file {
                Some(text) => repo.write(FLOORS_FILE, text),
                None if repo.0.join(FLOORS_FILE).exists() => repo.remove(FLOORS_FILE),
                None => {}
            }
            repo.write("crates/a/src/lib.rs", "pub fn a() { changed(); }\n");
            repo.commit("head");
            let found = check(&repo.0, &base, &Head::Commit(repo.sha()));
            let ok = match (&found, expected) {
                (Ok(detail), Ok(part)) => detail.contains(part),
                (Err(message), Err(part)) => message.contains(part),
                _ => false,
            };
            (!ok).then(|| format!("{name}: expected {expected:?}, found {found:?}"))
        });
    }

    pub(crate) fn workspace_of(names: &[&str]) -> Workspace {
        Workspace {
            packages: names
                .iter()
                .map(|name| crate::metadata::Package {
                    name: (*name).to_owned(),
                    dir: format!("crates/{name}"),
                    edition: "2024".to_owned(),
                    dependencies: Vec::new(),
                    targets: Vec::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn every_workspace_package_has_one_floor_entry() {
        let entries = |names: &[&str]| -> Floors {
            parse(&names.iter().map(|n| unmeasured(n)).collect::<String>()).unwrap()
        };
        type Case<'a> = (&'a str, &'a [&'a str], &'a [&'a str], Vec<&'a str>);
        let rows: Vec<Case> = vec![
            ("a complete file", &["a", "b"], &["a", "b"], vec![]),
            (
                "a package without an entry",
                &["a", "b"],
                &["a"],
                vec!["b: the workspace package has no entry"],
            ),
            (
                "an entry for a package outside the workspace",
                &["a"],
                &["a", "gone"],
                vec![
                    "gone: quality/floors.toml has an entry for a package that is not in the workspace",
                ],
            ),
            (
                "both offenders are named",
                &["a", "new"],
                &["a", "gone"],
                vec![
                    "new: the workspace package",
                    "gone: quality/floors.toml has an entry",
                ],
            ),
        ];
        let mut wrong = Vec::new();
        for (name, packages, listed, expected) in rows {
            let found = completeness_problems(&entries(listed), &workspace_of(packages));
            let ok = found.len() == expected.len()
                && found.iter().zip(&expected).all(|(f, e)| f.contains(e));
            if !ok {
                wrong.push(format!("{name}: expected {expected:?}, found {found:?}"));
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));

        let build = |carried: bool, head_file: &str| {
            let repo = Repo::new("floors-complete");
            repo.init();
            seed(&repo);
            if carried {
                repo.write(FLOORS_FILE, &unmeasured("a"));
            }
            repo.commit("base");
            let base = repo.sha();
            repo.write(FLOORS_FILE, head_file);
            repo.commit("head");
            let found = completeness(
                &repo.0,
                &base,
                &Head::Commit(repo.sha()),
                &workspace_of(&["a", "b"]),
            );
            (repo, found)
        };
        let (_repo, found) = build(true, &unmeasured("a"));
        let message = found.unwrap_err();
        assert!(
            message.contains("b: the workspace package has no entry"),
            "{message}"
        );
        let (_repo, found) = build(false, &unmeasured("a"));
        let note = found.unwrap();
        assert!(
            note.contains("not in force yet")
                && note.contains("1 entries incomplete")
                && note.contains("b:"),
            "{note}"
        );
        let both = format!("{}{}", unmeasured("a"), unmeasured("b"));
        let (_repo, found) = build(true, &both);
        assert_eq!(found.unwrap(), "2 floor entries complete");
    }
}
