//! Oracle fixtures by name. `fixture(name)` resolves a name in
//! `tools/fixtures.json`, requires `tools/captures.json` to record it at the
//! corpus version, and reads it with `text`, `bytes`, `file`, `files` or
//! `dump(magic)`, which also checks a binary dump's header.
//!
//! A parity file is a `.rs` file whose stem ends in `_parity` or is `parity`.
//! Every test in it loads a fixture and takes each expected value from the
//! fixture or from the code under test. Nothing else in the workspace names a
//! fixture file.
#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "an application has no caller to return an error to"
)]

mod error;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use mcrs_minecraft_client_jar::{ClientJar, Progress, vanilla_data};
use mcrs_minecraft_core::VERSION;
use serde::Deserialize;

pub use error::FixtureError;

pub const MANIFEST: &str = "tools/captures.json";
const TABLE_FILE: &str = "tools/fixtures.json";
const GOLDEN_TASK: &str = "dumpGolden";
const MAGIC_LEN: usize = 8;
const FORMAT_LEN: usize = 4;
const HEADER_LEN: usize = 16;
const DUMP_FORMAT: u32 = 1;

type Manifest = BTreeMap<String, String>;

/// The pinned client jar's `data/minecraft` tree and `version.json`, laid out as the
/// `minecraft` directory of a data pack root. The jar must be installed; nothing is downloaded.
pub fn vanilla_dir() -> &'static Path {
    static DIR: LazyLock<PathBuf> = LazyLock::new(|| {
        vanilla_data(&ClientJar::Installed, None, &Progress::default()).unwrap_or_else(|error| {
            panic!(
                "the vanilla data pack is unavailable: {error}; \
                 `cargo run -p mcrs_minecraft_client_jar --example fetch` installs it"
            )
        })
    });
    &DIR
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Files {
    Named(String),
    Matching(String),
}

impl Files {
    pub fn matches(&self, file_name: &str) -> bool {
        match self {
            Files::Named(name) => name == file_name,
            Files::Matching(pattern) => matches_pattern(pattern, file_name),
        }
    }

    fn describe(&self) -> &str {
        match self {
            Files::Named(text) | Files::Matching(text) => text,
        }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub files: Files,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "RawEntry")]
pub struct Entry {
    pub name: String,
    pub task: String,
    pub outputs: Vec<Output>,
}

impl Entry {
    pub fn is_golden(&self) -> bool {
        self.task == GOLDEN_TASK
    }

    fn output_names(&self) -> Vec<String> {
        self.outputs
            .iter()
            .map(|output| output.files.describe().to_owned())
            .collect()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    name: String,
    task: String,
    outputs: Vec<RawOutput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOutput {
    file: Option<String>,
    pattern: Option<String>,
    to: String,
}

impl TryFrom<RawEntry> for Entry {
    type Error = String;

    fn try_from(raw: RawEntry) -> Result<Self, String> {
        let mut outputs = Vec::with_capacity(raw.outputs.len());
        for output in raw.outputs {
            let files = match (output.file, output.pattern) {
                (Some(file), None) => Files::Named(file),
                (None, Some(pattern)) => Files::Matching(pattern),
                _ => {
                    return Err(format!(
                        "fixture {}: an output must name exactly one of file and pattern",
                        raw.name
                    ));
                }
            };
            outputs.push(Output {
                files,
                to: output.to,
            });
        }
        Ok(Entry {
            name: raw.name,
            task: raw.task,
            outputs,
        })
    }
}

static TABLE: LazyLock<Vec<Entry>> = LazyLock::new(|| {
    parse_table(include_str!("../../../tools/fixtures.json")).unwrap_or_else(|e| panic!("{e}"))
});

fn parse_table(text: &str) -> Result<Vec<Entry>, FixtureError> {
    serde_json::from_str(text).map_err(|error| FixtureError::Malformed {
        path: TABLE_FILE.to_owned(),
        message: error.to_string(),
    })
}

pub fn table() -> &'static [Entry] {
    &TABLE
}

pub fn entry(name: &str) -> Result<&'static Entry, FixtureError> {
    Repository::workspace().entry(name)
}

