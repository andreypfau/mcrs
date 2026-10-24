use crate::source::join;
use serde::Deserialize;
use std::collections::HashSet;

pub const OUTSIDE_THE_LOADER: &str =
    "fixture path outside the loader: load the fixture through mcrs_minecraft_fixtures";

const OWNERS: [&str; 2] = [
    "tools/mcrs_minecraft_fixtures/",
    "tools/mcrs_minecraft_update/",
];

#[derive(Deserialize)]
struct Entry {
    name: String,
    outputs: Vec<Output>,
}

#[derive(Deserialize)]
struct Output {
    file: Option<String>,
    pattern: Option<String>,
    to: String,
}

#[derive(Default)]
pub struct Routes {
    named: HashSet<String>,
    directories: HashSet<String>,
    patterns: Vec<(String, String)>,
    endings: Vec<String>,
}

impl Routes {
    pub fn parse(sides: &[(&str, Option<&str>)]) -> Result<Routes, String> {
        let mut routes = Routes::default();
        for (side, text) in sides {
            let Some(text) = text else { continue };
            let entries: Vec<Entry> = serde_json::from_str(text)
                .map_err(|e| format!("{} at {side} is malformed: {e}", super::TABLE))?;
            for entry in entries {
                for output in entry.outputs {
                    routes
                        .add(&entry.name, &output)
                        .map_err(|problem| format!("{} at {side}: {problem}", super::TABLE))?;
                }
            }
        }
        Ok(routes)
    }

    fn add(&mut self, entry: &str, output: &Output) -> Result<(), String> {
        let to = join("", &output.to);
        if output.file.is_none() && output.pattern.is_none() {
            return Err(format!(
                "{entry} lists an output with neither a file nor a pattern"
            ));
        }
        self.directories.insert(to.clone());
        if let Some(file) = &output.file {
            self.named.insert(join(&to, file));
            let last = to.rsplit('/').next().unwrap_or_default();
            self.endings.push(format!("{last}/{file}"));
        }
        if let Some(pattern) = &output.pattern {
            self.patterns.push((to, pattern.clone()));
        }
        Ok(())
    }

    // chisle: each literal is judged alone, so a path joined from segments that each stop above a
    // destination directory still passes; resolving a `join` chain or `concat!` as one path lifts it.
    fn names_a_fixture(&self, literal: &str, resolved_against: &[&str]) -> bool {
        let tail = join("", literal);
        if tail.is_empty() {
            return false;
        }
        let by_ending = self
            .endings
            .iter()
            .any(|ending| tail == *ending || tail.ends_with(&format!("/{ending}")));
        by_ending
            || resolved_against
                .iter()
                .any(|base| self.is_output(&join(base, literal)))
    }

    fn is_output(&self, path: &str) -> bool {
        if self.named.contains(path) || self.directories.contains(path) {
            return true;
        }
        let (directory, name) = path.rsplit_once('/').unwrap_or(("", path));
        self.patterns
            .iter()
            .any(|(to, pattern)| to == directory && matches_pattern(pattern, name))
    }
}

fn matches_pattern(pattern: &str, name: &str) -> bool {
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or_default();
    let Some(mut rest) = name.strip_prefix(first) else {
        return false;
    };
    let mut parts: Vec<&str> = parts.collect();
    let Some(last) = parts.pop() else {
        return rest.is_empty();
    };
    for part in parts {
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }
    rest.ends_with(last)
}

fn crate_root(path: &str) -> &str {
    if let Some(inside) = path.strip_prefix("crates/")
        && let Some((directory, _)) = inside.split_once('/')
    {
        return &path[.."crates/".len() + directory.len()];
    }
    if path.starts_with("xtask/") {
        "xtask"
    } else {
        ""
    }
}

pub fn violations(routes: &Routes, path: &str, literals: &[(String, usize)]) -> Vec<usize> {
    if OWNERS.iter().any(|owner| path.starts_with(owner)) {
        return Vec::new();
    }
    let directory = crate::source::parent(path);
    let bases = [directory, crate_root(path), ""];
    literals
        .iter()
        .filter(|(literal, _)| routes.names_a_fixture(literal, &bases))
        .map(|(_, line)| *line)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pattern_matches_its_literal_parts_in_order() {
        let rows = [
            ("overworld_s*_c*.bin", "overworld_s12_c3.bin", true),
            ("overworld_s*_c*.bin", "overworld_c3_s12.bin", false),
            ("overworld_s*_c*.bin", "overworld_s12_c3.txt", false),
            ("overworld_s*_c*.bin", "x_overworld_s12_c3.bin", false),
            ("*ab*b", "ab", false),
            ("*ab*b", "abb", true),
            ("*ab*b", "xabyb", true),
            ("a*b", "ab", true),
            ("a*a", "a", false),
            ("a*", "a", true),
            ("*b", "ab", true),
            ("exact.bin", "exact.bin", true),
            ("exact.bin", "exact.bin.bak", false),
            ("exact.bin", "other.bin", false),
        ];
        for (pattern, name, expected) in rows {
            assert_eq!(matches_pattern(pattern, name), expected, "{pattern} {name}");
        }
    }
}
