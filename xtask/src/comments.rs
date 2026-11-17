use crate::c_comments::{self, Syntax};
use crate::checks::Outcome;
use crate::scope::Head;
use crate::{hash_comments, markup_comments, source};
use ra_ap_rustc_lexer::{FrontmatterAllowed, TokenKind};
use std::ops::Range;
use std::path::Path;

const SHOWN: usize = 20;

const GENERATED: [&str; 4] = [
    "Cargo.lock",
    "gradlew",
    "gradlew.bat",
    "gradle-wrapper.properties",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Language {
    Rust,
    C(Syntax),
    Toml,
    Python,
    Yaml,
    Properties,
    Shell,
    Markup,
    Astro,
}

fn language(path: &str, text: &str) -> Option<Language> {
    let name = source::file_name(path);
    let extension = name.rsplit_once('.').map(|(_, extension)| extension);
    let language = match extension {
        Some("rs") => Language::Rust,
        Some("wgsl" | "wesl") => Language::C(c_comments::SHADER),
        Some("java" | "gradle" | "groovy") => Language::C(c_comments::JAVA),
        Some("kt" | "kts") => Language::C(c_comments::KOTLIN),
        Some("js" | "mjs" | "cjs" | "ts" | "mts" | "jsx" | "tsx") => {
            Language::C(c_comments::SCRIPT)
        }
        Some("css") => Language::C(c_comments::CSS),
        Some("html" | "htm" | "xml" | "svg") => Language::Markup,
        Some("astro") => Language::Astro,
        Some("toml") => Language::Toml,
        Some("py") => Language::Python,
        Some("yml" | "yaml") => Language::Yaml,
        Some("properties") => Language::Properties,
        Some("sh" | "bash" | "zsh") => Language::Shell,
        Some(_) => return None,
        None => {
            let shebang = text.lines().next().filter(|line| line.starts_with("#!"))?;
            if shebang.contains("python") {
                Language::Python
            } else if shebang.contains("node") {
                Language::C(c_comments::SCRIPT)
            } else if shebang.contains("sh") {
                Language::Shell
            } else {
                return None;
            }
        }
    };
    Some(language)
}

fn is_exempt(path: &str) -> bool {
    path.split('/').any(|segment| segment == "examples")
        || GENERATED.contains(&source::file_name(path))
}

fn code_files(repo: &Path, head: &Head) -> Result<source::Files, String> {
    let mut paths = source::files(repo, head)?;
    paths.retain(|path| {
        let name = source::file_name(path);
        !is_exempt(path)
            && (!name.contains('.') || language(path, "").is_some())
            && (*head != Head::Worktree || repo.join(path).is_file())
    });
    source::read_files(repo, head, &paths)
}

fn rust(text: &str) -> Vec<Range<usize>> {
    let mut at = ra_ap_rustc_lexer::strip_shebang(text).unwrap_or(0);
    let mut found = Vec::new();
    for token in ra_ap_rustc_lexer::tokenize(text.get(at..).unwrap_or(""), FrontmatterAllowed::Yes)
    {
        let end = at + token.len as usize;
        if matches!(
            token.kind,
            TokenKind::LineComment { .. } | TokenKind::BlockComment { .. }
        ) {
            let end = if text.get(end - 1..end) == Some("\r") {
                end - 1
            } else {
                end
            };
            found.push(at..end);
        }
        at = end;
    }
    found
}

fn is_plain_line_comment(comment: &str) -> bool {
    comment.starts_with("//") && !comment.starts_with("///") && !comment.starts_with("//!")
}

fn guards_unsafe(text: &str, start: usize, end: usize) -> bool {
    let before = text.get(line_start(text, start)..start).unwrap_or("");
    if before.contains("unsafe") {
        return true;
    }
    text.get(end..)
        .unwrap_or("")
        .lines()
        .skip(1)
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("//") && !line.starts_with("#["))
        .is_some_and(|line| line.contains("unsafe"))
}

fn without_safety_notes(text: &str, ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    let mut kept = Vec::with_capacity(ranges.len());
    let mut note: Vec<Range<usize>> = Vec::new();
    let close = |note: &mut Vec<Range<usize>>, kept: &mut Vec<Range<usize>>| {
        if let (Some(first), Some(last)) = (note.first(), note.last())
            && !guards_unsafe(text, first.start, last.end)
        {
            kept.append(note);
        }
        note.clear();
    };
    for range in ranges {
        let comment = text.get(range.clone()).unwrap_or("");
        let continues = note.last().is_some_and(|last| {
            text.get(last.end..range.start)
                .is_some_and(|gap| gap.trim().is_empty() && gap.matches('\n').count() == 1)
        });
        if comment.starts_with("// SAFETY:") {
            close(&mut note, &mut kept);
            note.push(range);
        } else if continues && is_plain_line_comment(comment) {
            note.push(range);
        } else {
            close(&mut note, &mut kept);
            kept.push(range);
        }
    }
    close(&mut note, &mut kept);
    kept.sort_by_key(|range| range.start);
    kept
}

