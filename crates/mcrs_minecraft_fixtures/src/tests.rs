use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use mcrs_minecraft_core::VERSION;

use super::*;

const MAGIC: &[u8; 8] = b"MCTESTS0";

fn scratch(name: &str) -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "mcrs_fixtures-{}-{}-{name}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn id() -> &'static str {
    &VERSION.id
}

fn header(magic: &[u8; 8], format: u32, world: i32) -> Vec<u8> {
    let mut bytes = magic.to_vec();
    bytes.extend(format.to_le_bytes());
    bytes.extend(world.to_le_bytes());
    bytes
}

fn dump(payload: &[u8]) -> Vec<u8> {
    let mut bytes = header(MAGIC, 1, VERSION.world_version);
    bytes.extend(payload);
    bytes
}

const SCRATCH_TABLE: &str = r#"[
  {"name": "plain", "task": "dumpGolden", "outputs": [{"file": "plain.txt", "to": "data"}]},
  {"name": "uncaptured", "task": "dumpX", "outputs": [{"file": "plain.txt", "to": "data"}]},
  {"name": "stale", "task": "dumpX", "outputs": [{"file": "plain.txt", "to": "data"}]},
  {"name": "binary", "task": "dumpX", "outputs": [{"file": "binary.dat", "to": "data"}]},
  {"name": "good", "task": "dumpX", "outputs": [{"file": "good.bin", "to": "dumps"}]},
  {"name": "bare", "task": "dumpX", "outputs": [{"file": "bare.bin", "to": "dumps"}]},
  {"name": "badmagic", "task": "dumpX", "outputs": [{"file": "badmagic.bin", "to": "dumps"}]},
  {"name": "format2", "task": "dumpX", "outputs": [{"file": "format2.bin", "to": "dumps"}]},
  {"name": "otherworld", "task": "dumpX", "outputs": [{"file": "otherworld.bin", "to": "dumps"}]},
  {"name": "short", "task": "dumpX", "outputs": [{"file": "short.bin", "to": "dumps"}]},
  {"name": "missing", "task": "dumpX", "outputs": [{"file": "missing.bin", "to": "dumps"}]},
  {"name": "two", "task": "dumpX", "outputs": [
    {"file": "a.txt", "to": "data"},
    {"file": "b.txt", "to": "data"}
  ]},
  {"name": "pattern", "task": "dumpX", "outputs": [{"pattern": "p*.bin", "to": "pats"}]}
]"#;

struct World {
    root: PathBuf,
    table: Vec<Entry>,
}

impl World {
    fn repository(&self) -> Repository<'_> {
        Repository::at(&self.root, &self.table)
    }
}

fn write(root: &Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn world() -> World {
    let root = scratch("world");
    let table = parse_table(SCRATCH_TABLE).unwrap();
    let mut manifest = Manifest::new();
    for entry in &table {
        manifest.insert(entry.name.clone(), id().to_owned());
    }
    manifest.remove("uncaptured");
    manifest.insert("stale".to_owned(), "old-id".to_owned());
    write(
        &root,
        MANIFEST,
        serde_json::to_string(&manifest).unwrap().as_bytes(),
    );

    write(&root, "data/plain.txt", b"plain text");
    write(&root, "data/binary.dat", &[0xff, 0xfe, 0x00]);
    write(&root, "data/a.txt", b"A");
    write(&root, "data/b.txt", b"B");
    write(&root, "dumps/good.bin", &dump(b"payload"));
    write(&root, "dumps/bare.bin", &dump(b""));
    write(
        &root,
        "dumps/badmagic.bin",
        &header(b"MCOTHER0", 1, VERSION.world_version),
    );
    write(
        &root,
        "dumps/format2.bin",
        &header(MAGIC, 2, VERSION.world_version),
    );
    write(
        &root,
        "dumps/otherworld.bin",
        &header(MAGIC, 1, VERSION.world_version + 1),
    );
    write(
        &root,
        "dumps/short.bin",
        &header(MAGIC, 1, VERSION.world_version)[..15],
    );
    write(&root, "pats/p2.bin", &dump(b"second"));
    write(&root, "pats/p1.bin", &dump(b"first"));
    write(&root, "pats/notes.txt", b"hand-written");
    write(&root, "pats/q.bin", b"outside the pattern");
    fs::create_dir_all(root.join("pats/pdir.bin")).unwrap();
    World { root, table }
}

struct Refusal {
    label: &'static str,
    run: fn(&Repository) -> Result<(), FixtureError>,
    kind: fn(&FixtureError) -> bool,
    mentions: &'static [&'static str],
}

