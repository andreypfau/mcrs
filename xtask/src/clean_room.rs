use crate::scope::Head;
use crate::source;
use std::path::Path;

const EXCEPTION_PREFIXES: [&str; 3] = [
    "crates/mcrs_minecraft_worldgen",
    "crates/mcrs_minecraft_biome/",
    "crates/mcrs_minecraft_vanilla_worldgen/",
];
const JDK_CLASSES: [&str; 17] = [
    "Arrays",
    "BigDecimal",
    "BigInteger",
    "BitSet",
    "Boolean",
    "Byte",
    "Character",
    "Double",
    "DoubleStream",
    "Float",
    "Integer",
    "Long",
    "Math",
    "Random",
    "Short",
    "StrictMath",
    "String",
];
const FILE_EXTENSIONS: [&str; 6] = ["json", "lock", "md", "rs", "toml", "txt"];

pub enum Selection<'a> {
    Every,
    Changed(&'a [String]),
}

pub fn is_exception(path: &str) -> bool {
    EXCEPTION_PREFIXES
        .iter()
        .any(|prefix| path.starts_with(prefix))
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn ident_at(chars: &[char], start: usize) -> usize {
    chars
        .iter()
        .skip(start)
        .take_while(|&&c| is_ident_char(c))
        .count()
}

fn text(chars: &[char], start: usize, len: usize) -> String {
    chars.iter().skip(start).take(len).collect()
}

fn is_class(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_uppercase()) && name.contains(|c: char| c.is_lowercase())
}

/// Every member a comment names in the Java form, a class, a dot or a hash, and a member, apart
/// from the Java standard library, a URL fragment and a field of a Rust type.
pub fn named_members(comment: &str) -> Vec<String> {
    let chars: Vec<char> = comment.chars().collect();
    let mut found = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        let before = at.checked_sub(1).and_then(|i| chars.get(i)).copied();
        let len = ident_at(&chars, at);
        if len == 0 {
            at += 1;
            continue;
        }
        let class = text(&chars, at, len);
        let mut end = at + len;
        if before.is_some_and(|c| matches!(c, '.' | ':' | '/')) || !is_class(&class) {
            at = end;
            continue;
        }
        let mut members = Vec::new();
        while let Some('.' | '#') = chars.get(end) {
            let member = ident_at(&chars, end + 1);
            if member == 0 || !chars.get(end + 1).is_some_and(|c| c.is_alphabetic()) {
                break;
            }
            members.push(text(&chars, end + 1, member));
            end += 1 + member;
        }
        let is_rust_field = members
            .iter()
            .any(|m| m.contains('_') && m.starts_with(|c: char| c.is_lowercase()));
        let is_file = members.len() == 1
            && members
                .first()
                .is_some_and(|m| FILE_EXTENSIONS.contains(&m.as_str()));
        if !members.is_empty()
            && !is_file
            && !is_rust_field
            && !JDK_CLASSES.contains(&class.as_str())
        {
            found.push(text(&chars, at, end - at));
        }
        at = end;
    }
    found
}

#[derive(Clone, Copy)]
enum State {
    Code,
    Str,
    RawStr(usize),
    LineComment,
    BlockComment(usize),
}

/// The text of every comment in a Rust source, with the line it is on.
pub fn comments(source: &str) -> Vec<(usize, String)> {
    let chars: Vec<char> = source.chars().collect();
    let mut found: Vec<(usize, String)> = Vec::new();
    let mut state = State::Code;
    let mut line = 1;
    let mut at = 0;
    let next = |i: usize| chars.get(i).copied();
    while let Some(c) = next(at) {
        if c == '\n' {
            line += 1;
            if let State::LineComment = state {
                state = State::Code;
            }
            at += 1;
            continue;
        }
        let in_comment = matches!(state, State::LineComment | State::BlockComment(_));
        if in_comment && found.last().is_none_or(|(l, _)| *l != line) {
            found.push((line, String::new()));
        }
        match state {
            State::Code => match (c, next(at + 1)) {
                ('/', Some('/')) => {
                    state = State::LineComment;
                    at += 2;
                    continue;
                }
                ('/', Some('*')) => {
                    state = State::BlockComment(1);
                    at += 2;
                    continue;
                }
                ('"', _) => state = State::Str,
                ('r', Some('"' | '#')) if !next(at.wrapping_sub(1)).is_some_and(is_ident_char) => {
                    let hashes = chars.iter().skip(at + 1).take_while(|&&h| h == '#').count();
                    if next(at + 1 + hashes) == Some('"') {
                        state = State::RawStr(hashes);
                        at += 2 + hashes;
                        continue;
                    }
                }
                ('\'', Some('\\')) => {
                    let close = chars.iter().skip(at + 3).position(|&q| q == '\'');
                    at += close.map_or(1, |close| close + 4);
                    continue;
                }
                ('\'', Some(_)) if next(at + 2) == Some('\'') => {
                    at += 3;
                    continue;
                }
                _ => {}
            },
            State::Str => match c {
                '\\' => {
                    at += 2;
                    continue;
                }
                '"' => state = State::Code,
                _ => {}
            },
            State::RawStr(hashes) => {
                let closes = chars.iter().skip(at + 1).take(hashes).all(|&h| h == '#');
                if c == '"' && closes {
                    state = State::Code;
                    at += 1 + hashes;
                    continue;
                }
            }
            State::LineComment => {
                if let Some((_, text)) = found.last_mut() {
                    text.push(c);
                }
            }
            State::BlockComment(depth) => match (c, next(at + 1)) {
                ('*', Some('/')) => {
                    state = if depth == 1 {
                        State::Code
                    } else {
                        State::BlockComment(depth - 1)
                    };
                    at += 2;
                    continue;
                }
                ('/', Some('*')) => {
                    state = State::BlockComment(depth + 1);
                    at += 2;
                    continue;
                }
                _ => {
                    if let Some((_, text)) = found.last_mut() {
                        text.push(c);
                    }
                }
            },
        }
        at += 1;
    }
    found
}