pub fn try_fixture(name: &str) -> Result<Fixture, FixtureError> {
    Repository::workspace().try_fixture(name)
}

pub fn fixture(name: &str) -> Fixture {
    try_fixture(name).unwrap_or_else(|e| panic!("{e}"))
}

pub(crate) struct Repository<'a> {
    root: PathBuf,
    table: &'a [Entry],
}

impl<'a> Repository<'a> {
    pub(crate) fn at(root: impl Into<PathBuf>, table: &'a [Entry]) -> Self {
        Repository {
            root: root.into(),
            table,
        }
    }

    fn workspace() -> Repository<'static> {
        Repository::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."), table())
    }

    pub(crate) fn manifest(&self) -> Result<Manifest, FixtureError> {
        match fs::read(self.root.join(MANIFEST)) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| FixtureError::Malformed {
                path: MANIFEST.to_owned(),
                message: error.to_string(),
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Manifest::new()),
            Err(source) => Err(FixtureError::Read {
                path: MANIFEST.to_owned(),
                source,
            }),
        }
    }

    pub(crate) fn entry(&self, name: &str) -> Result<&'a Entry, FixtureError> {
        self.table
            .iter()
            .find(|entry| entry.name == name)
            .ok_or_else(|| FixtureError::Unknown {
                name: name.to_owned(),
                known: self.table.iter().map(|entry| entry.name.clone()).collect(),
            })
    }

    pub(crate) fn try_fixture(&self, name: &str) -> Result<Fixture, FixtureError> {
        let entry = self.entry(name)?;
        match self.manifest()?.get(name) {
            None => Err(FixtureError::NotCaptured {
                name: name.to_owned(),
            }),
            Some(recorded) if *recorded != VERSION.id => Err(FixtureError::OtherVersion {
                name: name.to_owned(),
                recorded: recorded.clone(),
                corpus: VERSION.id.clone(),
            }),
            Some(_) => Ok(Fixture {
                root: self.root.clone(),
                entry: entry.clone(),
            }),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Fixture {
    root: PathBuf,
    entry: Entry,
}

impl Fixture {
    pub fn name(&self) -> &str {
        &self.entry.name
    }

    fn file_in(&self, to: &str, name: &str) -> FixtureFile {
        FixtureFile {
            root: self.root.clone(),
            to: to.to_owned(),
            name: name.to_owned(),
        }
    }

    pub(crate) fn try_file(&self, file_name: &str) -> Result<FixtureFile, FixtureError> {
        self.entry
            .outputs
            .iter()
            .find(|output| output.files.matches(file_name))
            .map(|output| self.file_in(&output.to, file_name))
            .ok_or_else(|| FixtureError::NoSuchFile {
                name: self.entry.name.clone(),
                file: file_name.to_owned(),
                outputs: self.entry.output_names(),
            })
    }

    pub(crate) fn try_files(&self) -> Result<Vec<FixtureFile>, FixtureError> {
        let mut files = Vec::new();
        let mut patterned = false;
        for output in &self.entry.outputs {
            if !matches!(output.files, Files::Matching(_)) {
                continue;
            }
            patterned = true;
            let read_error = |source| FixtureError::Read {
                path: output.to.clone(),
                source,
            };
            for listed in fs::read_dir(self.root.join(&output.to)).map_err(read_error)? {
                let listed = listed.map_err(read_error)?;
                let name = listed.file_name().to_string_lossy().into_owned();
                if listed.path().is_file() && output.files.matches(&name) {
                    files.push(self.file_in(&output.to, &name));
                }
            }
        }
        if !patterned {
            return Err(FixtureError::NoPattern {
                name: self.entry.name.clone(),
            });
        }
        files.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(files)
    }

    fn try_single(&self) -> Result<FixtureFile, FixtureError> {
        match self.entry.outputs.as_slice() {
            [
                Output {
                    files: Files::Named(file),
                    to,
                },
            ] => Ok(self.file_in(to, file)),
            _ => Err(FixtureError::NotOneFile {
                name: self.entry.name.clone(),
                outputs: self.entry.output_names(),
            }),
        }
    }

    pub(crate) fn try_bytes(&self) -> Result<Vec<u8>, FixtureError> {
        self.try_single()?.try_bytes()
    }

    pub(crate) fn try_text(&self) -> Result<String, FixtureError> {
        self.try_single()?.try_text()
    }

    pub(crate) fn try_dump(&self, magic: &[u8; 8]) -> Result<Vec<u8>, FixtureError> {
        self.try_single()?.try_dump(magic)
    }

    pub fn bytes(&self) -> Vec<u8> {
        self.try_bytes().unwrap_or_else(|e| panic!("{e}"))
    }

    pub fn text(&self) -> String {
        self.try_text().unwrap_or_else(|e| panic!("{e}"))
    }

    pub fn dump(&self, magic: &[u8; 8]) -> Vec<u8> {
        self.try_dump(magic).unwrap_or_else(|e| panic!("{e}"))
    }

    pub fn file(&self, file_name: &str) -> FixtureFile {
        self.try_file(file_name).unwrap_or_else(|e| panic!("{e}"))
    }

    pub fn files(&self) -> Vec<FixtureFile> {
        self.try_files().unwrap_or_else(|e| panic!("{e}"))
    }
}

#[derive(Debug, Clone)]
pub struct FixtureFile {
    root: PathBuf,
    to: String,
    name: String,
}

impl FixtureFile {
    pub fn name(&self) -> &str {
        &self.name
    }

    fn relative(&self) -> String {
        format!("{}/{}", self.to, self.name)
    }

    pub(crate) fn try_bytes(&self) -> Result<Vec<u8>, FixtureError> {
        fs::read(self.root.join(&self.to).join(&self.name)).map_err(|source| FixtureError::Read {
            path: self.relative(),
            source,
        })
    }

    pub(crate) fn try_text(&self) -> Result<String, FixtureError> {
        String::from_utf8(self.try_bytes()?).map_err(|_| FixtureError::NotText {
            path: self.relative(),
        })
    }

    pub(crate) fn try_dump(&self, magic: &[u8; 8]) -> Result<Vec<u8>, FixtureError> {
        let bytes = self.try_bytes()?;
        let Some((header, payload)) = bytes.split_first_chunk::<HEADER_LEN>() else {
            return Err(FixtureError::ShortHeader {
                path: self.relative(),
                len: bytes.len(),
                header: HEADER_LEN,
            });
        };
        let (found_magic, rest) = header.split_at(MAGIC_LEN);
        let (format, world) = rest.split_at(FORMAT_LEN);
        if found_magic != magic {
            return Err(FixtureError::WrongMagic {
                path: self.relative(),
                expected: String::from_utf8_lossy(magic).into_owned(),
                found: String::from_utf8_lossy(found_magic).into_owned(),
            });
        }
        let format = u32::from_le_bytes(format.try_into().expect("the format field is 4 bytes"));
        if format != DUMP_FORMAT {
            return Err(FixtureError::WrongFormat {
                path: self.relative(),
                expected: DUMP_FORMAT,
                found: format,
            });
        }
        let world = i32::from_le_bytes(world.try_into().expect("the world field is 4 bytes"));
        if world != VERSION.world_version {
            return Err(FixtureError::WrongWorldVersion {
                path: self.relative(),
                expected: VERSION.world_version,
                found: world,
            });
        }
        Ok(payload.to_vec())
    }

    pub fn bytes(&self) -> Vec<u8> {
        self.try_bytes().unwrap_or_else(|e| panic!("{e}"))
    }

    pub fn text(&self) -> String {
        self.try_text().unwrap_or_else(|e| panic!("{e}"))
    }

    pub fn dump(&self, magic: &[u8; 8]) -> Vec<u8> {
        self.try_dump(magic).unwrap_or_else(|e| panic!("{e}"))
    }
}

#[cfg(test)]
mod tests;