#[expect(
    clippy::too_many_lines,
    reason = "the table of refused fixture requests reads as one list of cases, so a new refusal is one more entry"
)]
fn refusals() -> Vec<Refusal> {
    vec![
        Refusal {
            label: "an unknown name",
            run: |r| r.entry("no_such").map(drop),
            kind: |e| matches!(e, FixtureError::Unknown { .. }),
            mentions: &["no_such", "plain", "uncaptured", "pattern"],
        },
        Refusal {
            label: "an unknown name through the version check",
            run: |r| r.try_fixture("no_such").map(drop),
            kind: |e| matches!(e, FixtureError::Unknown { .. }),
            mentions: &["no_such", "plain"],
        },
        Refusal {
            label: "a table name absent from the manifest",
            run: |r| r.try_fixture("uncaptured").map(drop),
            kind: |e| matches!(e, FixtureError::NotCaptured { .. }),
            mentions: &[
                "uncaptured",
                "tools/captures.json",
                "cargo run -p mcrs_minecraft_update -- recapture uncaptured",
            ],
        },
        Refusal {
            label: "a root with no manifest",
            run: |r| {
                Repository::at(scratch("bare"), r.table)
                    .try_fixture("plain")
                    .map(drop)
            },
            kind: |e| matches!(e, FixtureError::NotCaptured { .. }),
            mentions: &["plain"],
        },
        Refusal {
            label: "another id",
            run: |r| r.try_fixture("stale").map(drop),
            kind: |e| matches!(e, FixtureError::OtherVersion { .. }),
            mentions: &[
                "stale",
                "old-id",
                "{id}",
                "cargo run -p mcrs_minecraft_update -- recapture stale",
            ],
        },
        Refusal {
            label: "a dump with the wrong magic",
            run: |r| r.try_fixture("badmagic")?.try_dump(MAGIC).map(drop),
            kind: |e| matches!(e, FixtureError::WrongMagic { .. }),
            mentions: &["dumps/badmagic.bin", "MCTESTS0", "MCOTHER0"],
        },
        Refusal {
            label: "a dump with format version 2",
            run: |r| r.try_fixture("format2")?.try_dump(MAGIC).map(drop),
            kind: |e| matches!(e, FixtureError::WrongFormat { .. }),
            mentions: &["dumps/format2.bin", "2", "1"],
        },
        Refusal {
            label: "a dump at another world version",
            run: |r| r.try_fixture("otherworld")?.try_dump(MAGIC).map(drop),
            kind: |e| matches!(e, FixtureError::WrongWorldVersion { .. }),
            mentions: &["dumps/otherworld.bin", "{world}", "{other}"],
        },
        Refusal {
            label: "a file shorter than the header",
            run: |r| r.try_fixture("short")?.try_dump(MAGIC).map(drop),
            kind: |e| matches!(e, FixtureError::ShortHeader { .. }),
            mentions: &["dumps/short.bin", "15", "16"],
        },
        Refusal {
            label: "a missing file",
            run: |r| r.try_fixture("missing")?.try_bytes().map(drop),
            kind: |e| matches!(e, FixtureError::Read { .. }),
            mentions: &["dumps/missing.bin"],
        },
        Refusal {
            label: "bytes of a fixture with two outputs",
            run: |r| r.try_fixture("two")?.try_bytes().map(drop),
            kind: |e| matches!(e, FixtureError::NotOneFile { .. }),
            mentions: &["two", "a.txt", "b.txt"],
        },
        Refusal {
            label: "bytes of a pattern output",
            run: |r| r.try_fixture("pattern")?.try_bytes().map(drop),
            kind: |e| matches!(e, FixtureError::NotOneFile { .. }),
            mentions: &["pattern", "p*.bin"],
        },
        Refusal {
            label: "text that is not UTF-8",
            run: |r| r.try_fixture("binary")?.try_text().map(drop),
            kind: |e| matches!(e, FixtureError::NotText { .. }),
            mentions: &["data/binary.dat"],
        },
        Refusal {
            label: "a file name the fixture lacks",
            run: |r| r.try_fixture("two")?.try_file("c.txt").map(drop),
            kind: |e| matches!(e, FixtureError::NoSuchFile { .. }),
            mentions: &["two", "c.txt", "a.txt", "b.txt"],
        },
        Refusal {
            label: "a file outside the pattern",
            run: |r| r.try_fixture("pattern")?.try_file("notes.txt").map(drop),
            kind: |e| matches!(e, FixtureError::NoSuchFile { .. }),
            mentions: &["pattern", "notes.txt", "p*.bin"],
        },
        Refusal {
            label: "files of a fixture with no pattern output",
            run: |r| r.try_fixture("two")?.try_files().map(drop),
            kind: |e| matches!(e, FixtureError::NoPattern { .. }),
            mentions: &["two"],
        },
    ]
}

