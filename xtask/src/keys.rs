use crate::checks::Outcome;
use crate::scope::{self, Head};
use std::path::Path;

pub(crate) const HEADER: &str =
    "// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.";

pub(crate) const MARKED: [&str; 3] = [
    "crates/*/src/keys/**",
    "crates/mcrs_minecraft_keys/src/**",
    "crates/mcrs_minecraft_registry_catalog/src/**",
];

pub(crate) const GENERATE: [&str; 6] = [
    "run",
    "--locked",
    "-p",
    "mcrs_minecraft_update",
    "--",
    "keys",
];

fn segment_matches(glob: &str, segment: &str) -> bool {
    if glob == "*" {
        !segment.is_empty()
    } else {
        glob == segment
    }
}

fn matches(glob: &str, path: &str) -> bool {
    let glob: Vec<&str> = glob.split('/').collect();
    let path: Vec<&str> = path.split('/').collect();
    let (literal, rest_needed) = match glob.split_last() {
        Some((&"**", head)) => (head, true),
        _ => (glob.as_slice(), false),
    };
    let length_fits = if rest_needed {
        path.len() > literal.len()
    } else {
        path.len() == literal.len()
    };
    length_fits
        && literal
            .iter()
            .zip(&path)
            .all(|(glob, segment)| segment_matches(glob, segment))
}

pub(crate) fn is_marked(path: &str) -> bool {
    MARKED.iter().any(|glob| matches(glob, path))
}

pub(crate) fn regenerates(paths: &[String]) -> bool {
    paths.iter().any(|path| {
        is_marked(path)
            || path.starts_with("assets/mcrs/reports/")
            || path == "assets/mcrs/registry_definition/game_rule.json"
            || path.starts_with("tools/mcrs_minecraft_update/")
            || path == "Cargo.lock"
            || path == "Cargo.toml"
            || path.ends_with("/Cargo.toml")
    })
}

