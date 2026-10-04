use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::corpus::{self, Report};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub kind: &'static str,
    pub file: String,
    pub path: String,
    pub old: Option<String>,
    pub new: Option<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Diff {
    pub rows: Vec<Row>,
    pub text_only: usize,
}

impl fmt::Display for Row {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let path = if self.path.is_empty() {
            "-"
        } else {
            &self.path
        };
        write!(
            f,
            "{}\t{}\t{}\t{}\t{}",
            self.kind,
            self.file,
            path,
            self.old.as_deref().unwrap_or("-"),
            self.new.as_deref().unwrap_or("-"),
        )
    }
}

impl Diff {
    pub fn summary(&self, name: &str) -> String {
        if self.rows.is_empty() && self.text_only == 0 {
            return format!("{name}: nothing changed\n");
        }
        let mut text = format!(
            "{name}: {} rows, {} files changed only in text\n",
            self.rows.len(),
            self.text_only
        );
        for row in &self.rows {
            text.push_str(&format!("{row}\n"));
        }
        text
    }
}

fn listing(dir: &Path) -> Result<BTreeSet<String>, String> {
    let mut names = Vec::new();
    corpus::files_below(dir, dir, &mut names)?;
    Ok(names.into_iter().collect())
}

fn dumped_listing(dumped: &Path) -> Result<BTreeSet<String>, String> {
    let names = listing(dumped)?;
    if names.is_empty() {
        return Err(format!("{}: the dump wrote no file", dumped.display()));
    }
    Ok(names)
}

fn escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

fn row(
    kind: &'static str,
    file: &str,
    path: &str,
    old: Option<&Value>,
    new: Option<&Value>,
) -> Row {
    Row {
        kind,
        file: file.to_owned(),
        path: path.to_owned(),
        old: old.map(ToString::to_string),
        new: new.map(ToString::to_string),
    }
}

fn walk(file: &str, path: &str, old: Option<&Value>, new: Option<&Value>, rows: &mut Vec<Row>) {
    match (old, new) {
        (Some(Value::Object(a)), Some(Value::Object(b))) => {
            let keys: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
            for key in keys {
                let path = format!("{path}/{}", escape(key));
                walk(file, &path, a.get(key), b.get(key), rows);
            }
        }
        (Some(Value::Array(a)), Some(Value::Array(b))) => {
            for index in 0..a.len().max(b.len()) {
                let path = format!("{path}/{index}");
                walk(file, &path, a.get(index), b.get(index), rows);
            }
        }
        (Some(a), Some(b)) if a != b => rows.push(row("changed", file, path, old, new)),
        (Some(_), None) => rows.push(row("removed", file, path, old, None)),
        (None, Some(_)) => rows.push(row("added", file, path, None, new)),
        _ => {}
    }
}

fn read(dir: &Path, name: &str) -> Result<Vec<u8>, String> {
    let path = dir.join(name);
    fs::read(&path).map_err(|error| corpus::io(&path, error))
}

fn parse(dir: &Path, name: &str, bytes: &[u8]) -> Result<Value, String> {
    serde_json::from_slice(bytes).map_err(|error| format!("{}: {error}", dir.join(name).display()))
}

pub fn diff(checked_in: &Path, dumped: &Path) -> Result<Diff, String> {
    let old_names = listing(checked_in)?;
    let new_names = dumped_listing(dumped)?;
    let mut diff = Diff::default();
    for name in old_names.union(&new_names) {
        let (old, new) = (old_names.contains(name), new_names.contains(name));
        if !new {
            diff.rows.push(row("file_removed", name, "", None, None));
            continue;
        }
        if !old {
            diff.rows.push(row("file_added", name, "", None, None));
            continue;
        }
        let (old_bytes, new_bytes) = (read(checked_in, name)?, read(dumped, name)?);
        if old_bytes == new_bytes {
            continue;
        }
        if !name.ends_with(".json") {
            diff.rows.push(row("text", name, "", None, None));
            continue;
        }
        let before = diff.rows.len();
        walk(
            name,
            "",
            Some(&parse(checked_in, name, &old_bytes)?),
            Some(&parse(dumped, name, &new_bytes)?),
            &mut diff.rows,
        );
        if diff.rows.len() == before {
            diff.text_only += 1;
        }
    }
    diff.rows
        .sort_by(|a, b| (&a.file, &a.path).cmp(&(&b.file, &b.path)));
    Ok(diff)
}

