use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    #[allow(dead_code)]
    #[serde(default)]
    pub default: Option<String>,
    pub protocol_id: u16,
    pub entries: BTreeMap<String, Entry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub protocol_id: u16,
}

pub type Report = BTreeMap<String, Registry>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub kind: &'static str,
    pub registry: String,
    pub entry: Option<String>,
    pub old: Option<u16>,
    pub new: Option<u16>,
}

impl Row {
    fn key(&self) -> (&str, bool, Option<u16>, Option<&str>) {
        (
            &self.registry,
            self.entry.is_some(),
            self.new.or(self.old),
            self.entry.as_deref(),
        )
    }
}

impl fmt::Display for Row {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}\t{}\t", self.kind, self.registry)?;
        match &self.entry {
            Some(entry) => write!(f, "{entry}")?,
            None => write!(f, "-")?,
        }
        for id in [self.old, self.new] {
            match id {
                Some(id) => write!(f, "\t{id}")?,
                None => write!(f, "\t-")?,
            }
        }
        Ok(())
    }
}

pub fn parse(text: &str) -> Result<Report, String> {
    serde_json::from_str(text).map_err(|error| error.to_string())
}

pub fn read(path: &Path) -> Result<Report, String> {
    let text = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    parse(&text).map_err(|error| format!("{}: {error}", path.display()))
}

pub fn diff(old: &Report, new: &Report) -> Vec<Row> {
    let mut rows = Vec::new();
    let names: BTreeSet<&String> = old.keys().chain(new.keys()).collect();
    for name in names {
        let (before, after) = (old.get(name), new.get(name));
        match (before, after) {
            (Some(before), Some(after)) if before.protocol_id != after.protocol_id => {
                rows.push(registry_row(
                    "registry_moved",
                    name,
                    Some(before.protocol_id),
                    Some(after.protocol_id),
                ));
            }
            (Some(before), None) => {
                rows.push(registry_row(
                    "registry_removed",
                    name,
                    Some(before.protocol_id),
                    None,
                ));
            }
            (None, Some(after)) => {
                rows.push(registry_row(
                    "registry_added",
                    name,
                    None,
                    Some(after.protocol_id),
                ));
            }
            _ => {}
        }
        let empty = BTreeMap::new();
        let before = before.map_or(&empty, |registry| &registry.entries);
        let after = after.map_or(&empty, |registry| &registry.entries);
        for (entry, was) in before {
            let (kind, new) = match after.get(entry) {
                Some(is) if is.protocol_id != was.protocol_id => {
                    ("entry_moved", Some(is.protocol_id))
                }
                Some(_) => continue,
                None => ("entry_removed", None),
            };
            rows.push(entry_row(kind, name, entry, Some(was.protocol_id), new));
        }
        for (entry, is) in after {
            if !before.contains_key(entry) {
                rows.push(entry_row(
                    "entry_added",
                    name,
                    entry,
                    None,
                    Some(is.protocol_id),
                ));
            }
        }
    }
    rows.sort_by(|a, b| a.key().cmp(&b.key()));
    rows
}

fn registry_row(kind: &'static str, registry: &str, old: Option<u16>, new: Option<u16>) -> Row {
    Row {
        kind,
        registry: registry.to_owned(),
        entry: None,
        old,
        new,
    }
}

fn entry_row(
    kind: &'static str,
    registry: &str,
    entry: &str,
    old: Option<u16>,
    new: Option<u16>,
) -> Row {
    Row {
        kind,
        registry: registry.to_owned(),
        entry: Some(entry.to_owned()),
        old,
        new,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(text: &str) -> Report {
        parse(text).unwrap()
    }

    fn lines(rows: &[Row]) -> Vec<String> {
        rows.iter().map(Row::to_string).collect()
    }

    #[test]
    fn rows_are_sorted_by_registry_then_id_then_entry_and_repeat_exactly() {
        let old = report(
            r#"{"a:z":{"protocol_id":1,"entries":{"a:q":{"protocol_id":2}}},
                "a:b":{"protocol_id":2,"entries":{"a:m":{"protocol_id":0},"a:n":{"protocol_id":1}}}}"#,
        );
        let new = report(
            r#"{"a:z":{"protocol_id":1,"entries":{"a:q":{"protocol_id":0}}},
                "a:b":{"protocol_id":2,"entries":{"a:n":{"protocol_id":0},"a:m":{"protocol_id":1}}}}"#,
        );
        let first = diff(&old, &new);
        assert_eq!(
            lines(&first),
            [
                "entry_moved\ta:b\ta:n\t1\t0",
                "entry_moved\ta:b\ta:m\t0\t1",
                "entry_moved\ta:z\ta:q\t2\t0",
            ]
        );
        assert_eq!(first, diff(&old, &new));
    }

    #[test]
    fn a_missing_previous_report_is_an_error_naming_the_path() {
        let path = std::env::temp_dir().join("mcrs-update-no-such-report.json");
        let error = read(&path).unwrap_err();
        assert!(error.contains(&path.display().to_string()), "{error}");
    }

    #[test]
    fn an_unknown_member_fails_to_parse() {
        assert!(parse(r#"{"a:r":{"protocol_id":3,"entries":{},"extra":1}}"#).is_err());
        assert!(
            parse(r#"{"a:r":{"protocol_id":3,"entries":{"a:x":{"protocol_id":0,"extra":1}}}}"#)
                .is_err()
        );
    }
}
