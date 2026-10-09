use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use mcrs_minecraft_core::Version;

use crate::gradle;
use crate::release;

pub const MANIFEST: &str = "tools/captures.json";
const VERSION_FILE: &str = "assets/minecraft/version.json";
pub const ORACLE: &str = "tools/vanilla-oracle";
const GOLDEN_TASK: &str = "dumpGolden";
const GENERATOR_FIXTURES: &str = "crates/mcrs_minecraft_worldgen_generator/src/tests/fixtures";
const TEXT_FIXTURES: &str = "crates/mcrs_minecraft_protocol/tests/fixtures/text";
const ITEM_FIXTURES: &str = "crates/mcrs_minecraft_protocol/tests/fixtures/item";
const PROTOCOL_FIXTURES: &str = "crates/mcrs_minecraft_protocol/tests/fixtures";
const ANVIL_FIXTURES: &str = "crates/mcrs_minecraft_anvil/src/fixtures/vanilla";
const WORLD_FIXTURES: &str = "crates/mcrs_minecraft_world/tests/fixtures";
const PLACE_FIXTURES: &str = "crates/mcrs_minecraft_worldgen_feature_place/tests/fixtures/vanilla";

pub enum Files {
    Named(&'static str),
    Matching(&'static str),
}

pub struct Output {
    pub files: Files,
    pub to: &'static str,
}

pub struct Fixture {
    pub name: &'static str,
    pub task: &'static str,
    pub outputs: &'static [Output],
}

const fn named(file: &'static str, to: &'static str) -> Output {
    Output {
        files: Files::Named(file),
        to,
    }
}

const fn matching(pattern: &'static str, to: &'static str) -> Output {
    Output {
        files: Files::Matching(pattern),
        to,
    }
}

const fn dump(name: &'static str, task: &'static str, outputs: &'static [Output]) -> Fixture {
    Fixture {
        name,
        task,
        outputs,
    }
}

const fn golden(name: &'static str, outputs: &'static [Output]) -> Fixture {
    dump(name, GOLDEN_TASK, outputs)
}

pub const FIXTURES: &[Fixture] = &[
    dump(
        "density",
        "dumpOracle",
        &[matching(
            "overworld_s*_c*.bin",
            "crates/mcrs_minecraft_worldgen_density/tests/fixtures/vanilla",
        )],
    ),
    dump(
        "surface",
        "dumpSurface",
        &[matching("surface_s*_c*.bin", GENERATOR_FIXTURES)],
    ),
    dump(
        "biome_containers",
        "dumpBiomes",
        &[named("biome_containers.bin", GENERATOR_FIXTURES)],
    ),
    dump(
        "feature_steps",
        "dumpFeatureSteps",
        &[named(
            "feature_steps.bin",
            "crates/mcrs_minecraft_worldgen_feature/tests/fixtures/vanilla",
        )],
    ),
    dump(
        "ore_vein",
        "dumpOreVeins",
        &[named("ore_vein.bin", PLACE_FIXTURES)],
    ),
    dump(
        "tree_geometry",
        "dumpTrees",
        &[named("tree_geometry.bin", PLACE_FIXTURES)],
    ),
    dump(
        "templates",
        "dumpTemplates",
        &[named("templates.bin", GENERATOR_FIXTURES)],
    ),
    dump(
        "structure_placement",
        "dumpPlacement",
        &[
            named(
                "structure_cells.bin",
                "crates/mcrs_minecraft_worldgen_structure/tests/fixtures/vanilla",
            ),
            named("structure_sites.bin", GENERATOR_FIXTURES),
        ],
    ),
    dump(
        "structure_layouts",
        "dumpJigsaw",
        &[named("structure_layouts.bin", GENERATOR_FIXTURES)],
    ),
    dump(
        "template_placement",
        "dumpTemplatePlacement",
        &[named("template_placement.bin", GENERATOR_FIXTURES)],
    ),
    dump(
        "beard",
        "dumpBeard",
        &[named(
            "beard.bin",
            "crates/mcrs_minecraft_worldgen/tests/fixtures/vanilla",
        )],
    ),
    dump(
        "registry_census",
        "dumpRegistryCensus",
        &[named(
            "registry_census.bin",
            "crates/mcrs_minecraft_world/src/entity/fixtures",
        )],
    ),
    dump(
        "structure_pieces",
        "dumpStructurePieces",
        &[named("structure_pieces.bin", GENERATOR_FIXTURES)],
    ),
    dump(
        "structure_geometry",
        "dumpStructureGeometry",
        &[named("structure_geometry.bin", GENERATOR_FIXTURES)],
    ),
    dump(
        "vanilla_chunk",
        "dumpChunks",
        &[
            named("chunk_full.nbt", ANVIL_FIXTURES),
            named("chunk_terrain.nbt", ANVIL_FIXTURES),
            named("chunk_retrogen.nbt", ANVIL_FIXTURES),
            named("chunk_retrogen_minimal.nbt", ANVIL_FIXTURES),
        ],
    ),
    golden(
        "snbt",
        &[named("snbt.json", "crates/mcrs_minecraft_nbt/src/fixtures")],
    ),
    golden(
        "hash_ops",
        &[named(
            "hash_ops.json",
            "crates/mcrs_minecraft_item/src/fixtures",
        )],
    ),
    golden("text_vanilla", &[named("vanilla.json", TEXT_FIXTURES)]),
    golden("text_probe", &[named("probe.json", TEXT_FIXTURES)]),
    golden("text_nbt", &[named("nbt.json", TEXT_FIXTURES)]),
    golden("text_wire", &[named("wire.json", TEXT_FIXTURES)]),
    golden("item_plain", &[named("plain_golden.json", ITEM_FIXTURES)]),
    golden("item_nested", &[named("nested_golden.json", ITEM_FIXTURES)]),
    golden(
        "item_predicate",
        &[named("predicate_vanilla.json", ITEM_FIXTURES)],
    ),
    golden("item_kinds", &[named("kinds.json", ITEM_FIXTURES)]),
    golden(
        "item_holders",
        &[named("holders_golden.txt", ITEM_FIXTURES)],
    ),
    golden(
        "item_registry_refs",
        &[named("registry_refs_golden.txt", ITEM_FIXTURES)],
    ),
    golden(
        "item_records",
        &[named("vanilla_records.txt", ITEM_FIXTURES)],
    ),
    golden(
        "recipe_packets",
        &[named("recipe_packets_golden.txt", PROTOCOL_FIXTURES)],
    ),
    golden(
        "particles",
        &[named("particles_golden.txt", PROTOCOL_FIXTURES)],
    ),
    golden(
        "inventory_packets",
        &[named("inventory_packets_golden.txt", PROTOCOL_FIXTURES)],
    ),
    golden(
        "join_packets",
        &[named("join_packets_golden.txt", PROTOCOL_FIXTURES)],
    ),
    golden(
        "serverbound_game_packets",
        &[named(
            "serverbound_game_packets_golden.txt",
            PROTOCOL_FIXTURES,
        )],
    ),
    golden("frames", &[named("frames_golden.txt", PROTOCOL_FIXTURES)]),
    golden(
        "vanilla_player",
        &[named("vanilla_player.dat", WORLD_FIXTURES)],
    ),
    golden(
        "registry_values",
        &[named("registry_values.txt", WORLD_FIXTURES)],
    ),
];

pub type Manifest = BTreeMap<String, String>;

pub fn names() -> Vec<&'static str> {
    FIXTURES.iter().map(|fixture| fixture.name).collect()
}

pub fn lookup(name: &str) -> Result<&'static Fixture, String> {
    FIXTURES
        .iter()
        .find(|fixture| fixture.name == name)
        .ok_or_else(|| {
            format!(
                "unknown fixture {name}; known fixtures: {}",
                names().join(", ")
            )
        })
}

