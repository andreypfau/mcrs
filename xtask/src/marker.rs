use crate::git;
use crate::scope::{self, Head};
use std::collections::BTreeSet;
use std::path::Path;

const CRATE: &str = "mutants";
const SKIP: &str = "skip";
const EXCLUDE: &str = "exclude_re";

fn listing(repo: &Path, args: &[&str], strip: Option<&str>) -> Result<Vec<String>, String> {
    let found = git::output(repo, args).map_err(|e| e.to_string())?;
    if !matches!(found.status, Some(0 | 1)) || !found.stderr.is_empty() {
        return Err(format!(
            "git grep could not search every file (status {:?}): {}",
            found.status, found.stderr
        ));
    }
    found
        .stdout
        .split(|&byte| byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| {
            let path = std::str::from_utf8(path)
                .map_err(|_| "a path that is not valid UTF-8 was found; rename it".to_owned())?;
            match strip {
                Some(prefix) => path
                    .strip_prefix(prefix)
                    .map(str::to_owned)
                    .ok_or_else(|| format!("git grep printed an unexpected path {path}")),
                None => Ok(path.to_owned()),
            }
        })
        .collect()
}

fn pending_whitespace(rest: &[u8]) -> usize {
    match rest {
        [0x09..=0x0D | 0x20, ..] => 1,
        [0xC2, 0x85, ..] => 2,
        [0xE2, 0x80, 0x8E | 0x8F | 0xA8 | 0xA9, ..] => 3,
        _ => 0,
    }
}

fn skip_trivia(bytes: &[u8], mut at: usize) -> usize {
    loop {
        let rest = &bytes[at..];
        let space = pending_whitespace(rest);
        if space > 0 {
            at += space;
        } else if rest.starts_with(b"//") {
            at += rest
                .iter()
                .position(|&byte| byte == b'\n')
                .unwrap_or(rest.len());
        } else if rest.starts_with(b"/*") {
            let mut depth = 0usize;
            let mut cursor = 0;
            while cursor < rest.len() {
                if rest[cursor..].starts_with(b"/*") {
                    depth += 1;
                    cursor += 2;
                } else if rest[cursor..].starts_with(b"*/") {
                    depth -= 1;
                    cursor += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    cursor += 1;
                }
            }
            at += cursor;
        } else {
            return at;
        }
    }
}

fn attribute_lines(bytes: &[u8]) -> Vec<usize> {
    let mut lines = Vec::new();
    let mut from = 0;
    while let Some(offset) = bytes[from..]
        .windows(CRATE.len())
        .position(|window| window == CRATE.as_bytes())
    {
        let start = from + offset;
        from = start + 1;
        let mut at = skip_trivia(bytes, start + CRATE.len());
        if !bytes[at..].starts_with(b"::") {
            continue;
        }
        at = skip_trivia(bytes, at + 2);
        if bytes[at..].starts_with(b"r#") {
            at += 2;
        }
        let named = [SKIP, EXCLUDE].into_iter().any(|name| {
            bytes[at..].starts_with(name.as_bytes())
                && !bytes
                    .get(at + name.len())
                    .is_some_and(|next| next.is_ascii_alphanumeric() || *next == b'_')
        });
        if named {
            lines.push(1 + bytes[..start].iter().filter(|&&byte| byte == b'\n').count());
        }
    }
    lines
}

fn from_working_tree(repo: &Path, paths: &[String]) -> Result<Vec<(String, Vec<u8>)>, String> {
    paths
        .iter()
        .map(|path| {
            std::fs::read(repo.join(path))
                .map(|bytes| (path.clone(), bytes))
                .map_err(|e| format!("could not read {path}: {e}; the scan cannot vouch for a file it cannot read"))
        })
        .collect()
}

fn from_git(repo: &Path, prefix: &str, paths: &[String]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let specs: Vec<String> = paths.iter().map(|path| format!("{prefix}{path}")).collect();
    scope::blobs(repo, &specs)?
        .into_iter()
        .zip(paths)
        .map(|(blob, path)| {
            blob.map(|bytes| (path.clone(), bytes)).ok_or_else(|| {
                format!("could not read {path}; the scan cannot vouch for a file it cannot read")
            })
        })
        .collect()
}

fn grep_args<'a>(extra: &[&'a str], rev: Option<&'a str>) -> Vec<&'a str> {
    let mut args = vec!["grep"];
    args.extend(extra);
    args.extend(["-a", "-l", "-z", "-F", "-e", CRATE]);
    args.extend(rev);
    args
}

