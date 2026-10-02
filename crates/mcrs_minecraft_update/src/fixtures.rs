use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use mcrs_minecraft_core::Version;

use crate::gradle;

pub const MANIFEST: &str = "tools/captures.json";
const VERSION_FILE: &str = "assets/minecraft/version.json";
const ORACLE: &str = "tools/vanilla-oracle";
const GENERATOR_FIXTURES: &str = "crates/mcrs_minecraft_worldgen_generator/src/tests/fixtures";
const TEXT_FIXTURES: &str = "crates/mcrs_minecraft_protocol/tests/fixtures/text";
const ITEM_FIXTURES: &str = "crates/mcrs_minecraft_protocol/tests/fixtures/item";
const PROTOCOL_FIXTURES: &str = "crates/mcrs_minecraft_protocol/tests/fixtures";
const ANVIL_FIXTURES: &str = "crates/mcrs_minecraft_anvil/src/fixtures/vanilla";
const WORLD_FIXTURES: &str = "crates/mcrs_minecraft_world/tests/fixtures";
const PLACE_FIXTURES: &str = "crates/mcrs_minecraft_worldgen_feature_place/tests/fixtures/vanilla";

pub enum Files {
    Named(&'static str),
    All,
}

pub struct Output {
    pub files: Files,
    pub to: &'static str,
}

pub struct Fixture {
    pub name: &'static str,
    pub project: &'static str,
    pub task: &'static str,
    pub golden: Option<&'static str>,
    pub outputs: &'static [Output],
}

const fn named(file: &'static str, to: &'static str) -> Output {
    Output {
        files: Files::Named(file),
        to,
    }
}

const fn every_file(to: &'static str) -> Output {
    Output {
        files: Files::All,
        to,
    }
}

pub const FIXTURES: &[Fixture] = &[
    Fixture {
        name: "density",
        project: ORACLE,
        task: "dumpOracle",
        golden: None,
        outputs: &[every_file(
            "crates/mcrs_minecraft_worldgen_density/tests/fixtures/vanilla",
        )],
    },
    Fixture {
        name: "surface",
        project: ORACLE,
        task: "dumpSurface",
        golden: None,
        outputs: &[every_file(GENERATOR_FIXTURES)],
    },
    Fixture {
        name: "biome_containers",
        project: ORACLE,
        task: "dumpBiomes",
        golden: None,
        outputs: &[named("biome_containers.bin", GENERATOR_FIXTURES)],
    },
    Fixture {
        name: "feature_steps",
        project: ORACLE,
        task: "dumpFeatureSteps",
        golden: None,
        outputs: &[named(
            "feature_steps.bin",
            "crates/mcrs_minecraft_worldgen_feature/tests/fixtures/vanilla",
        )],
    },
    Fixture {
        name: "ore_vein",
        project: ORACLE,
        task: "dumpOreVeins",
        golden: None,
        outputs: &[named("ore_vein.bin", PLACE_FIXTURES)],
    },
    Fixture {
        name: "tree_geometry",
        project: ORACLE,
        task: "dumpTrees",
        golden: None,
        outputs: &[named("tree_geometry.bin", PLACE_FIXTURES)],
    },
    Fixture {
        name: "templates",
        project: ORACLE,
        task: "dumpTemplates",
        golden: None,
        outputs: &[named("templates.bin", GENERATOR_FIXTURES)],
    },
    Fixture {
        name: "structure_placement",
        project: ORACLE,
        task: "dumpPlacement",
        golden: None,
        outputs: &[
            named(
                "structure_cells.bin",
                "crates/mcrs_minecraft_worldgen_structure/tests/fixtures/vanilla",
            ),
            named("structure_sites.bin", GENERATOR_FIXTURES),
        ],
    },
    Fixture {
        name: "structure_layouts",
        project: ORACLE,
        task: "dumpJigsaw",
        golden: None,
        outputs: &[named("structure_layouts.bin", GENERATOR_FIXTURES)],
    },
    Fixture {
        name: "template_placement",
        project: ORACLE,
        task: "dumpTemplatePlacement",
        golden: None,
        outputs: &[named("template_placement.bin", GENERATOR_FIXTURES)],
    },
    Fixture {
        name: "beard",
        project: ORACLE,
        task: "dumpBeard",
        golden: None,
        outputs: &[named(
            "beard.bin",
            "crates/mcrs_minecraft_worldgen/tests/fixtures/vanilla",
        )],
    },
    Fixture {
        name: "registry_census",
        project: ORACLE,
        task: "dumpRegistryCensus",
        golden: None,
        outputs: &[named(
            "registry_census.bin",
            "crates/mcrs_minecraft_world/src/entity/fixtures",
        )],
    },
    Fixture {
        name: "structure_pieces",
        project: ORACLE,
        task: "dumpStructurePieces",
        golden: None,
        outputs: &[named("structure_pieces.bin", GENERATOR_FIXTURES)],
    },
    Fixture {
        name: "structure_geometry",
        project: ORACLE,
        task: "dumpStructureGeometry",
        golden: None,
        outputs: &[named("structure_geometry.bin", GENERATOR_FIXTURES)],
    },
    Fixture {
        name: "vanilla_chunk",
        project: ORACLE,
        task: "dumpChunks",
        golden: None,
        outputs: &[
            named("chunk_full.nbt", ANVIL_FIXTURES),
            named("chunk_terrain.nbt", ANVIL_FIXTURES),
            named("chunk_retrogen.nbt", ANVIL_FIXTURES),
            named("chunk_retrogen_minimal.nbt", ANVIL_FIXTURES),
        ],
    },
    Fixture {
        name: "snbt",
        project: ORACLE,
        task: "dumpGolden",
        golden: Some("snbt"),
        outputs: &[named("snbt.json", "crates/mcrs_minecraft_nbt/src/fixtures")],
    },
    Fixture {
        name: "hash_ops",
        project: ORACLE,
        task: "dumpGolden",
        golden: Some("hash_ops"),
        outputs: &[named(
            "hash_ops.json",
            "crates/mcrs_minecraft_item_component/src/fixtures",
        )],
    },
    Fixture {
        name: "text_vanilla",
        project: ORACLE,
        task: "dumpGolden",
        golden: Some("text_vanilla"),
        outputs: &[named("vanilla.json", TEXT_FIXTURES)],
    },
    Fixture {
        name: "text_probe",
        project: ORACLE,
        task: "dumpGolden",
        golden: Some("text_probe"),
        outputs: &[named("probe.json", TEXT_FIXTURES)],
    },
    Fixture {
        name: "text_nbt",
        project: ORACLE,
        task: "dumpGolden",
        golden: Some("text_nbt"),
        outputs: &[named("nbt.json", TEXT_FIXTURES)],
    },
    Fixture {
        name: "text_wire",
        project: ORACLE,
        task: "dumpGolden",
        golden: Some("text_wire"),
        outputs: &[named("wire.json", TEXT_FIXTURES)],
    },
    Fixture {
        name: "item_plain",
        project: ORACLE,
        task: "dumpGolden",
        golden: Some("item_plain"),
        outputs: &[named("plain_golden.json", ITEM_FIXTURES)],
    },
    Fixture {
        name: "item_nested",
        project: ORACLE,
        task: "dumpGolden",
        golden: Some("item_nested"),
        outputs: &[named("nested_golden.json", ITEM_FIXTURES)],
    },
    Fixture {
        name: "item_predicate",
        project: ORACLE,
        task: "dumpGolden",
        golden: Some("item_predicate"),
        outputs: &[named("predicate_vanilla.json", ITEM_FIXTURES)],
    },
    Fixture {
        name: "item_kinds",
        project: ORACLE,
        task: "dumpGolden",
        golden: Some("item_kinds"),
        outputs: &[named("kinds.json", ITEM_FIXTURES)],
    },
    Fixture {
        name: "item_holders",
        project: ORACLE,
        task: "dumpGolden",
        golden: Some("item_holders"),
        outputs: &[named("holders_golden.txt", ITEM_FIXTURES)],
    },
    Fixture {
        name: "item_registry_refs",
        project: ORACLE,
        task: "dumpGolden",
        golden: Some("item_registry_refs"),
        outputs: &[named("registry_refs_golden.txt", ITEM_FIXTURES)],
    },
    Fixture {
        name: "item_records",
        project: ORACLE,
        task: "dumpGolden",
        golden: Some("item_records"),
        outputs: &[named("vanilla_records.txt", ITEM_FIXTURES)],
    },
    Fixture {
        name: "recipe_packets",
        project: ORACLE,
        task: "dumpGolden",
        golden: Some("recipe_packets"),
        outputs: &[named("recipe_packets_golden.txt", PROTOCOL_FIXTURES)],
    },
    Fixture {
        name: "particles",
        project: ORACLE,
        task: "dumpGolden",
        golden: Some("particles"),
        outputs: &[named("particles_golden.txt", PROTOCOL_FIXTURES)],
    },
    Fixture {
        name: "inventory_packets",
        project: ORACLE,
        task: "dumpGolden",
        golden: Some("inventory_packets"),
        outputs: &[named("inventory_packets_golden.txt", PROTOCOL_FIXTURES)],
    },
    Fixture {
        name: "join_packets",
        project: ORACLE,
        task: "dumpGolden",
        golden: Some("join_packets"),
        outputs: &[named("join_packets_golden.txt", PROTOCOL_FIXTURES)],
    },
    Fixture {
        name: "frames",
        project: ORACLE,
        task: "dumpGolden",
        golden: Some("frames"),
        outputs: &[named("frames_golden.txt", PROTOCOL_FIXTURES)],
    },
    Fixture {
        name: "vanilla_player",
        project: ORACLE,
        task: "dumpGolden",
        golden: Some("vanilla_player"),
        outputs: &[named("vanilla_player.dat", WORLD_FIXTURES)],
    },
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

pub fn render(manifest: &Manifest) -> String {
    let mut text = serde_json::to_string_pretty(manifest).expect("a string map serializes");
    text.push('\n');
    text
}

pub fn record(path: &Path, name: &str, id: &str) -> Result<(), String> {
    let mut manifest = read_manifest(path)?;
    manifest.insert(name.to_owned(), id.to_owned());
    fs::write(path, render(&manifest)).map_err(|error| format!("{}: {error}", path.display()))
}

pub fn stale(manifest: &Manifest, table: &[Fixture], id: &str) -> Vec<String> {
    let missing_or_other = table
        .iter()
        .filter(|fixture| manifest.get(fixture.name).map(String::as_str) != Some(id))
        .map(|fixture| fixture.name.to_owned());
    let unknown = manifest
        .keys()
        .filter(|key| !table.iter().any(|fixture| fixture.name == key.as_str()))
        .cloned();
    missing_or_other.chain(unknown).collect()
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
            Files::All => {
                let mut names = Vec::new();
                for entry in
                    fs::read_dir(out).map_err(|error| format!("{}: {error}", out.display()))?
                {
                    let entry = entry.map_err(|error| format!("{}: {error}", out.display()))?;
                    if entry.path().is_file() {
                        names.push(entry.file_name());
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

pub fn capture(
    root: &Path,
    fixture: &Fixture,
    run: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<Vec<PathBuf>, String> {
    static RUNS: AtomicUsize = AtomicUsize::new(0);
    let id = corpus_id(root)?;
    let out = std::env::temp_dir().join(format!(
        "mcrs-update-fixture-{}-{}-{}",
        std::process::id(),
        RUNS.fetch_add(1, Ordering::Relaxed),
        fixture.name
    ));
    let _ = fs::remove_dir_all(&out);
    fs::create_dir_all(&out).map_err(|error| format!("{}: {error}", out.display()))?;
    let result = run(&out)
        .and_then(|()| store(root, fixture, &out))
        .and_then(|copied| {
            record(&root.join(MANIFEST), fixture.name, &id)?;
            Ok(copied)
        });
    let _ = fs::remove_dir_all(&out);
    result
}

fn utf8(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| format!("{}: not valid UTF-8", path.display()))
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
    let out = utf8(out)?.to_owned();
    match fixture.golden {
        None => Ok(vec![("oracleOut", out)]),
        Some(golden) => {
            let current = current_file(root, fixture)?;
            Ok(vec![
                ("golden", golden.to_owned()),
                ("goldenIn", utf8(&current)?.to_owned()),
                ("goldenOut", out),
            ])
        }
    }
}

pub fn recapture(root: &Path, fixture: &Fixture) -> Result<Vec<PathBuf>, String> {
    capture(root, fixture, |out| {
        let owned = properties(root, fixture, out)?;
        let borrowed: Vec<(&str, &str)> = owned
            .iter()
            .map(|(name, value)| (*name, value.as_str()))
            .collect();
        gradle::run(&root.join(fixture.project), fixture.task, &borrowed)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "mcrs_fixtures-{}-{}-{name}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

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
        let behind = stale(&manifest, FIXTURES, id());
        assert!(
            behind.is_empty(),
            "not captured on {}: {}",
            id(),
            behind.join(", ")
        );
    }

    #[test]
    fn fixture_names_are_unique() {
        let mut names = names();
        let all = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), all);
    }

    #[test]
    fn a_dump_fixture_passes_only_the_oracle_output_directory() {
        let root = Path::new("/repo");
        let out = Path::new("/tmp/out");
        assert_eq!(
            properties(root, fixture("beard"), out).unwrap(),
            [("oracleOut", "/tmp/out".to_owned())]
        );
    }

    #[test]
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

    #[test]
    fn every_golden_current_file_exists_in_the_repository() {
        let root = repository();
        for fixture in FIXTURES.iter().filter(|fixture| fixture.golden.is_some()) {
            let current = current_file(&root, fixture).unwrap();
            assert!(current.is_file(), "{}: {}", fixture.name, current.display());
        }
    }

    #[test]
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

    #[test]
    fn a_named_output_is_copied_to_its_destination() {
        let fixture = fixture("beard");
        let root = root_for(fixture);
        let out = produce(&[("beard.bin", b"MC")]);
        let copied = store(&root, fixture, &out).unwrap();
        let to = root.join("crates/mcrs_minecraft_worldgen/tests/fixtures/vanilla/beard.bin");
        assert_eq!(copied, std::slice::from_ref(&to));
        assert_eq!(fs::read(to).unwrap(), b"MC");
    }

    #[test]
    fn an_every_file_output_copies_everything_the_task_wrote() {
        let fixture = fixture("density");
        let root = root_for(fixture);
        let out = produce(&[("overworld_a.bin", b"a"), ("overworld_b.bin", b"b")]);
        let copied = store(&root, fixture, &out).unwrap();
        assert_eq!(copied.len(), 2);
        let to = root.join("crates/mcrs_minecraft_worldgen_density/tests/fixtures/vanilla");
        assert_eq!(fs::read(to.join("overworld_a.bin")).unwrap(), b"a");
        assert_eq!(fs::read(to.join("overworld_b.bin")).unwrap(), b"b");
    }

    #[test]
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

    #[test]
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

    #[test]
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

    #[test]
    fn a_task_output_with_no_file_is_an_error() {
        let fixture = fixture("density");
        let root = root_for(fixture);
        let out = produce(&[]);
        assert!(store(&root, fixture, &out).is_err());
    }

    #[test]
    fn an_empty_file_among_every_file_is_an_error_and_nothing_is_copied() {
        let fixture = fixture("density");
        let root = root_for(fixture);
        let out = produce(&[("overworld_a.bin", b"a"), ("overworld_b.bin", b"")]);
        assert!(store(&root, fixture, &out).is_err());
        let to = root.join("crates/mcrs_minecraft_worldgen_density/tests/fixtures/vanilla");
        assert_eq!(fs::read_dir(to).unwrap().count(), 0);
    }

    #[test]
    fn recording_in_either_order_writes_the_same_bytes() {
        let (a, b) = (
            scratch("order-a").join("captures.json"),
            scratch("order-b").join("captures.json"),
        );
        record(&a, "density", "26.4").unwrap();
        record(&a, "beard", "26.4").unwrap();
        record(&b, "beard", "26.4").unwrap();
        record(&b, "density", "26.4").unwrap();
        assert_eq!(fs::read(&a).unwrap(), fs::read(&b).unwrap());
    }

    #[test]
    fn the_manifest_is_pretty_printed_with_sorted_keys_and_one_trailing_newline() {
        let path = scratch("format").join("captures.json");
        record(&path, "surface", "x").unwrap();
        record(&path, "density", "x").unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "{\n  \"density\": \"x\",\n  \"surface\": \"x\"\n}\n"
        );
    }

    #[test]
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
    fn an_empty_or_absent_manifest_makes_every_fixture_stale() {
        let all: Vec<String> = names().into_iter().map(String::from).collect();
        assert_eq!(stale(&Manifest::new(), FIXTURES, "26.4"), all);
        let absent = read_manifest(&scratch("absent").join("captures.json")).unwrap();
        assert_eq!(stale(&absent, FIXTURES, "26.4"), all);
    }

    #[test]
    fn a_manifest_that_lacks_a_fixture_reports_it() {
        let mut manifest: Manifest = names()
            .into_iter()
            .map(|n| (n.into(), "26.4".into()))
            .collect();
        manifest.remove("beard");
        assert_eq!(stale(&manifest, FIXTURES, "26.4"), ["beard"]);
    }

    #[test]
    fn a_complete_current_manifest_has_nothing_stale() {
        let manifest: Manifest = names()
            .into_iter()
            .map(|n| (n.into(), "26.4".into()))
            .collect();
        assert!(stale(&manifest, FIXTURES, "26.4").is_empty());
    }

    #[test]
    fn an_entry_with_another_id_is_stale() {
        let mut manifest: Manifest = names()
            .into_iter()
            .map(|n| (n.into(), "26.4".into()))
            .collect();
        manifest.insert("surface".into(), "26.3".into());
        assert_eq!(stale(&manifest, FIXTURES, "26.4"), ["surface"]);
    }

    #[test]
    fn a_key_that_is_not_in_the_table_is_stale() {
        let mut manifest: Manifest = names()
            .into_iter()
            .map(|n| (n.into(), "26.4".into()))
            .collect();
        manifest.insert("no_such_fixture".into(), "26.4".into());
        assert_eq!(stale(&manifest, FIXTURES, "26.4"), ["no_such_fixture"]);
    }

    #[test]
    fn the_comparison_is_exact() {
        let current = id();
        let mut manifest: Manifest = names()
            .into_iter()
            .map(|n| (n.into(), current.to_owned()))
            .collect();
        manifest.insert("density".into(), current.to_uppercase());
        manifest.insert("beard".into(), format!("{current} "));
        let mut found = stale(&manifest, FIXTURES, current);
        found.sort();
        assert_eq!(found, ["beard", "density"]);
    }

    #[test]
    fn an_unknown_fixture_name_is_an_error_that_lists_the_known_names() {
        let error = lookup("no_such_fixture").err().unwrap();
        assert!(error.contains("no_such_fixture"), "{error}");
        for name in names() {
            assert!(error.contains(name), "{error}");
        }
    }

    #[test]
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

    #[test]
    fn a_failing_task_records_nothing() {
        let fixture = fixture("beard");
        let root = root_for(fixture);
        assert!(capture(&root, fixture, |_| Err("task failed".into())).is_err());
        assert!(!root.join(MANIFEST).exists());
    }

    #[test]
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

    #[test]
    fn a_task_that_wrote_nothing_records_nothing() {
        let fixture = fixture("density");
        let root = root_for(fixture);
        assert!(capture(&root, fixture, |_| Ok(())).is_err());
        assert!(!root.join(MANIFEST).exists());
    }

    #[test]
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
}
