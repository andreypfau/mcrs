use std::path::{Path, PathBuf};

use mcrs_minecraft_core::VERSION;

const EXTENSIONS: [&str; 4] = ["rs", "toml", "gradle", "kts"];
const FILE_NAMES: [&str; 1] = ["gradle.properties"];
const SKIPPED_DIRS: [&str; 4] = ["build", "run", "assets", "node_modules"];
const INTEGER_SUFFIXES: [&str; 12] = [
    "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize",
];

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn states_text(line: &str, value: &str) -> bool {
    let bytes = line.as_bytes();
    line.match_indices(value).any(|(start, _)| {
        let end = start + value.len();
        let before_ok = match start.checked_sub(1).map(|i| bytes[i]) {
            None => true,
            Some(b'.') => !start
                .checked_sub(2)
                .is_some_and(|i| bytes[i].is_ascii_digit()),
            Some(b) => !is_word_byte(b),
        };
        let after_ok = match bytes.get(end) {
            None => true,
            Some(b'.') => !bytes.get(end + 1).is_some_and(u8::is_ascii_digit),
            Some(b) => !is_word_byte(*b),
        };
        before_ok && after_ok
    })
}

fn states_number(line: &str, value: &str) -> bool {
    let bytes = line.as_bytes();
    let mut start = 0;
    while start < bytes.len() {
        if !is_word_byte(bytes[start]) {
            start += 1;
            continue;
        }
        let end = bytes[start..]
            .iter()
            .position(|b| !is_word_byte(*b))
            .map_or(bytes.len(), |n| start + n);
        let word = &line[start..end];
        start = end;
        let digits = INTEGER_SUFFIXES
            .iter()
            .find_map(|suffix| word.strip_suffix(suffix))
            .unwrap_or(word);
        let literal = digits.as_bytes().first().is_some_and(u8::is_ascii_digit)
            && digits.bytes().all(|b| b.is_ascii_digit() || b == b'_');
        if !literal || digits.replace('_', "") != value {
            continue;
        }
        let word_start = end - word.len();
        let after_decimal_point = word_start >= 2
            && bytes[word_start - 1] == b'.'
            && bytes[word_start - 2].is_ascii_digit();
        let before_fraction =
            bytes.get(end) == Some(&b'.') && bytes.get(end + 1).is_some_and(u8::is_ascii_digit);
        if !after_decimal_point && !before_fraction {
            return true;
        }
    }
    false
}

enum Value {
    Text(String),
    Number(String),
}

impl Value {
    fn states(&self, line: &str) -> bool {
        match self {
            Value::Text(text) => states_text(line, text),
            Value::Number(number) => states_number(line, number),
        }
    }

    fn shown(&self) -> &str {
        match self {
            Value::Text(v) | Value::Number(v) => v,
        }
    }
}

fn version_values() -> Vec<Value> {
    let mut values = vec![Value::Text(VERSION.id.clone())];
    if VERSION.name != VERSION.id {
        values.push(Value::Text(VERSION.name.clone()));
    }
    values.push(Value::Number(VERSION.protocol_version.to_string()));
    values.push(Value::Number(VERSION.world_version.to_string()));
    values
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn is_scanned(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.contains(&e))
        || path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| FILE_NAMES.contains(&n))
}

fn is_skipped_dir(name: &str) -> bool {
    SKIPPED_DIRS.contains(&name)
        || name.starts_with("target")
        || (name.starts_with('.') && name != ".cargo")
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    for entry in entries {
        let entry = entry.unwrap();
        let path = entry.path();
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            if !is_skipped_dir(&entry.file_name().to_string_lossy()) {
                collect(&path, out);
            }
        } else if kind.is_file() && is_scanned(&path) {
            out.push(path);
        }
    }
}

#[test]
fn a_value_matches_only_as_a_whole_token() {
    let text = "77.7";
    for line in [
        r#"let id = "77.7";"#,
        "assets/77.7/data",
        "77.7.json",
        "77.7-pre",
        "version 77.7.",
        "77.7",
    ] {
        assert!(states_text(line, text), "{line}");
    }
    for line in [
        "177.75", "77.70", "1.77.7", "77.7.1", "v77_7", "a77.7", "_77.7",
    ] {
        assert!(!states_text(line, text), "{line}");
    }

    let number = "4242";
    for line in [
        "4242",
        "let x = 4242;",
        "[1, 4242]",
        "42_42",
        "4_242",
        "4242u32",
        "4242_i64",
        "0..4242",
        "-4242",
        r#"= "4242""#,
    ] {
        assert!(states_number(line, number), "{line}");
    }
    for line in [
        "14242",
        "42420",
        "0x1F4242",
        "0xFF_4242",
        "4242.5",
        "0.4242",
        "FOO_4242",
        "v4242",
        "4242abc",
    ] {
        assert!(!states_number(line, number), "{line}");
    }
}

#[test]
fn no_source_file_states_a_version_value() {
    let values = version_values();
    let own = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/version_stated_once.rs")
        .canonicalize()
        .unwrap();
    let root = workspace_root().canonicalize().unwrap();

    let mut files = Vec::new();
    collect(&root, &mut files);
    files.sort();
    files.retain(|path| path.canonicalize().unwrap() != own);

    let mut offences = Vec::new();
    for path in &files {
        let text =
            std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        for (index, line) in text.lines().enumerate() {
            for value in &values {
                if value.states(line) {
                    offences.push(format!(
                        "{}:{}: states {}",
                        path.strip_prefix(&root).unwrap().display(),
                        index + 1,
                        value.shown()
                    ));
                }
            }
        }
    }

    assert!(
        !files.is_empty(),
        "no source file found under {}",
        root.display()
    );
    assert!(
        offences.is_empty(),
        "{} place(s) state a value of the corpus version file; read it from VERSION instead:\n{}",
        offences.len(),
        offences.join("\n")
    );
}