fn comments(path: &str, text: &str) -> Vec<Range<usize>> {
    match language(path, text) {
        None => Vec::new(),
        Some(Language::Rust) => without_safety_notes(text, rust(text)),
        Some(Language::C(syntax)) => c_comments::comments(text, syntax),
        Some(Language::Toml) => hash_comments::toml(text),
        Some(Language::Python) => hash_comments::python(text),
        Some(Language::Yaml) => hash_comments::yaml(text),
        Some(Language::Properties) => hash_comments::properties(text),
        Some(Language::Shell) => hash_comments::shell(text),
        Some(Language::Markup) => markup_comments::comments(text, false),
        Some(Language::Astro) => markup_comments::comments(text, true),
    }
}

fn line_start(text: &str, at: usize) -> usize {
    text.get(..at)
        .and_then(|before| before.rfind('\n'))
        .map_or(0, |newline| newline + 1)
}

fn line_end(text: &str, at: usize) -> usize {
    text.get(at..)
        .and_then(|after| after.find('\n'))
        .map_or(text.len(), |offset| at + offset)
}

fn is_blank(text: &str, range: Range<usize>) -> bool {
    text.get(range).is_none_or(|part| part.trim().is_empty())
}

fn after_line(text: &str, end: usize) -> usize {
    (end + 1).min(text.len())
}

fn strip(text: &str, ranges: &[Range<usize>]) -> String {
    let mut out = text.to_owned();
    for range in ranges.iter().rev() {
        let start = line_start(&out, range.start);
        let end = line_end(&out, range.end);
        let alone_before = is_blank(&out, start..range.start);
        let alone_after = is_blank(&out, range.end..end);
        if alone_before && alone_after {
            let mut cut_end = after_line(&out, end);
            let blank_above = start == 0 || {
                let above = line_start(&out, start - 1);
                is_blank(&out, above..start)
            };
            if blank_above && cut_end < out.len() {
                let next_end = line_end(&out, cut_end);
                if is_blank(&out, cut_end..next_end) {
                    cut_end = after_line(&out, next_end);
                }
            }
            out.replace_range(start..cut_end, "");
        } else if alone_after {
            let kept = out.get(..range.start).map_or(range.start, |before| {
                before.trim_end_matches([' ', '\t']).len()
            });
            out.replace_range(kept..range.end, "");
        } else if alone_before {
            let tail = out.get(range.end..).map_or(0, |after| {
                after.len() - after.trim_start_matches([' ', '\t']).len()
            });
            out.replace_range(range.start..range.end + tail, "");
        } else {
            let left = out.get(..range.start).and_then(|s| s.chars().next_back());
            let right = out.get(range.end..).and_then(|s| s.chars().next());
            let spaced = |c: Option<char>| c.is_none_or(char::is_whitespace);
            if !spaced(left) && !spaced(right) {
                out.replace_range(range.clone(), " ");
            } else if spaced(right) && spaced(left) {
                let tail = out.get(range.end..).map_or(0, |after| {
                    after.len() - after.trim_start_matches([' ', '\t']).len()
                });
                out.replace_range(range.start..range.end + tail, "");
            } else if spaced(right) || right.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                out.replace_range(range.clone(), "");
            } else {
                let kept = out.get(..range.start).map_or(range.start, |before| {
                    before.trim_end_matches([' ', '\t']).len()
                });
                out.replace_range(kept..range.end, "");
            }
        }
    }
    if out.trim().is_empty() {
        out.clear();
    }
    out
}

fn line_number(text: &str, at: usize) -> usize {
    text.get(..at)
        .map_or(0, |before| before.matches('\n').count())
        + 1
}

pub fn check(repo: &Path, head: &Head) -> Outcome {
    let files = code_files(repo, head)?;
    let mut found = Vec::new();
    for (path, text) in &files {
        for range in comments(path, text) {
            found.push(format!("{path}:{}", line_number(text, range.start)));
        }
    }
    if found.is_empty() {
        return Ok(String::new());
    }
    let mut message = format!(
        "{} comments outside examples/ other than // SAFETY: notes; remove them, or run cargo run -p xtask -- strip-comments:",
        found.len()
    );
    for at in found.iter().take(SHOWN) {
        message.push_str("\n  ");
        message.push_str(at);
    }
    if found.len() > SHOWN {
        message.push_str(&format!("\n  and {} more", found.len() - SHOWN));
    }
    Err(message)
}