pub fn corpus_id(root: &Path) -> Result<String, String> {
    let path = root.join(VERSION_FILE);
    let bytes = fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let version: Version =
        serde_json::from_slice(&bytes).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(version.id)
}

pub fn read_manifest(path: &Path) -> Result<Manifest, String> {
    match fs::read(path) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).map_err(|error| format!("{}: {error}", path.display()))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Manifest::new()),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

pub fn record(path: &Path, name: &str, id: &str) -> Result<(), String> {
    let mut manifest = read_manifest(path)?;
    manifest.insert(name.to_owned(), id.to_owned());
    fs::write(path, release::pretty(&manifest)?)
        .map_err(|error| format!("{}: {error}", path.display()))
}

#[cfg(test)]
pub fn stale(manifest: &Manifest, id: &str) -> Vec<String> {
    let missing_or_other = FIXTURES
        .iter()
        .filter(|fixture| manifest.get(fixture.name).map(String::as_str) != Some(id))
        .map(|fixture| fixture.name.to_owned());
    let unknown = manifest
        .keys()
        .filter(|key| !FIXTURES.iter().any(|fixture| fixture.name == key.as_str()))
        .cloned();
    missing_or_other.chain(unknown).collect()
}

fn matches(pattern: &str, name: &str) -> bool {
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

fn read_non_empty(path: &Path) -> Result<Vec<u8>, String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if bytes.is_empty() {
        return Err(format!("{}: the task wrote an empty file", path.display()));
    }
    Ok(bytes)
}