fn dirty(repo: &Path) -> Result<Vec<String>, String> {
    let status = crate::git::text(repo, &["status", "--porcelain", "--untracked-files=all"])
        .map_err(|e| e.to_string())?;
    Ok(status
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}

struct Headers {
    marked: usize,
    wrong: Vec<String>,
}

fn headers(repo: &Path, head: &Head) -> Result<Headers, String> {
    let marked: Vec<String> = scope::tracked(repo, head)?
        .into_iter()
        .filter(|path| is_marked(path))
        .collect();
    let specs: Vec<String> = marked
        .iter()
        .map(|path| match head {
            Head::Commit(sha) => format!("{sha}:{path}"),
            Head::Worktree => format!(":{path}"),
        })
        .collect();
    let mut wrong = Vec::new();
    for (path, blob) in marked.iter().zip(scope::blobs(repo, &specs)?) {
        let bytes = blob.ok_or_else(|| format!("{path} is tracked but could not be read"))?;
        if !path.ends_with(".rs") {
            wrong.push(format!(
                "{path}: the key generator writes only .rs files here"
            ));
        } else if bytes.split(|&byte| byte == b'\n').next() != Some(HEADER.as_bytes()) {
            wrong.push(format!(
                "{path}: the first line is not the key generator's header"
            ));
        }
    }
    wrong.sort();
    Ok(Headers {
        marked: marked.len(),
        wrong,
    })
}

fn failure(condition: &str, lines: &[String], fix: &str) -> String {
    let mut text = vec![condition.to_owned()];
    text.extend(lines.iter().cloned());
    text.push(format!("fix: {fix}"));
    text.join("\n")
}

pub(crate) fn check(
    repo: &Path,
    head: &Head,
    changed: &[String],
    generate: impl FnOnce() -> Result<bool, String>,
) -> Outcome {
    if !regenerates(changed) {
        return Ok("no generator input or output changed; not regenerated".to_owned());
    }
    let headers = headers(repo, head)?;
    if !headers.wrong.is_empty() {
        return Err(failure(
            "these files sit under the key paths but the generator did not write them:",
            &headers.wrong,
            "only cargo run -p mcrs_minecraft_update -- keys writes under crates/*/src/keys, crates/mcrs_minecraft_keys/src and crates/mcrs_minecraft_registry_catalog/src; move hand-written files out of them",
        ));
    }
    let before = dirty(repo)?;
    if !before.is_empty() {
        return Err(failure(
            "the worktree differs from the commit, and the generator would overwrite or delete these files:",
            &before,
            "commit or stash them, then run the check again",
        ));
    }
    if !generate()? {
        return Err("the key generator failed; fix the errors above".to_owned());
    }
    let after = dirty(repo)?;
    if !after.is_empty() {
        return Err(failure(
            "regenerating the key sources changed what the commit holds:",
            &after,
            "run cargo run -p mcrs_minecraft_update -- keys and commit what it writes; never edit a generated source by hand",
        ));
    }
    Ok(format!(
        "{} marked files carry the header; regenerating the key sources changed nothing",
        headers.marked
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scope::tests::Repo;
    use std::cell::Cell;

    #[test]
    fn a_change_reaches_the_generator_only_through_its_inputs_and_outputs() {
        let rows = [
            ("crates/mcrs_minecraft_block/src/keys/block.rs", true),
            ("crates/mcrs_minecraft_keys/src/lib.rs", true),
            ("crates/mcrs_minecraft_registry_catalog/src/lib.rs", true),
            ("assets/mcrs/reports/names.json", true),
            ("tools/mcrs_minecraft_update/src/owners.rs", true),
            ("Cargo.lock", true),
            ("Cargo.toml", true),
            ("crates/mcrs_minecraft_level/Cargo.toml", true),
            ("crates/mcrs_minecraft_block/src/lib.rs", false),
            ("crates/mcrs_minecraft_block/src/keys.rs", false),
            ("crates/a/b/src/keys/x.rs", false),
            ("crates/mcrs_minecraft_keys/README.md", false),
            ("assets/mcrs/reports_old/x.json", false),
            ("tools/mcrs_minecraft_update_extra/src/lib.rs", false),
            ("docs/Cargo.toml.md", false),
            ("README.md", false),
        ];
        for (path, expected) in rows {
            assert_eq!(regenerates(&[path.to_owned()]), expected, "{path}");
        }
        assert!(!regenerates(&[]));
    }

    #[test]
    fn the_marked_paths_are_the_ones_gitattributes_and_coderabbit_mark() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let attributes = std::fs::read_to_string(root.join(".gitattributes")).unwrap();
        let coderabbit = std::fs::read_to_string(root.join(".coderabbit.yaml")).unwrap();
        for glob in MARKED {
            let line = format!("{glob} linguist-generated");
            assert!(
                attributes.lines().any(|l| l == line),
                "{glob}: .gitattributes"
            );
            let entry = format!("- \"!{glob}\"");
            assert!(
                coderabbit.lines().any(|l| l.trim() == entry),
                "{glob}: .coderabbit.yaml"
            );
        }
    }

    #[test]
    fn only_rs_files_that_open_with_the_generator_header_may_sit_under_the_marked_paths() {
        let repo = Repo::new("keys-header");
        repo.init();
        let headed = format!("{HEADER}\npub const A: u8 = 1;\n");
        repo.write("crates/a/src/keys/mod.rs", &headed);
        repo.write("crates/a/src/keys/plain.rs", "pub const A: u8 = 1;\n");
        repo.write(
            "crates/a/src/keys/altered.rs",
            &headed.replace("do not edit.", "do not change."),
        );
        repo.write("crates/a/src/keys/second.rs", &format!("\n{headed}"));
        repo.write("crates/a/src/keys/data.json", &format!("{HEADER}\n{{}}\n"));
        repo.write("crates/a/src/keys/notes.txt", "notes\n");
        repo.write("crates/a/src/lib.rs", "pub const A: u8 = 1;\n");
        repo.write("crates/mcrs_minecraft_keys/src/lib.rs", &headed);
        repo.write(
            "crates/mcrs_minecraft_registry_catalog/src/extra.rs",
            "pub const A: u8 = 1;\n",
        );
        repo.commit("base");

        let found = headers(&repo.0, &Head::Commit(repo.sha())).unwrap();
        let paths: Vec<&str> = found
            .wrong
            .iter()
            .map(|line| line.split(": ").next().unwrap())
            .collect();
        assert_eq!(found.marked, 8);
        assert_eq!(
            paths,
            [
                "crates/a/src/keys/altered.rs",
                "crates/a/src/keys/data.json",
                "crates/a/src/keys/notes.txt",
                "crates/a/src/keys/plain.rs",
                "crates/a/src/keys/second.rs",
                "crates/mcrs_minecraft_registry_catalog/src/extra.rs",
            ]
        );
    }

    struct Row {
        name: &'static str,
        untracked_before: Option<&'static str>,
        written: Option<&'static str>,
        generator_succeeds: bool,
        passes: bool,
        fragment: &'static str,
        runs: bool,
    }

    #[test]
    fn regeneration_refuses_a_dirty_worktree_and_fails_on_anything_it_leaves_behind() {
        const GENERATED: &str = "crates/x/src/keys/mod.rs";
        let rows = [
            Row {
                name: "an untracked file before the run",
                untracked_before: Some("crates/x/src/keys/draft.rs"),
                written: None,
                generator_succeeds: true,
                passes: false,
                fragment: "?? crates/x/src/keys/draft.rs",
                runs: false,
            },
            Row {
                name: "a file the generator leaves untracked",
                untracked_before: None,
                written: Some("crates/x/src/keys/new.rs"),
                generator_succeeds: true,
                passes: false,
                fragment: "?? crates/x/src/keys/new.rs",
                runs: true,
            },
            Row {
                name: "a generator that exits non-zero without writing",
                untracked_before: None,
                written: None,
                generator_succeeds: false,
                passes: false,
                fragment: "the key generator failed",
                runs: true,
            },
            Row {
                name: "a generator that writes nothing",
                untracked_before: None,
                written: None,
                generator_succeeds: true,
                passes: true,
                fragment: "changed nothing",
                runs: true,
            },
        ];
        for row in rows {
            let repo = Repo::new("keys-tree");
            repo.init();
            repo.write(GENERATED, &format!("{HEADER}\n"));
            repo.commit("base");
            if let Some(path) = row.untracked_before {
                repo.write(path, "// draft\n");
            }
            let ran = Cell::new(false);
            let outcome = check(
                &repo.0,
                &Head::Commit(repo.sha()),
                &[GENERATED.to_owned()],
                || {
                    ran.set(true);
                    if let Some(path) = row.written {
                        repo.write(path, "// new\n");
                    }
                    Ok(row.generator_succeeds)
                },
            );
            assert_eq!(ran.get(), row.runs, "{}", row.name);
            let (verdict, text) = match &outcome {
                Ok(text) => (true, text),
                Err(text) => (false, text),
            };
            assert_eq!(verdict, row.passes, "{}: {text}", row.name);
            assert!(text.contains(row.fragment), "{}: {text}", row.name);
        }
    }
}