#[test]
fn refused_names_versions_and_headers_fail_with_their_cause() {
    let world = world();
    let repository = world.repository();
    let mut failures = Vec::new();
    for refusal in refusals() {
        match (refusal.run)(&repository) {
            Ok(()) => failures.push(format!("{}: accepted", refusal.label)),
            Err(error) => {
                if !(refusal.kind)(&error) {
                    failures.push(format!("{}: wrong error {error:?}", refusal.label));
                }
                let text = error.to_string();
                for mention in refusal.mentions {
                    let needle = mention
                        .replace("{id}", id())
                        .replace("{world}", &VERSION.world_version.to_string())
                        .replace("{other}", &(VERSION.world_version + 1).to_string());
                    if !text.contains(&needle) {
                        failures.push(format!(
                            "{}: `{text}` does not mention `{needle}`",
                            refusal.label
                        ));
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn an_unknown_name_displays_the_known_names_in_table_order() {
    let table = parse_table(
        r#"[
          {"name": "alpha", "task": "t", "outputs": [{"file": "a", "to": "d"}]},
          {"name": "beta", "task": "t", "outputs": [{"file": "b", "to": "d"}]}
        ]"#,
    )
    .unwrap();
    let error = Repository::at(scratch("unknown"), &table)
        .entry("zeta")
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "unknown fixture zeta; known fixtures: alpha, beta"
    );
}

#[test]
fn entry_finds_a_fixture_the_version_check_refuses() {
    let world = world();
    let repository = world.repository();
    assert_eq!(repository.entry("stale").unwrap().name, "stale");
    assert!(matches!(
        repository.try_fixture("stale"),
        Err(FixtureError::OtherVersion { .. })
    ));
    assert_eq!(repository.entry("uncaptured").unwrap().name, "uncaptured");
    assert!(matches!(
        repository.try_fixture("uncaptured"),
        Err(FixtureError::NotCaptured { .. })
    ));
}

#[test]
fn a_dump_is_read_past_its_checked_header() {
    let world = world();
    let repository = world.repository();
    let good = repository.try_fixture("good").unwrap();
    assert_eq!(good.name(), "good");
    assert_eq!(good.try_dump(MAGIC).unwrap(), b"payload");
    assert_eq!(
        repository
            .try_fixture("bare")
            .unwrap()
            .try_dump(MAGIC)
            .unwrap(),
        b""
    );
    assert_eq!(
        repository
            .try_fixture("pattern")
            .unwrap()
            .try_file("p1.bin")
            .unwrap()
            .try_dump(MAGIC)
            .unwrap(),
        b"first"
    );
}

#[test]
fn a_pattern_output_lists_only_its_matching_files_sorted_by_name() {
    let world = world();
    let files = world
        .repository()
        .try_fixture("pattern")
        .unwrap()
        .try_files()
        .unwrap();
    let names: Vec<&str> = files.iter().map(FixtureFile::name).collect();
    assert_eq!(names, ["p1.bin", "p2.bin"]);
}

#[test]
fn one_output_of_several_is_read_by_its_file_name() {
    let world = world();
    let repository = world.repository();
    let two = repository.try_fixture("two").unwrap();
    let a = two.try_file("a.txt").unwrap();
    assert_eq!(a.name(), "a.txt");
    assert_eq!(a.try_text().unwrap(), "A");
    assert_eq!(two.try_file("b.txt").unwrap().try_text().unwrap(), "B");
    assert_eq!(
        repository
            .try_fixture("pattern")
            .unwrap()
            .try_file("p2.bin")
            .unwrap()
            .try_bytes()
            .unwrap(),
        dump(b"second")
    );
    assert_eq!(
        repository.try_fixture("plain").unwrap().try_text().unwrap(),
        "plain text"
    );
}

#[test]
fn an_output_must_name_exactly_one_of_a_file_and_a_pattern() {
    for (label, output) in [
        ("both", r#"{"file": "a", "pattern": "a*", "to": "d"}"#),
        ("neither", r#"{"to": "d"}"#),
    ] {
        let text = format!(r#"[{{"name": "broken", "task": "t", "outputs": [{output}]}}]"#);
        let Err(FixtureError::Malformed { path, message }) = parse_table(&text) else {
            panic!("{label}: the table was accepted");
        };
        assert_eq!(path, "tools/fixtures.json", "{label}");
        assert!(message.contains("broken"), "{label}: {message}");
    }
}

#[test]
fn a_pattern_matches_what_its_dump_writes_and_nothing_else() {
    for (fixture_name, name, expected) in [
        ("density", "overworld_s-5_c-3_7.bin", true),
        ("density", "overworld_s42_c0_0_dense.bin", true),
        ("density", "overworld_s1.bin", false),
        ("density", "xoverworld_s1_c0_0.bin", false),
        ("density", "overworld_s1_c0_0.bin.bak", false),
        ("surface", "surface_s7_c1_2.bin", true),
        ("surface", "overworld_s7_c1_2.bin", false),
    ] {
        let files = &entry(fixture_name).unwrap().outputs[0].files;
        assert!(matches!(files, Files::Matching(_)), "{fixture_name}");
        assert_eq!(
            files.matches(name),
            expected,
            "{fixture_name} against {name}"
        );
    }
    for (pattern, name, expected) in [
        ("abc", "abc", true),
        ("abc", "abcd", false),
        ("abc", "ab", false),
        ("a*", "a", true),
        ("a*", "abc", true),
        ("a*", "ba", false),
        ("*b", "ab", true),
        ("*b", "ba", false),
        ("*", "", true),
        ("*", "anything.bin", true),
        ("a*b*c", "axbyc", true),
        ("a*b*c", "abc", true),
        ("a*b*c", "axcyb", false),
        ("a*a", "a", false),
        ("a*b", "a", false),
        ("a*b*b", "ab", false),
        ("*ab*", "xxabyy", true),
        ("*ab*", "xxbayy", false),
    ] {
        assert_eq!(
            Files::Matching(pattern.to_owned()).matches(name),
            expected,
            "{pattern} against {name}"
        );
    }
}

#[test]
fn a_named_output_matches_only_its_own_file_name() {
    let named = Files::Named("a.txt".to_owned());
    assert!(named.matches("a.txt"));
    assert!(!named.matches("b.txt"));
    assert!(!named.matches("a.txt.bak"));
    assert!(!Files::Named("a*".to_owned()).matches("abc"));
}

#[test]
fn a_golden_is_the_entry_taken_by_the_golden_task() {
    assert!(entry("snbt").unwrap().is_golden());
    assert!(!entry("beard").unwrap().is_golden());
}

fn repository() -> PathBuf {
    Repository::workspace().root
}

fn stale(manifest: &Manifest, id: &str) -> Vec<String> {
    let missing_or_other = table()
        .iter()
        .filter(|fixture| manifest.get(&fixture.name).map(String::as_str) != Some(id))
        .map(|fixture| fixture.name.clone());
    let unknown = manifest
        .keys()
        .filter(|key| !table().iter().any(|fixture| &fixture.name == *key))
        .cloned();
    missing_or_other.chain(unknown).collect()
}

fn names() -> Vec<&'static str> {
    table()
        .iter()
        .map(|fixture| fixture.name.as_str())
        .collect()
}

#[test]
fn every_fixture_was_captured_on_the_corpus_version() {
    assert!(!table().is_empty());
    let manifest = Repository::workspace().manifest().unwrap();
    let behind = stale(&manifest, id());
    assert!(
        behind.is_empty(),
        "not captured on {}: {}",
        id(),
        behind.join(", ")
    );
}

#[test]
fn a_fixture_is_stale_when_absent_or_on_another_id_and_so_is_an_unknown_key() {
    let all: Vec<String> = names().into_iter().map(String::from).collect();
    let full: Manifest = all.iter().map(|n| (n.clone(), "26.4".into())).collect();
    let absent = Repository::at(scratch("absent"), table())
        .manifest()
        .unwrap();
    let (mut lacking, mut behind, mut unknown) = (full.clone(), full.clone(), full);
    lacking.remove("beard");
    behind.insert("surface".into(), "26.3".into());
    unknown.insert("no_such_fixture".into(), "26.4".into());
    for (manifest, expected) in [
        (Manifest::new(), all.clone()),
        (absent, all),
        (lacking, vec!["beard".to_owned()]),
        (behind, vec!["surface".to_owned()]),
        (unknown, vec!["no_such_fixture".to_owned()]),
    ] {
        assert_eq!(stale(&manifest, "26.4"), expected);
    }
}

#[test]
fn an_unknown_fixture_name_is_an_error_that_lists_the_known_names() {
    let error = entry("no_such_fixture").err().unwrap().to_string();
    assert!(error.contains("no_such_fixture"), "{error}");
    for name in names() {
        assert!(error.contains(name), "{error}");
    }
}

fn fixture_names_are_unique() {
    let mut names = names();
    let all = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), all);
}

fn every_golden_current_file_exists_in_the_repository() {
    let root = repository();
    for fixture in table().iter().filter(|fixture| fixture.is_golden()) {
        let Some(Output {
            files: Files::Named(file),
            to,
        }) = fixture.outputs.first()
        else {
            panic!(
                "{}: a golden is rewritten from one named file",
                fixture.name
            );
        };
        let current = root.join(to).join(file);
        assert!(current.is_file(), "{}: {}", fixture.name, current.display());
    }
}

fn every_destination_directory_exists_in_the_repository() {
    let root = repository();
    for fixture in table() {
        for output in &fixture.outputs {
            assert!(
                root.join(&output.to).is_dir(),
                "{}: {} is not a directory",
                fixture.name,
                output.to
            );
        }
    }
}

fn no_output_names_a_file_in_a_pattern_directory_that_the_pattern_matches() {
    let outputs: Vec<(&Entry, &Output)> = table()
        .iter()
        .flat_map(|fixture| fixture.outputs.iter().map(move |output| (fixture, output)))
        .collect();
    for (owner, pattern) in &outputs {
        let Files::Matching(text) = &pattern.files else {
            continue;
        };
        for (other, output) in &outputs {
            if std::ptr::eq(*pattern, *output) || output.to != pattern.to {
                continue;
            }
            match &output.files {
                Files::Named(name) => assert!(
                    !pattern.files.matches(name),
                    "{}: {name} is in {}'s directory and matches {text}",
                    other.name,
                    owner.name
                ),
                Files::Matching(_) => panic!(
                    "{} and {} both write patterns into {}",
                    owner.name, other.name, pattern.to
                ),
            }
        }
    }
}

#[test]
fn the_fixture_table_agrees_with_the_repository() {
    fixture_names_are_unique();
    every_golden_current_file_exists_in_the_repository();
    every_destination_directory_exists_in_the_repository();
    no_output_names_a_file_in_a_pattern_directory_that_the_pattern_matches();
}

#[test]
fn the_loader_resolves_the_table_against_the_repository() {
    for fixture in table() {
        assert_eq!(entry(&fixture.name).unwrap(), fixture);
        assert_eq!(try_fixture(&fixture.name).unwrap().name(), fixture.name);
    }
    let beard = fixture("beard");
    let path = repository().join("crates/mcrs_minecraft_worldgen/tests/fixtures/vanilla/beard.bin");
    let on_disk = fs::metadata(path).unwrap().len() as usize;
    let payload = beard.dump(b"MCBEARD0");
    assert!(!payload.is_empty());
    assert_eq!(payload.len() + HEADER_LEN, on_disk);
    assert_eq!(beard.bytes().len(), on_disk);
    assert!(fixture("snbt").text().starts_with(['[', '{']));
    assert!(
        !fixture("vanilla_chunk")
            .file("chunk_full.nbt")
            .bytes()
            .is_empty()
    );
    let density = fixture("density").files();
    assert!(!density.is_empty());
    assert!(density.iter().all(|file| {
        entry("density").unwrap().outputs[0]
            .files
            .matches(file.name())
    }));
}

#[test]
fn the_public_readers_return_what_the_checked_readers_return() {
    let world = world();
    let repository = world.repository();
    let two = repository.try_fixture("two").unwrap();
    assert_eq!(two.file("a.txt").text(), "A");
    assert_eq!(two.file("b.txt").bytes(), b"B");
    let plain = repository.try_fixture("plain").unwrap();
    assert_eq!(plain.text(), "plain text");
    assert_eq!(plain.bytes(), b"plain text");
    let good = repository.try_fixture("good").unwrap();
    assert_eq!(good.dump(MAGIC), b"payload");
    let pattern = repository.try_fixture("pattern").unwrap();
    assert_eq!(pattern.file("p1.bin").dump(MAGIC), b"first");
    let seconds: Vec<Vec<u8>> = pattern.files().iter().map(FixtureFile::bytes).collect();
    assert_eq!(seconds, [dump(b"first"), dump(b"second")]);
}

type Panicking = (
    &'static str,
    Box<dyn FnOnce() + std::panic::UnwindSafe>,
    &'static str,
);

#[test]
fn the_public_readers_panic_with_the_refusal_text() {
    let world = world();
    let repository = world.repository();
    let binary = repository.try_fixture("binary").unwrap();
    let two = repository.try_fixture("two").unwrap();
    let badmagic = repository.try_fixture("badmagic").unwrap();
    let panics: Vec<Panicking> = vec![
        (
            "text",
            Box::new({
                let binary = binary.clone();
                move || drop(binary.text())
            }),
            "data/binary.dat: the file is not UTF-8 text",
        ),
        (
            "bytes",
            Box::new({
                let two = two.clone();
                move || drop(two.bytes())
            }),
            "fixture two is not a single file",
        ),
        (
            "dump",
            Box::new(move || drop(badmagic.dump(MAGIC))),
            "dumps/badmagic.bin: expected a MCTESTS0 dump",
        ),
        (
            "file",
            Box::new({
                let two = two.clone();
                move || drop(two.file("c.txt"))
            }),
            "fixture two has no output c.txt",
        ),
        (
            "files",
            Box::new(move || drop(two.files())),
            "fixture two has no output that matches a pattern",
        ),
        (
            "file text",
            Box::new({
                let binary = binary.clone();
                move || drop(binary.file("binary.dat").text())
            }),
            "data/binary.dat: the file is not UTF-8 text",
        ),
        (
            "file dump",
            Box::new(move || drop(binary.file("binary.dat").dump(MAGIC))),
            "data/binary.dat: 3 bytes cannot hold the 16-byte dump header",
        ),
    ];
    for (label, run, expected) in panics {
        let payload = std::panic::catch_unwind(run).expect_err(label);
        let message = payload
            .downcast_ref::<String>()
            .cloned()
            .unwrap_or_default();
        assert!(message.contains(expected), "{label}: {message}");
    }
}

#[test]
fn only_a_failed_read_has_a_source() {
    use std::error::Error;

    let world = world();
    let repository = world.repository();
    let read = repository
        .try_fixture("missing")
        .unwrap()
        .try_bytes()
        .unwrap_err();
    let source = read.source().expect("a failed read keeps its cause");
    assert!(source.downcast_ref::<std::io::Error>().is_some());
    assert!(repository.entry("no_such").unwrap_err().source().is_none());
}

#[test]
fn every_refusal_displays_its_cause() {
    let world = world();
    let repository = world.repository();
    for refusal in refusals() {
        let error = (refusal.run)(&repository).unwrap_err();
        assert!(!error.to_string().is_empty(), "{}", refusal.label);
    }
}

#[test]
fn an_unreadable_or_malformed_manifest_is_an_error_and_a_missing_one_is_empty() {
    let table = parse_table(SCRATCH_TABLE).unwrap();

    let unreadable = scratch("unreadable");
    fs::create_dir_all(unreadable.join(MANIFEST)).unwrap();
    assert!(matches!(
        Repository::at(&unreadable, &table).manifest(),
        Err(FixtureError::Read { .. })
    ));

    let malformed = scratch("malformed");
    write(&malformed, MANIFEST, b"[1, 2]");
    let error = Repository::at(&malformed, &table).manifest().unwrap_err();
    assert!(
        matches!(&error, FixtureError::Malformed { path, .. } if path == MANIFEST),
        "{error}"
    );

    assert!(
        Repository::at(scratch("empty"), &table)
            .manifest()
            .unwrap()
            .is_empty()
    );
}