pub fn check(repo: &Path, head: &Head) -> Result<String, String> {
    let files = match head {
        Head::Worktree => {
            let working = listing(repo, &grep_args(&[], None), None)?;
            let staged = listing(repo, &grep_args(&["--cached"], None), None)?;
            let mut files = from_working_tree(repo, &working)?;
            files.extend(from_git(repo, ":", &staged)?);
            files
        }
        Head::Commit(sha) => {
            let found = listing(repo, &grep_args(&[], Some(sha)), Some(&format!("{sha}:")))?;
            from_git(repo, &format!("{sha}:"), &found)?
        }
    };
    let hits: BTreeSet<(&str, usize)> = files
        .iter()
        .flat_map(|(path, bytes)| {
            attribute_lines(bytes)
                .into_iter()
                .map(move |line| (path.as_str(), line))
        })
        .collect();
    if hits.is_empty() {
        return Ok(String::new());
    }
    let places: Vec<String> = hits
        .into_iter()
        .map(|(path, line)| format!("{path}:{line}"))
        .collect();
    Err(format!(
        "the mutation-testing skip attribute appears at {}; remove it, because mutation exclusions belong in the mutants configuration",
        places.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scope::tests::Repo;
    use std::os::unix::fs::PermissionsExt;

    const SEP: &str = "::";

    fn form(template: &str) -> Vec<u8> {
        template
            .replace("{M}", CRATE)
            .replace("{S}", SKIP)
            .replace("{X}", EXCLUDE)
            .replace("{::}", SEP)
            .into_bytes()
    }

    const PREFIX: &str = "fn f() {}\n";

    fn forms() -> Vec<(&'static str, &'static str, String)> {
        let at_line_two = |body: &str| format!("{PREFIX}{body}");
        vec![
            (
                "the plain attribute",
                "src/a.rs",
                at_line_two("#[{M}{::}{S}]\nfn g() {}\n"),
            ),
            (
                "the spaced form",
                "src/a.rs",
                at_line_two("#[ {M} {::} {S} ]\n"),
            ),
            (
                "the cfg_attr form",
                "src/a.rs",
                at_line_two("#[cfg_attr(test, {M}{::}{S})]\n"),
            ),
            (
                "the inner attribute",
                "src/a.rs",
                at_line_two("#![{M}{::}{S}]\n"),
            ),
            (
                "the exclude_re form",
                "src/a.rs",
                at_line_two("#[{M}{::}{X}(\"x\")]\n"),
            ),
            (
                "a form split across lines",
                "src/a.rs",
                at_line_two("#[{M}\n{::}\n{S}]\n"),
            ),
            (
                "a block comment between the tokens",
                "src/a.rs",
                at_line_two("#[{M}/* c */{::}/* c */{S}]\n"),
            ),
            (
                "a nested block comment between the tokens",
                "src/a.rs",
                at_line_two("#[{M}/* a /* b */ c */{::}{S}]\n"),
            ),
            (
                "a line comment between the tokens",
                "src/a.rs",
                at_line_two("#[{M} // c\n{::} // d\n{S}]\n"),
            ),
            (
                "a leading path separator",
                "src/a.rs",
                at_line_two("#[{::}{M}{::}{S}]\n"),
            ),
            (
                "carriage return line endings",
                "src/a.rs",
                "fn f() {}\r\n#[{M}\r\n{::}\r\n{S}]\r\n".to_owned(),
            ),
            (
                "a NUL byte in a comment before the attribute",
                "src/a.rs",
                "// a\0b\n#[{M}{::}{S}]\n".to_owned(),
            ),
            (
                "a left-to-right mark around the separator",
                "src/a.rs",
                at_line_two("#[{M}\u{200E}{::}\u{200E}{S}]\n"),
            ),
            (
                "a next-line character around the separator",
                "src/a.rs",
                at_line_two("#[{M}\u{0085}{::}\u{0085}{S}]\n"),
            ),
            (
                "a vertical tab around the separator",
                "src/a.rs",
                at_line_two("#[{M}\u{000B}{::}\u{000B}{S}]\n"),
            ),
            (
                "a raw identifier",
                "src/a.rs",
                at_line_two("#[{M}{::}r#{S}]\n"),
            ),
            (
                "a file name with a space and a letter outside ASCII",
                "src/é file.rs",
                at_line_two("#[{M}{::}{S}]\n"),
            ),
        ]
    }

    #[derive(Clone, Copy, Debug)]
    enum Place {
        Commit,
        Staged,
        WorkingTreeOnly,
        IndexOnly,
    }

    fn setup(label: &str, place: Place, files: &[(&str, Vec<u8>)]) -> (Repo, Head) {
        let repo = Repo::new(label);
        repo.init();
        repo.write("README.md", "readme\n");
        let clean: Vec<(&str, Vec<u8>)> = files
            .iter()
            .map(|(path, _)| (*path, b"fn clean() {}\n".to_vec()))
            .collect();
        match place {
            Place::Commit => {
                repo.commit("base");
                repo.git(&["checkout", "--quiet", "-b", "topic"]);
                for (path, bytes) in files {
                    repo.write_bytes(path, bytes);
                }
                repo.commit("topic");
                let head = Head::Commit(repo.sha());
                repo.git(&["checkout", "--quiet", "main"]);
                (repo, head)
            }
            Place::Staged => {
                repo.commit("base");
                for (path, bytes) in files {
                    repo.write_bytes(path, bytes);
                }
                repo.git(&["add", "--all"]);
                (repo, Head::Worktree)
            }
            Place::WorkingTreeOnly => {
                for (path, bytes) in &clean {
                    repo.write_bytes(path, bytes);
                }
                repo.commit("base");
                for (path, bytes) in files {
                    repo.write_bytes(path, bytes);
                }
                (repo, Head::Worktree)
            }
            Place::IndexOnly => {
                repo.commit("base");
                for (path, bytes) in files {
                    repo.write_bytes(path, bytes);
                }
                repo.git(&["add", "--all"]);
                for (path, _) in files {
                    repo.remove(path);
                }
                (repo, Head::Worktree)
            }
        }
    }

    #[test]
    fn the_mutation_marker_fails_in_every_form_and_place() {
        for (name, path, template) in forms() {
            for place in [Place::Commit, Place::Staged] {
                let (repo, head) = setup("marker", place, &[(path, form(&template))]);
                let message = check(&repo.0, &head).unwrap_err();
                assert!(
                    message.contains(&format!("{path}:2")),
                    "{name} ({place:?}): {message}"
                );
            }
        }

        let marker = form("fn f() {}\n#[{M}{::}{S}]\n");
        for place in [Place::WorkingTreeOnly, Place::IndexOnly] {
            let (repo, head) = setup("marker", place, &[("src/a.rs", marker.clone())]);
            let message = check(&repo.0, &head).unwrap_err();
            assert!(message.contains("src/a.rs:2"), "{place:?}: {message}");
        }
    }

    #[test]
    fn a_candidate_that_cannot_be_read_fails_the_check_and_is_named() {
        let marker = form("fn f() {}\n#[{M}{::}{S}]\n");
        let (repo, head) = setup(
            "unreadable",
            Place::WorkingTreeOnly,
            &[("src/a.rs", marker)],
        );
        let file = repo.0.join("src/a.rs");
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000)).unwrap();
        let found = check(&repo.0, &head);
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        let message = found.unwrap_err();
        assert!(message.contains("src/a.rs"), "{message}");
    }

    #[test]
    fn a_tree_without_the_attribute_passes_even_beside_its_parts() {
        let harmless = form(
            "use {M};\nlet {M} = 1; // {S} {::}\nlet a = {M}{::}other();\nlet b = {M}{::}{S}ped();\n",
        );
        for place in [Place::Commit, Place::Staged] {
            let (repo, head) = setup("clean", place, &[("src/ok.rs", harmless.clone())]);
            assert_eq!(check(&repo.0, &head), Ok(String::new()), "{place:?}");
        }
    }

    #[test]
    fn a_failure_lists_each_match_as_path_and_line_sorted_and_without_repeats() {
        let first = form("x\ny\n#[{M}{::}{S}]\n");
        let second = form("#[{M}{::}{S}]\n#[{M}{::}{X}(\"x\")] #[{M}{::}{S}]\n");
        let (repo, head) = setup(
            "sorted",
            Place::Commit,
            &[("src/b.rs", first), ("src/a.rs", second)],
        );
        let message = check(&repo.0, &head).unwrap_err();
        assert!(
            message.contains("src/a.rs:1, src/a.rs:2, src/b.rs:3;"),
            "{message}"
        );
    }

    #[test]
    fn a_head_that_is_not_in_the_repository_fails_closed() {
        let repo = Repo::new("missing");
        repo.init();
        repo.write("a.txt", "a\n");
        repo.commit("one");
        let message = check(&repo.0, &Head::Commit("0".repeat(40))).unwrap_err();
        assert!(message.contains("git grep"), "{message}");
    }
}