pub fn violations(path: &str, source: &str) -> Vec<String> {
    if is_exception(path) {
        return Vec::new();
    }
    comments(source)
        .into_iter()
        .flat_map(|(line, comment)| {
            named_members(&comment).into_iter().map(move |name| {
                format!("{path}:{line}: names `{name}` of the game's code; state the behaviour in its own words (docs/clean-room.md)")
            })
        })
        .collect()
}

fn selected(repo: &Path, head: &Head, selection: &Selection) -> Result<Vec<String>, String> {
    match selection {
        Selection::Changed(paths) if !paths.iter().any(|p| p == "xtask/src/clean_room.rs") => {
            let mut rust: Vec<String> = paths
                .iter()
                .filter(|path| path.ends_with(".rs"))
                .cloned()
                .collect();
            rust.sort();
            rust.dedup();
            Ok(rust)
        }
        _ => source::rust_files(repo, head),
    }
}

pub fn check(repo: &Path, head: &Head, selection: Selection) -> Result<String, String> {
    let paths = selected(repo, head, &selection)?;
    let files = source::read_files(repo, head, &paths)?;
    let found: Vec<String> = files
        .iter()
        .flat_map(|(path, text)| violations(path, text))
        .collect();
    for line in &found {
        println!("{}", crate::one_line(line));
    }
    if found.is_empty() {
        Ok(format!("{} files", files.len()))
    } else {
        Err(format!(
            "{} comments name the game's code outside the world generation exception",
            found.len()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_member_of_a_game_class_is_named_and_the_standard_library_is_not() {
        let rows: [(&str, &[&str]); 11] = [
            ("`Mth.clamp(float, float, float)`", &["Mth.clamp"]),
            (
                "`BlockBehaviour.Properties.instrument`",
                &["BlockBehaviour.Properties.instrument"],
            ),
            (
                "`Timeline.TRACKS_CODEC`, then `Foo#bar`",
                &["Timeline.TRACKS_CODEC", "Foo#bar"],
            ),
            ("`Math.floorMod` and `Double.toString`", &[]),
            ("`java.util.Random.nextLong`", &[]),
            ("see Cargo.toml and README.md", &[]),
            ("[colors](https://wiki.vg/Chat#Colors)", &[]),
            ("drains `PendingInboundPartition.per_dim[entity]`", &[]),
            ("e.g. a value. Then the next", &[]),
            ("assert_eq!(BlockStateId::AIR.number(), 0);", &[]),
            ("Java 26.3 behaviour", &[]),
        ];
        for (comment, expected) in rows {
            assert_eq!(named_members(comment), expected, "{comment}");
        }
    }

    #[test]
    fn only_comments_are_read_not_strings_or_chars() {
        let source = "let a = \"// `Mth.lerp`\";\nlet b = r#\"/* `Mth.lerp` */\"#;\nlet c = '\"'; // `Mth.floor`\n/* `Camera.tick`\n   `Entity.turn` */\nfn f<'a>(x: &'a str) {} // tail\n";
        let found: Vec<(usize, String)> = comments(source)
            .into_iter()
            .filter(|(_, text)| !named_members(text).is_empty())
            .collect();
        let lines: Vec<usize> = found.iter().map(|(line, _)| *line).collect();
        assert_eq!(lines, [3, 4, 5]);
    }

    #[test]
    fn the_world_generation_exception_is_not_judged() {
        let source = "/// `Mth.lerp`.\nfn lerp() {}\n";
        assert!(violations("crates/mcrs_minecraft_worldgen_noise/src/a.rs", source).is_empty());
        assert!(violations("crates/mcrs_minecraft_biome/src/a.rs", source).is_empty());
        assert_eq!(
            violations("crates/mcrs_minecraft_biome_file/src/a.rs", source),
            [
                "crates/mcrs_minecraft_biome_file/src/a.rs:1: names `Mth.lerp` of the game's code; state the behaviour in its own words (docs/clean-room.md)"
            ]
        );
    }
}