pub fn replace(checked_in: &Path, dumped: &Path) -> Result<Report, String> {
    let wanted = dumped_listing(dumped)?;
    let mut report = Report::default();
    for name in &wanted {
        if corpus::write_if_changed(checked_in, name, &read(dumped, name)?)? {
            report.written.push(name.clone());
        }
    }
    for name in listing(checked_in)?.difference(&wanted) {
        let path = checked_in.join(name);
        fs::remove_file(&path).map_err(|error| corpus::io(&path, error))?;
        report.deleted.push(name.clone());
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::testing::scratch;

    fn put(dir: &Path, name: &str, text: &str) {
        fs::write(dir.join(name), text).unwrap();
    }

    fn pair(name: &str, old: &[(&str, &str)], new: &[(&str, &str)]) -> (PathBuf, PathBuf) {
        let (a, b) = (
            scratch(&format!("{name}-old")),
            scratch(&format!("{name}-new")),
        );
        for (file, text) in old {
            put(&a, file, text);
        }
        for (file, text) in new {
            put(&b, file, text);
        }
        (a, b)
    }

    fn lines(diff: &Diff) -> Vec<String> {
        diff.rows.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn a_value_changed_deep_inside_a_file_yields_one_row_with_both_values() {
        let (a, b) = pair(
            "deep",
            &[(
                "oak.json",
                r#"{"block":{"components":{"movable":"push_pull"}}}"#,
            )],
            &[(
                "oak.json",
                r#"{"block":{"components":{"movable":"immovable"}}}"#,
            )],
        );

        let diff = diff(&a, &b).unwrap();

        assert_eq!(
            lines(&diff),
            ["changed\toak.json\t/block/components/movable\t\"push_pull\"\t\"immovable\""]
        );
        assert_eq!(diff.text_only, 0);
    }

    #[test]
    fn documents_that_differ_only_in_key_order_yield_no_row_and_count_as_text_only() {
        let (a, b) = pair(
            "order",
            &[
                ("x.json", r#"{"a":1,"b":{"c":2,"d":3}}"#),
                ("same.json", "{}"),
            ],
            &[
                ("x.json", r#"{"b":{"d":3,"c":2},"a":1}"#),
                ("same.json", "{}"),
            ],
        );

        let diff = diff(&a, &b).unwrap();

        assert!(diff.rows.is_empty());
        assert_eq!(diff.text_only, 1);
    }

    #[test]
    fn equal_directories_yield_no_rows_and_say_that_nothing_changed() {
        let (a, b) = pair(
            "equal",
            &[("x.json", r#"{"a":1}"#), ("README.md", "r")],
            &[("x.json", r#"{"a":1}"#), ("README.md", "r")],
        );

        let diff = diff(&a, &b).unwrap();

        assert!(diff.rows.is_empty());
        assert_eq!(diff.text_only, 0);
        assert_eq!(
            diff.summary("block_definition"),
            "block_definition: nothing changed\n"
        );
    }

    #[test]
    fn the_summary_counts_rows_and_text_only_files() {
        let (a, b) = pair(
            "summary",
            &[("x.json", r#"{"a":1}"#), ("y.json", r#"{"a":1,"b":2}"#)],
            &[("x.json", r#"{"a":2}"#), ("y.json", r#"{"b":2,"a":1}"#)],
        );

        assert_eq!(
            diff(&a, &b).unwrap().summary("item_definition"),
            "item_definition: 1 rows, 1 files changed only in text\n\
             changed\tx.json\t/a\t1\t2\n"
        );
    }

    #[test]
    fn rows_are_sorted_by_file_and_path_and_repeat_byte_for_byte() {
        let (a, b) = pair(
            "sorted",
            &[
                ("b.json", r#"{"z":1,"a":1}"#),
                ("a.json", r#"{"y":1,"x":1}"#),
            ],
            &[
                ("b.json", r#"{"z":2,"a":2}"#),
                ("a.json", r#"{"y":2,"x":2}"#),
            ],
        );

        let first = diff(&a, &b).unwrap();

        assert_eq!(
            lines(&first),
            [
                "changed\ta.json\t/x\t1\t2",
                "changed\ta.json\t/y\t1\t2",
                "changed\tb.json\t/a\t1\t2",
                "changed\tb.json\t/z\t1\t2",
            ]
        );
        assert_eq!(first, diff(&a, &b).unwrap());
        assert_eq!(first.summary("d"), diff(&a, &b).unwrap().summary("d"));
    }

    #[test]
    fn a_json_file_that_does_not_parse_is_an_error_naming_the_file() {
        let (a, b) = pair("bad", &[("x.json", "{}")], &[("x.json", "{")]);

        assert!(diff(&a, &b).unwrap_err().contains("x.json"));
    }

    #[test]
    fn a_dump_without_files_is_an_error_and_not_a_removal_of_everything() {
        let (a, b) = pair("empty-dump", &[("x.json", "{}")], &[]);

        assert!(diff(&a, &b).is_err());
        assert!(replace(&a, &b).is_err());
        assert!(a.join("x.json").exists());
    }

    #[test]
    fn replace_writes_changed_files_deletes_missing_ones_and_makes_the_directories_equal() {
        let (a, b) = pair(
            "replace",
            &[
                ("same.json", "{}"),
                ("changed.json", "old"),
                ("gone.json", "{}"),
            ],
            &[
                ("same.json", "{}"),
                ("changed.json", "new"),
                ("added.json", "{}"),
            ],
        );
        let outside = scratch("replace-outside");
        put(&outside, "keep.json", "{}");

        let report = replace(&a, &b).unwrap();

        assert_eq!(report.written, ["added.json", "changed.json"]);
        assert_eq!(report.deleted, ["gone.json"]);
        assert!(!a.join("gone.json").exists());
        assert!(outside.join("keep.json").exists());
        assert!(diff(&a, &b).unwrap().rows.is_empty());
        assert_eq!(diff(&a, &b).unwrap().text_only, 0);
        assert!(replace(&a, &b).unwrap().written.is_empty());
    }
}