pub fn strip_tree(repo: &Path) -> Result<usize, String> {
    let files = code_files(repo, &Head::Worktree)?;
    let mut changed = 0;
    for (path, text) in &files {
        let ranges = comments(path, text);
        if ranges.is_empty() {
            continue;
        }
        std::fs::write(repo.join(path), strip(text, &ranges))
            .map_err(|e| format!("could not write {path}: {e}"))?;
        changed += 1;
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stripped(path: &str, text: &str) -> String {
        strip(text, &comments(path, text))
    }

    #[test]
    fn each_language_loses_its_comments_and_keeps_look_alikes_in_strings() {
        let cases = [
            (
                "a.rs",
                "//! crate\n/// item\nfn f() -> &'static str { /* x */ r#\"// no\"# } // tail\nconst C: char = '\"';\n",
                "fn f() -> &'static str { r#\"// no\"# }\nconst C: char = '\"';\n",
            ),
            (
                "b.rs",
                "// SAFETY: one\n// two\nunsafe { f() }\n// three\n\n// SAFETY: four\n#[inline]\nunsafe fn g() {}\n// SAFETY: free\nfn h() {}\nlet x = unsafe { y() }; // SAFETY: tail\n",
                "// SAFETY: one\n// two\nunsafe { f() }\n\n// SAFETY: four\n#[inline]\nunsafe fn g() {}\nfn h() {}\nlet x = unsafe { y() }; // SAFETY: tail\n",
            ),
            (
                "a.wgsl",
                "/* a /* nested */ b */\nlet x = 1; // y\n",
                "let x = 1;\n",
            ),
            (
                "A.java",
                "/** doc */\nString s = \"\"\"\n  // kept\n  \"\"\"; char c = '/';\n",
                "String s = \"\"\"\n  // kept\n  \"\"\"; char c = '/';\n",
            ),
            (
                "a.mjs",
                "const r = /\\/\\//g; // c\nconst t = `${a /* in */}// text`;\nx = a / b; /* d */\n",
                "const r = /\\/\\//g;\nconst t = `${a}// text`;\nx = a / b;\n",
            ),
            (
                "c.mjs",
                "x = a/* c */ b;\nreturn/* c */ v;\n",
                "x = a b;\nreturn v;\n",
            ),
            (
                "b.astro",
                "<p>{a && <b>x</b>}</p>\n<p>see https://example.com</p>\n",
                "<p>{a && <b>x</b>}</p>\n<p>see https://example.com</p>\n",
            ),
            ("b.sh", "echo $((1 << 2))\n# c\n", "echo $((1 << 2))\n"),
            (
                "b.mjs",
                "return /[/]/.test(s); // c\n",
                "return /[/]/.test(s);\n",
            ),
            (
                "a.css",
                "a { background: url(http://x); } /* c */\n",
                "a { background: url(http://x); }\n",
            ),
            (
                "a.toml",
                "# head\nkey = \"a # b\" # tail\nmulti = '''\n# kept\n'''\n",
                "key = \"a # b\"\nmulti = '''\n# kept\n'''\n",
            ),
            (
                "a.py",
                "#!/usr/bin/env python3\n# c\nx = '#'  # t\n",
                "#!/usr/bin/env python3\nx = '#'\n",
            ),
            (
                "a.yml",
                "# c\nkey: 'a # b' # t\nrun: |\n  # kept\n  echo\nurl: a#b\n",
                "key: 'a # b'\nrun: |\n  # kept\n  echo\nurl: a#b\n",
            ),
            (
                "a.properties",
                "# c\n! d\nurl=http://x#y\n",
                "url=http://x#y\n",
            ),
            (
                "run",
                "#!/bin/sh\n# c\necho \"$#\" ${x#y} '#' a#b # t\ncat <<'EOF'\n# kept\nEOF\n",
                "#!/bin/sh\necho \"$#\" ${x#y} '#' a#b\ncat <<'EOF'\n# kept\nEOF\n",
            ),
            (
                "a.html",
                "<!-- c -->\n<a href=\"//x\">y</a>\n<script>let a = 1; // c\n</script>\n",
                "<a href=\"//x\">y</a>\n<script>let a = 1;\n</script>\n",
            ),
            (
                "a.astro",
                "---\n// c\nconst a = 1;\n---\n{/* gone */}\n<p>{a /* c */}</p>\n",
                "---\nconst a = 1;\n---\n<p>{a}</p>\n",
            ),
            (
                "examples/a.rs",
                "// kept in an example\n",
                "// kept in an example\n",
            ),
        ];
        for (path, input, expected) in cases {
            let actual = if is_exempt(path) {
                input.to_owned()
            } else {
                stripped(path, input)
            };
            assert_eq!(actual, expected, "{path}");
        }
    }

    #[test]
    fn stripping_leaves_one_blank_line_and_separates_joined_tokens() {
        let input = "a\n\n// one\n// two\n\nb /* x */c\n";
        assert_eq!(stripped("a.rs", input), "a\n\nb c\n");
    }
}
