use crate::metadata::{Package, Workspace};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const MAP_PATH: &str = "docs/crates.md";
const COMMAND: &str = "cargo run -p xtask -- crate-map";
const OTHER: &str = "Other";

const HEADER: &str = "# Crate map

This file is generated from the `description` of each crate under `crates/`.
Do not edit it. Run `cargo run -p xtask -- crate-map` to regenerate it.

Read this file before you write a helper or a value type.
Use the crate that owns the item. Do not write a second copy.
";

struct Entry<'a> {
    name: &'a str,
    description: &'a str,
}

fn short_name(name: &str) -> &str {
    name.strip_prefix("mcrs_minecraft_")
        .or_else(|| name.strip_prefix("mcrs_"))
        .unwrap_or(name)
}

fn domain(name: &str) -> &str {
    let short = short_name(name);
    short.split_once('_').map_or(short, |(domain, _)| domain)
}

fn words(text: &str) -> String {
    text.trim()
        .to_lowercase()
        .replace(['_', '-'], " ")
        .trim_end_matches('.')
        .to_owned()
}

fn crates(workspace: &Workspace) -> Vec<&Package> {
    let mut crates: Vec<&Package> = workspace
        .packages
        .iter()
        .filter(|package| package.dir.starts_with("crates/"))
        .collect();
    crates.sort_by(|a, b| a.name.cmp(&b.name));
    crates
}

pub(crate) fn render(workspace: &Workspace) -> String {
    let crates = crates(workspace);
    let mut by_domain: BTreeMap<&str, Vec<Entry>> = BTreeMap::new();
    for package in &crates {
        by_domain
            .entry(domain(&package.name))
            .or_default()
            .push(Entry {
                name: &package.name,
                description: package.description.as_deref().unwrap_or("").trim(),
            });
    }
    let mut other: Vec<Entry> = Vec::new();
    let mut sections: Vec<(&str, Vec<Entry>)> = Vec::new();
    for (name, entries) in by_domain {
        if entries.len() == 1 {
            other.extend(entries);
        } else {
            sections.push((name, entries));
        }
    }
    other.sort_by(|a, b| a.name.cmp(b.name));
    sections.push((OTHER, other));
    let mut text = String::from(HEADER);
    for (name, entries) in sections {
        if entries.is_empty() {
            continue;
        }
        text.push_str(&format!("\n## {name}\n\n"));
        for entry in entries {
            text.push_str(&format!("- `{}`: {}\n", entry.name, entry.description));
        }
    }
    text
}

pub(crate) fn problems<'a>(
    crates: impl IntoIterator<Item = (&'a str, Option<&'a str>)>,
) -> Vec<String> {
    let mut problems = Vec::new();
    let mut seen: BTreeMap<String, &str> = BTreeMap::new();
    for (name, description) in crates {
        let Some(description) = description.map(str::trim).filter(|d| !d.is_empty()) else {
            problems.push(format!(
                "crate {name} has no description; add one to its Cargo.toml"
            ));
            continue;
        };
        let text = words(short_name(description));
        if text == words(short_name(name)) {
            problems.push(format!(
                "crate {name} has a description that repeats its name; say what it owns"
            ));
        }
        if let Some(first) = seen.insert(text, name) {
            problems.push(format!(
                "crates {first} and {name} have the same description; tell them apart"
            ));
        }
    }
    problems
}

pub fn check(root: &Path) -> Result<String, String> {
    let workspace = Workspace::load(root)?;
    let crates = crates(&workspace);
    let problems = problems(
        crates
            .iter()
            .map(|package| (package.name.as_str(), package.description.as_deref())),
    );
    if !problems.is_empty() {
        return Err(problems.join("; "));
    }
    let current = fs::read_to_string(root.join(MAP_PATH)).unwrap_or_default();
    if current != render(&workspace) {
        return Err(format!("{MAP_PATH} is out of date; run {COMMAND}"));
    }
    Ok(format!("{} crates in {MAP_PATH}", crates.len()))
}

pub fn write(root: &Path) -> Result<usize, String> {
    let workspace = Workspace::load(root)?;
    let crates = crates(&workspace);
    let problems = problems(
        crates
            .iter()
            .map(|package| (package.name.as_str(), package.description.as_deref())),
    );
    if !problems.is_empty() {
        return Err(problems.join("; "));
    }
    let path = root.join(MAP_PATH);
    fs::write(&path, render(&workspace)).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(crates.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_empty_repeated_or_shared_description_is_a_problem() {
        let rows: [(&[(&str, Option<&str>)], usize); 6] = [
            (&[("mcrs_minecraft_a", Some("Block keys"))], 0),
            (&[("mcrs_minecraft_a", None)], 1),
            (&[("mcrs_minecraft_a", Some("  "))], 1),
            (
                &[(
                    "mcrs_minecraft_chunk_store",
                    Some("mcrs_minecraft_chunk_store"),
                )],
                1,
            ),
            (&[("mcrs_minecraft_chunk_store", Some("Chunk store."))], 1),
            (
                &[
                    ("mcrs_a", Some("Block keys")),
                    ("mcrs_b", Some("block keys")),
                ],
                1,
            ),
        ];
        for (crates, expected) in rows {
            let found = problems(crates.iter().copied());
            assert_eq!(found.len(), expected, "{crates:?}: {found:?}");
        }
    }
}