fn collect(root: &Path, fixture: &Fixture, out: &Path) -> Result<Vec<(PathBuf, Vec<u8>)>, String> {
    let mut found = Vec::new();
    for output in fixture.outputs {
        let to = root.join(output.to);
        match output.files {
            Files::Named(name) => found.push((to.join(name), read_non_empty(&out.join(name))?)),
            Files::Matching(pattern) => {
                let mut names = Vec::new();
                for entry in
                    fs::read_dir(out).map_err(|error| format!("{}: {error}", out.display()))?
                {
                    let entry = entry.map_err(|error| format!("{}: {error}", out.display()))?;
                    if entry.path().is_file() {
                        let name = entry.file_name();
                        if !matches(pattern, &name.to_string_lossy()) {
                            return Err(format!(
                                "{}: the file does not match {pattern}; widen the fixture's pattern and its .gitattributes line together",
                                entry.path().display()
                            ));
                        }
                        names.push(name);
                    }
                }
                if names.is_empty() {
                    return Err(format!("{}: the task wrote no file", out.display()));
                }
                names.sort();
                for name in names {
                    found.push((to.join(&name), read_non_empty(&out.join(&name))?));
                }
            }
        }
    }
    Ok(found)
}

pub fn store(root: &Path, fixture: &Fixture, out: &Path) -> Result<Vec<PathBuf>, String> {
    let files = collect(root, fixture, out)?;
    let staged: Vec<PathBuf> = files
        .iter()
        .map(|(to, _)| {
            let mut name = to.file_name().expect("a destination has a name").to_owned();
            name.push(".recapture");
            to.with_file_name(name)
        })
        .collect();
    for (stage, (_, bytes)) in staged.iter().zip(&files) {
        if let Err(error) = fs::write(stage, bytes) {
            for stage in &staged {
                let _ = fs::remove_file(stage);
            }
            return Err(format!("{}: {error}", stage.display()));
        }
    }
    for (stage, (to, _)) in staged.iter().zip(&files) {
        fs::rename(stage, to).map_err(|error| format!("{}: {error}", to.display()))?;
    }
    Ok(files.into_iter().map(|(to, _)| to).collect())
}

