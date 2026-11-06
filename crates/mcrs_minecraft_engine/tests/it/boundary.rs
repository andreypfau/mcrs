//! The muxer and the coordinator talk only through the messages of `to_muxer` and
//! `to_coordinator`: neither tree names the other, nor the state that the other owns, and no
//! module outside the two trees reaches into both but the steps between two ticks.

use std::fs;
use std::io;
use std::path::Path;

const SOURCE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src");

/// The resources and components of the coordinator that live outside its tree.
const COORDINATOR_STATE: [&str; 5] = [
    "DimSpawnQueue",
    "DimDespawnQueue",
    "DimensionList",
    "WorldClocks",
    "RegistrySnapshot",
];

/// The resources and components of the muxer that live outside its tree.
const MUXER_STATE: [&str; 5] = [
    "SessionAnchors",
    "SessionPlacement",
    "PlayerSessionCounter",
    "ServerSideConnection",
    "Session",
];

/// Whether `line` holds `name` as a whole path or identifier, not as a part of a longer one.
fn names(line: &str, name: &str) -> bool {
    let is_identifier = |character: char| character.is_alphanumeric() || character == '_';
    line.match_indices(name).any(|(start, _)| {
        let before = line.get(..start).and_then(|head| head.chars().next_back());
        let after = line
            .get(start.saturating_add(name.len())..)
            .and_then(|tail| tail.chars().next());
        !before.is_some_and(is_identifier) && !after.is_some_and(is_identifier)
    })
}

fn offences(tree: &str, forbidden: &[&str]) -> io::Result<Vec<String>> {
    let directory = Path::new(SOURCE).join(tree);
    let mut found = Vec::new();
    for entry in fs::read_dir(&directory)? {
        let path = entry?.path();
        let text = fs::read_to_string(&path)?;
        for (number, line) in text.lines().enumerate() {
            for name in forbidden {
                if names(line, name) {
                    found.push(format!(
                        "{}:{}: {name}",
                        path.display(),
                        number.saturating_add(1)
                    ));
                }
            }
        }
    }
    Ok(found)
}

#[test]
fn the_muxer_names_nothing_of_the_coordinator() -> io::Result<()> {
    let forbidden: Vec<&str> = ["crate::coordinator"]
        .into_iter()
        .chain(COORDINATOR_STATE)
        .collect();
    let found = offences("muxer", &forbidden)?;
    assert!(found.is_empty(), "{}", found.join("\n"));
    Ok(())
}

#[test]
fn the_coordinator_names_nothing_of_the_muxer() -> io::Result<()> {
    let forbidden: Vec<&str> = ["crate::muxer"].into_iter().chain(MUXER_STATE).collect();
    let found = offences("coordinator", &forbidden)?;
    assert!(found.is_empty(), "{}", found.join("\n"));
    Ok(())
}

/// The modules outside the two trees that may name both: the crate root, which adds both
/// plugins, and the steps between two ticks, which let the coordinator act and then the muxer.
const GLUE: [&str; 2] = ["lib.rs", "between_ticks.rs"];

#[test]
fn no_other_module_names_both_trees() -> io::Result<()> {
    let mut found = Vec::new();
    for entry in fs::read_dir(SOURCE)? {
        let path = entry?.path();
        let glue = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| GLUE.contains(&name));
        if glue || !path.is_file() {
            continue;
        }
        let text = fs::read_to_string(&path)?;
        let both = ["crate::coordinator", "crate::muxer"]
            .iter()
            .all(|tree| text.lines().any(|line| names(line, tree)));
        if both {
            found.push(path.display().to_string());
        }
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
    Ok(())
}
