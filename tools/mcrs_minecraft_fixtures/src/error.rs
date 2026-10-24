#[derive(Debug, thiserror::Error)]
pub enum FixtureError {
    #[error("unknown fixture {name}; known fixtures: {}", .known.join(", "))]
    Unknown { name: String, known: Vec<String> },
    #[error(
        "fixture {name} is not recorded in {}; run cargo run -p mcrs_minecraft_update -- recapture {name}",
        crate::MANIFEST
    )]
    NotCaptured { name: String },
    #[error(
        "fixture {name} was captured at {recorded} but the corpus is {corpus}; run cargo run -p mcrs_minecraft_update -- recapture {name}"
    )]
    OtherVersion {
        name: String,
        recorded: String,
        corpus: String,
    },
    #[error("{path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("{path}: the file is not UTF-8 text")]
    NotText { path: String },
    #[error("fixture {name} is not a single file; its outputs are {}", .outputs.join(", "))]
    NotOneFile { name: String, outputs: Vec<String> },
    #[error("fixture {name} has no output {file}; its outputs are {}", .outputs.join(", "))]
    NoSuchFile {
        name: String,
        file: String,
        outputs: Vec<String>,
    },
    #[error("fixture {name} has no output that matches a pattern")]
    NoPattern { name: String },
    #[error("{path}: expected a {expected} dump, found {found}")]
    WrongMagic {
        path: String,
        expected: String,
        found: String,
    },
    #[error("{path}: oracle format version {found}, this reader takes {expected}")]
    WrongFormat {
        path: String,
        expected: u32,
        found: u32,
    },
    #[error("{path}: dumped at world version {found}, the corpus is at {expected}")]
    WrongWorldVersion {
        path: String,
        expected: i32,
        found: i32,
    },
    #[error("{path}: {len} bytes cannot hold the {header}-byte dump header")]
    ShortHeader {
        path: String,
        len: usize,
        header: usize,
    },
    #[error("{path}: {message}")]
    Malformed { path: String, message: String },
}