pub fn with_scratch<T>(
    name: &str,
    then: impl FnOnce(&Path) -> Result<T, String>,
) -> Result<T, String> {
    static RUNS: AtomicUsize = AtomicUsize::new(0);
    let out = std::env::temp_dir().join(format!(
        "mcrs-update-{}-{}-{name}",
        std::process::id(),
        RUNS.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&out);
    let result = fs::create_dir_all(&out)
        .map_err(|error| format!("{}: {error}", out.display()))
        .and_then(|()| then(&out));
    let _ = fs::remove_dir_all(&out);
    result
}

pub fn capture(
    root: &Path,
    fixture: &Fixture,
    run: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<Vec<PathBuf>, String> {
    let id = corpus_id(root)?;
    with_scratch(fixture.name, |out| {
        run(out)?;
        let copied = store(root, fixture, out)?;
        record(&root.join(MANIFEST), fixture.name, &id)?;
        Ok(copied)
    })
}

fn current_file(root: &Path, fixture: &Fixture) -> Result<PathBuf, String> {
    match fixture.outputs.first() {
        Some(Output {
            files: Files::Named(file),
            to,
        }) => Ok(root.join(to).join(file)),
        _ => Err(format!(
            "{}: a golden is rewritten from one named file",
            fixture.name
        )),
    }
}

fn properties<'a>(
    root: &Path,
    fixture: &'a Fixture,
    out: &'a Path,
) -> Result<Vec<(&'static str, String)>, String> {
    let out = gradle::utf8(out)?.to_owned();
    if fixture.task != GOLDEN_TASK {
        return Ok(vec![("oracleOut", out)]);
    }
    let current = current_file(root, fixture)?;
    Ok(vec![
        ("golden", fixture.name.to_owned()),
        ("goldenIn", gradle::utf8(&current)?.to_owned()),
        ("goldenOut", out),
    ])
}

pub fn recapture(root: &Path, fixture: &Fixture) -> Result<Vec<PathBuf>, String> {
    capture(root, fixture, |out| {
        gradle::run(
            &root.join(ORACLE),
            fixture.task,
            &properties(root, fixture, out)?,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::scratch;

    fn repository() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn root_for(fixture: &Fixture) -> PathBuf {
        let root = scratch("root");
        for output in fixture.outputs {
            fs::create_dir_all(root.join(output.to)).unwrap();
        }
        fs::create_dir_all(root.join("tools")).unwrap();
        fs::create_dir_all(root.join("assets/minecraft")).unwrap();
        fs::write(root.join(VERSION_FILE), mcrs_minecraft_core::VERSION_JSON).unwrap();
        root
    }

    fn fixture(name: &str) -> &'static Fixture {
        lookup(name).unwrap()
    }

    fn produce(files: &[(&str, &[u8])]) -> PathBuf {
        let out = scratch("out");
        for (name, bytes) in files {
            fs::write(out.join(name), bytes).unwrap();
        }
        out
    }

    fn id() -> &'static str {
        &mcrs_minecraft_core::VERSION.id
    }

    #[test]
    fn every_fixture_was_captured_on_the_corpus_version() {
        assert!(!FIXTURES.is_empty());
        let manifest = read_manifest(&repository().join(MANIFEST)).unwrap();
        let behind = stale(&manifest, id());
        assert!(
            behind.is_empty(),
            "not captured on {}: {}",
            id(),
            behind.join(", ")
        );
    }

    fn fixture_names_are_unique() {
        let mut names = names();
        let all = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), all);
    }

    fn a_dump_fixture_passes_only_the_oracle_output_directory() {
        let root = Path::new("/repo");
        let out = Path::new("/tmp/out");
        assert_eq!(
            properties(root, fixture("beard"), out).unwrap(),
            [("oracleOut", "/tmp/out".to_owned())]
        );
    }

    fn a_golden_fixture_passes_its_name_its_current_file_and_the_output_directory() {
        let root = Path::new("/repo");
        let out = Path::new("/tmp/out");
        assert_eq!(
            properties(root, fixture("snbt"), out).unwrap(),
            [
                ("golden", "snbt".to_owned()),
                (
                    "goldenIn",
                    "/repo/crates/mcrs_minecraft_nbt/src/fixtures/snbt.json".to_owned()
                ),
                ("goldenOut", "/tmp/out".to_owned()),
            ]
        );
    }

    fn every_golden_current_file_exists_in_the_repository() {
        let root = repository();
        for fixture in FIXTURES
            .iter()
            .filter(|fixture| fixture.task == GOLDEN_TASK)
        {
            let current = current_file(&root, fixture).unwrap();
            assert!(current.is_file(), "{}: {}", fixture.name, current.display());
        }
    }

    fn every_destination_directory_exists_in_the_repository() {
        let root = repository();
        for fixture in FIXTURES {
            for output in fixture.outputs {
                assert!(
                    root.join(output.to).is_dir(),
                    "{}: {} is not a directory",
                    fixture.name,
                    output.to
                );
            }
        }
    }

    fn a_named_output_is_copied_to_its_destination() {
        let fixture = fixture("beard");
        let root = root_for(fixture);
        let out = produce(&[("beard.bin", b"MC")]);
        let copied = store(&root, fixture, &out).unwrap();
        let to = root.join("crates/mcrs_minecraft_worldgen/tests/fixtures/vanilla/beard.bin");
        assert_eq!(copied, std::slice::from_ref(&to));
        assert_eq!(fs::read(to).unwrap(), b"MC");
    }

    fn a_pattern_output_copies_everything_the_task_wrote() {
        let fixture = fixture("density");
        let root = root_for(fixture);
        let out = produce(&[
            ("overworld_s1_c0_0.bin", b"a"),
            ("overworld_s1_c0_0_dense.bin", b"b"),
        ]);
        let copied = store(&root, fixture, &out).unwrap();
        assert_eq!(copied.len(), 2);
        let to = root.join("crates/mcrs_minecraft_worldgen_density/tests/fixtures/vanilla");
        assert_eq!(fs::read(to.join("overworld_s1_c0_0.bin")).unwrap(), b"a");
        assert_eq!(
            fs::read(to.join("overworld_s1_c0_0_dense.bin")).unwrap(),
            b"b"
        );
    }

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
            let Files::Matching(pattern) = fixture(fixture_name).outputs[0].files else {
                panic!("{fixture_name} does not carry a pattern");
            };
            assert_eq!(matches(pattern, name), expected, "{pattern} against {name}");
        }
    }

    fn a_fixture_with_two_destinations_writes_both() {
        let fixture = fixture("structure_placement");
        let root = root_for(fixture);
        let out = produce(&[("structure_cells.bin", b"c"), ("structure_sites.bin", b"s")]);
        let copied = store(&root, fixture, &out).unwrap();
        assert_eq!(copied.len(), 2);
        assert_eq!(
            fs::read(root.join(
                "crates/mcrs_minecraft_worldgen_structure/tests/fixtures/vanilla/structure_cells.bin"
            ))
            .unwrap(),
            b"c"
        );
        assert_eq!(
            fs::read(root.join(format!("{GENERATOR_FIXTURES}/structure_sites.bin"))).unwrap(),
            b"s"
        );
    }

    fn a_missing_file_of_two_leaves_both_destinations_untouched() {
        let fixture = fixture("structure_placement");
        let root = root_for(fixture);
        let cells = root.join(
            "crates/mcrs_minecraft_worldgen_structure/tests/fixtures/vanilla/structure_cells.bin",
        );
        let sites = root.join(format!("{GENERATOR_FIXTURES}/structure_sites.bin"));
        fs::write(&cells, b"old cells").unwrap();
        fs::write(&sites, b"old sites").unwrap();

        let out = produce(&[("structure_cells.bin", b"new cells")]);
        let error = store(&root, fixture, &out).unwrap_err();
        assert!(error.contains("structure_sites.bin"), "{error}");
        assert_eq!(fs::read(cells).unwrap(), b"old cells");
        assert_eq!(fs::read(sites).unwrap(), b"old sites");
    }

    fn an_empty_file_of_two_leaves_both_destinations_untouched() {
        let fixture = fixture("structure_placement");
        let root = root_for(fixture);
        let sites = root.join(format!("{GENERATOR_FIXTURES}/structure_sites.bin"));
        fs::write(&sites, b"old sites").unwrap();

        let out = produce(&[
            ("structure_cells.bin", b"new cells"),
            ("structure_sites.bin", b""),
        ]);
        assert!(store(&root, fixture, &out).is_err());
        assert_eq!(fs::read(sites).unwrap(), b"old sites");
        assert!(
            !root
                .join("crates/mcrs_minecraft_worldgen_structure/tests/fixtures/vanilla/structure_cells.bin")
                .exists()
        );
    }

    fn a_task_output_with_no_file_is_an_error() {
        let fixture = fixture("density");
        let root = root_for(fixture);
        let out = produce(&[]);
        assert!(store(&root, fixture, &out).is_err());
    }

    fn a_file_outside_the_pattern_fails_the_store_and_copies_nothing() {
        let fixture = fixture("density");
        let root = root_for(fixture);
        let out = produce(&[("overworld_s1_c0_0.bin", b"a"), ("notes.txt", b"b")]);
        let error = store(&root, fixture, &out).unwrap_err();
        assert!(error.contains("notes.txt"), "{error}");
        assert!(error.contains("overworld_s*_c*.bin"), "{error}");
        let to = root.join("crates/mcrs_minecraft_worldgen_density/tests/fixtures/vanilla");
        assert_eq!(fs::read_dir(to).unwrap().count(), 0);
    }

    fn an_empty_file_among_a_pattern_output_is_an_error_and_nothing_is_copied() {
        let fixture = fixture("density");
        let root = root_for(fixture);
        let out = produce(&[
            ("overworld_s1_c0_0.bin", b"a"),
            ("overworld_s1_c0_1.bin", b""),
        ]);
        assert!(store(&root, fixture, &out).is_err());
        let to = root.join("crates/mcrs_minecraft_worldgen_density/tests/fixtures/vanilla");
        assert_eq!(fs::read_dir(to).unwrap().count(), 0);
    }

    fn the_manifest_is_pretty_printed_with_sorted_keys_and_one_trailing_newline() {
        let path = scratch("format").join("captures.json");
        record(&path, "surface", "x").unwrap();
        record(&path, "density", "x").unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "{\n  \"density\": \"x\",\n  \"surface\": \"x\"\n}\n"
        );
    }

    fn recording_keeps_the_other_entries_and_replaces_the_same_one() {
        let path = scratch("replace").join("captures.json");
        record(&path, "density", "old").unwrap();
        record(&path, "beard", "old").unwrap();
        record(&path, "density", "new").unwrap();
        let manifest = read_manifest(&path).unwrap();
        assert_eq!(manifest["density"], "new");
        assert_eq!(manifest["beard"], "old");
        assert_eq!(manifest.len(), 2);
    }

    #[test]
    fn a_fixture_is_stale_when_absent_or_on_another_id_and_so_is_an_unknown_key() {
        let all: Vec<String> = names().into_iter().map(String::from).collect();
        let full: Manifest = all.iter().map(|n| (n.clone(), "26.4".into())).collect();
        let absent = read_manifest(&scratch("absent").join("captures.json")).unwrap();
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
        let error = lookup("no_such_fixture").err().unwrap();
        assert!(error.contains("no_such_fixture"), "{error}");
        for name in names() {
            assert!(error.contains(name), "{error}");
        }
    }

    fn a_successful_capture_copies_and_records_the_corpus_id() {
        let fixture = fixture("beard");
        let root = root_for(fixture);
        let copied = capture(&root, fixture, |out| {
            fs::write(out.join("beard.bin"), b"MC").map_err(|e| e.to_string())
        })
        .unwrap();
        assert_eq!(copied.len(), 1);
        let manifest = read_manifest(&root.join(MANIFEST)).unwrap();
        assert_eq!(manifest["beard"], id());
    }

    fn a_failing_task_records_nothing() {
        let fixture = fixture("beard");
        let root = root_for(fixture);
        assert!(capture(&root, fixture, |_| Err("task failed".into())).is_err());
        assert!(!root.join(MANIFEST).exists());
    }

    fn a_task_that_wrote_an_empty_file_records_nothing_and_copies_nothing() {
        let fixture = fixture("beard");
        let root = root_for(fixture);
        let result = capture(&root, fixture, |out| {
            fs::write(out.join("beard.bin"), b"").map_err(|e| e.to_string())
        });
        assert!(result.is_err());
        assert!(!root.join(MANIFEST).exists());
        let destination = root.join("crates/mcrs_minecraft_worldgen/tests/fixtures/vanilla");
        assert_eq!(fs::read_dir(destination).unwrap().count(), 0);
    }

    fn a_two_destination_capture_has_one_manifest_entry() {
        let fixture = fixture("structure_placement");
        let root = root_for(fixture);
        capture(&root, fixture, |out| {
            fs::write(out.join("structure_cells.bin"), b"c").map_err(|e| e.to_string())?;
            fs::write(out.join("structure_sites.bin"), b"s").map_err(|e| e.to_string())
        })
        .unwrap();
        let manifest = read_manifest(&root.join(MANIFEST)).unwrap();
        assert_eq!(manifest.keys().collect::<Vec<_>>(), ["structure_placement"]);
    }

    #[test]
    fn the_fixture_table_agrees_with_the_repository() {
        fixture_names_are_unique();
        every_golden_current_file_exists_in_the_repository();
        every_destination_directory_exists_in_the_repository();
    }

    #[test]
    fn the_task_properties_follow_the_fixture_kind() {
        a_dump_fixture_passes_only_the_oracle_output_directory();
        a_golden_fixture_passes_its_name_its_current_file_and_the_output_directory();
    }

    #[test]
    fn storing_copies_every_output_to_its_destination() {
        a_named_output_is_copied_to_its_destination();
        a_pattern_output_copies_everything_the_task_wrote();
        a_pattern_matches_what_its_dump_writes_and_nothing_else();
        a_fixture_with_two_destinations_writes_both();
    }

    #[test]
    fn a_missing_or_empty_output_fails_the_store_and_copies_nothing() {
        a_missing_file_of_two_leaves_both_destinations_untouched();
        an_empty_file_of_two_leaves_both_destinations_untouched();
        a_task_output_with_no_file_is_an_error();
        an_empty_file_among_a_pattern_output_is_an_error_and_nothing_is_copied();
        a_file_outside_the_pattern_fails_the_store_and_copies_nothing();
    }

    #[test]
    fn the_manifest_is_rewritten_sorted_and_keeps_other_entries() {
        the_manifest_is_pretty_printed_with_sorted_keys_and_one_trailing_newline();
        recording_keeps_the_other_entries_and_replaces_the_same_one();
    }

    #[test]
    fn a_successful_capture_records_one_entry_with_the_corpus_id() {
        a_successful_capture_copies_and_records_the_corpus_id();
        a_two_destination_capture_has_one_manifest_entry();
    }

    #[test]
    fn a_failed_capture_records_nothing() {
        a_failing_task_records_nothing();
        a_task_that_wrote_an_empty_file_records_nothing_and_copies_nothing();
    }

    fn attributes_of(paths: &[String]) -> Vec<(String, bool)> {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let mut stdin = Vec::new();
        for path in paths {
            stdin.extend_from_slice(path.as_bytes());
            stdin.push(0);
        }
        let mut child = Command::new("git")
            .arg("-C")
            .arg(repository())
            .args([
                "-c",
                "core.attributesFile=/dev/null",
                "check-attr",
                "-z",
                "--stdin",
                "linguist-generated",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut pipe = child.stdin.take().unwrap();
        let output = std::thread::scope(|scope| {
            scope.spawn(move || pipe.write_all(&stdin).unwrap());
            child.wait_with_output().unwrap()
        });
        assert!(output.status.success(), "git check-attr failed");
        let mut fields: Vec<&[u8]> = output.stdout.split(|byte| *byte == 0).collect();
        assert_eq!(fields.pop(), Some(&[][..]));
        let (triples, rest) = fields.as_chunks::<3>();
        assert!(rest.is_empty(), "git check-attr wrote a partial triple");
        triples
            .iter()
            .map(|triple| {
                (
                    String::from_utf8(triple[0].to_vec()).unwrap(),
                    triple[2] == b"set",
                )
            })
            .collect()
    }

    fn tracked_children(directory: &str) -> Vec<String> {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(repository())
            .args(["ls-files", "-z", "--", directory])
            .output()
            .unwrap();
        assert!(output.status.success(), "git ls-files failed");
        output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
            .map(|path| String::from_utf8(path.to_vec()).unwrap())
            .filter(|path| {
                path.strip_prefix(directory)
                    .and_then(|rest| rest.strip_prefix('/'))
                    .is_some_and(|name| !name.contains('/'))
            })
            .collect()
    }

    fn output_probe(output: &Output) -> String {
        match output.files {
            Files::Named(name) => format!("{}/{name}", output.to),
            Files::Matching(pattern) => format!("{}/{}", output.to, pattern.replace('*', "0")),
        }
    }

    fn data_probes() -> Vec<String> {
        let mut probes = vec![
            format!("{}/version.json", crate::CORPUS),
            format!("{}/probe.json", crate::CORPUS),
            crate::DESCRIPTOR.to_owned(),
            crate::FONT_HINT.to_owned(),
            MANIFEST.to_owned(),
        ];
        for file in crate::REPORT_FILES.into_iter().chain([crate::NAMES_FILE]) {
            probes.push(format!("{}/{file}", crate::REPORTS));
        }
        for directory in crate::DEFINITIONS {
            probes.push(format!(
                "{}/{directory}/probe.json",
                crate::DEFINITIONS_ROOT
            ));
        }
        probes
    }

    fn written_by(output: &Output, name: &str) -> bool {
        match output.files {
            Files::Named(file) => file == name,
            Files::Matching(pattern) => matches(pattern, name),
        }
    }

    #[test]
    fn the_attributes_mark_every_tool_output_and_no_hand_written_file() {
        let mut problems = Vec::new();

        let probes: Vec<String> = FIXTURES
            .iter()
            .flat_map(|fixture| fixture.outputs)
            .map(output_probe)
            .chain(data_probes())
            .collect();
        for (path, generated) in attributes_of(&probes) {
            if !generated {
                problems.push(format!("{path}: written by the update tool but not marked"));
            }
        }

        let mut directories: Vec<&str> = FIXTURES
            .iter()
            .flat_map(|fixture| fixture.outputs)
            .map(|output| output.to)
            .collect();
        directories.sort_unstable();
        directories.dedup();
        for directory in directories {
            let children = tracked_children(directory);
            for (path, generated) in attributes_of(&children) {
                let name = path.rsplit('/').next().unwrap();
                let written = FIXTURES
                    .iter()
                    .flat_map(|fixture| fixture.outputs)
                    .filter(|output| output.to == directory)
                    .any(|output| written_by(output, name));
                match (written, generated) {
                    (true, false) => {
                        problems.push(format!("{path}: written by the update tool but not marked"))
                    }
                    (false, true) => {
                        problems.push(format!("{path}: hand-written but marked generated"))
                    }
                    _ => {}
                }
            }
        }

        problems.sort();
        problems.dedup();
        assert!(
            problems.is_empty(),
            "add or narrow the linguist-generated line in .gitattributes:\n{}",
            problems.join("\n")
        );
    }
}
