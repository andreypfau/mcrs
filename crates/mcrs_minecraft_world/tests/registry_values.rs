use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_state::app::StatesPlugin;
use bevy_state::state::State;
use mcrs_minecraft_assets::{AppState, RegistryAccess};
use mcrs_minecraft_nbt::snbt::parse_tag;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_world::MinecraftWorldPlugin;
use serde::Serialize;
use serde::de::DeserializeOwned;

const NOT_SYNCHRONIZED: &str = "the game does not synchronize it";

const UNTYPED_COPIES: [&str; 6] = [
    "minecraft:banner_pattern",
    "minecraft:instrument",
    "minecraft:jukebox_song",
    "minecraft:painting_variant",
    "minecraft:trim_material",
    "minecraft:trim_pattern",
];

fn crate_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn read(relative: &str) -> String {
    let path = crate_path(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn split_top_level(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut current = String::new();
    for c in text.chars() {
        if in_string {
            current.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                current.push(c);
            }
            '(' | '[' | '{' => {
                depth += 1;
                current.push(c);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => parts.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current);
    }
    parts
}

fn closing(text: &str, open: usize) -> usize {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (index, &byte) in bytes.iter().enumerate().skip(open) {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth == 0 {
                    return index;
                }
            }
            _ => {}
        }
    }
    panic!("unbalanced brackets in the snapshot list")
}

fn snapshot_list_of(source: &str, file: &str) -> Vec<String> {
    const INVOCATION: &str = "snapshot_registry!(";
    let starts: Vec<usize> = source.match_indices(INVOCATION).map(|(i, _)| i).collect();
    assert_eq!(
        starts.len(),
        1,
        "{file} must hold exactly one {INVOCATION} invocation, found {}",
        starts.len()
    );
    let open = starts[0] + INVOCATION.len() - 1;
    let body = &source[open + 1..closing(source, open)];
    let arguments = split_top_level(body);
    let list = arguments
        .iter()
        .map(|argument| argument.trim())
        .find(|argument| argument.starts_with('['))
        .unwrap_or_else(|| panic!("{file}: the invocation has no bracketed list"));
    let inner = &list[1..list.len() - 1];
    let names: Vec<String> = split_top_level(inner)
        .iter()
        .map(|entry| entry.trim())
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let tuple = entry
                .strip_prefix('(')
                .and_then(|e| e.strip_suffix(')'))
                .unwrap_or_else(|| panic!("{file}: a list entry is not a tuple: {entry}"));
            let fields = split_top_level(tuple);
            let name = fields
                .get(1)
                .unwrap_or_else(|| panic!("{file}: a list entry has no registry name: {entry}"))
                .trim();
            name.strip_prefix('"')
                .and_then(|n| n.strip_suffix('"'))
                .unwrap_or_else(|| panic!("{file}: the second field is not a string: {name}"))
                .to_string()
        })
        .collect();
    assert!(!names.is_empty(), "{file}: the snapshot list holds no name");
    names
}

fn snapshot_list() -> Vec<String> {
    snapshot_list_of(&read("src/lib.rs"), "src/lib.rs")
}

struct Golden {
    registries: BTreeMap<String, BTreeMap<String, NbtTag>>,
}

fn golden() -> Golden {
    let mut registries: BTreeMap<String, BTreeMap<String, NbtTag>> = BTreeMap::new();
    for line in read("tests/fixtures/registry_values.txt").lines() {
        let mut fields = line.splitn(3, ' ');
        let registry = fields.next().unwrap();
        let entries = registries.entry(registry.to_string()).or_default();
        match (fields.next(), fields.next()) {
            (None, None) => {}
            (Some(entry), Some(snbt)) => {
                let tag = parse_tag(snbt)
                    .unwrap_or_else(|e| panic!("{registry} {entry}: snbt does not parse: {e:?}"));
                assert!(
                    entries.insert(entry.to_string(), tag).is_none(),
                    "{registry} {entry} appears twice in the golden"
                );
            }
            _ => panic!("malformed golden line: {line}"),
        }
    }
    Golden { registries }
}

fn not_compared_lines() -> Vec<(String, String)> {
    read("tests/fixtures/registry_values_not_compared.txt")
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let (registry, reason) = line
                .split_once(' ')
                .unwrap_or_else(|| panic!("malformed line of registries not compared: {line}"));
            (registry.to_string(), reason.to_string())
        })
        .collect()
}

fn coverage_problems(
    list: &[String],
    golden: &BTreeSet<String>,
    lines: &[(String, String)],
) -> Vec<String> {
    let listed: BTreeSet<&String> = list.iter().collect();
    let mut problems = Vec::new();
    let mut refused = BTreeSet::new();
    for (registry, reason) in lines {
        if reason != NOT_SYNCHRONIZED {
            continue;
        }
        if !listed.contains(registry) {
            problems.push(format!(
                "{registry} is on the fixture of registries not compared and is not in the snapshot list"
            ));
        }
        refused.insert(registry.clone());
    }
    for registry in &listed {
        if !golden.contains(*registry) && !refused.contains(*registry) {
            problems.push(format!("the golden lacks {registry}; it must be captured"));
        }
    }
    for registry in golden {
        if !listed.contains(registry) {
            problems.push(format!(
                "the golden holds {registry}, which is not in the snapshot list"
            ));
        }
        if refused.contains(registry) {
            problems.push(format!(
                "{registry} is in the golden and also refused by the capture"
            ));
        }
    }
    problems
}

#[test]
fn every_golden_line_parses_to_a_tag() {
    let golden = golden();
    for registry in UNTYPED_COPIES.iter().chain(&["minecraft:dialog"]) {
        let entries = golden
            .registries
            .get(*registry)
            .unwrap_or_else(|| panic!("{registry} is not in the golden"));
        assert!(
            !entries.is_empty(),
            "{registry} has no entries in the golden"
        );
        for (entry, tag) in entries {
            assert!(
                matches!(tag, NbtTag::Compound(_)),
                "{registry} {entry}: the game's value is not a compound"
            );
        }
    }
}

#[test]
fn the_golden_covers_the_snapshot_list() {
    let golden: BTreeSet<String> = golden().registries.keys().cloned().collect();
    let problems = coverage_problems(&snapshot_list(), &golden, &not_compared_lines());
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

const NOT_OBTAINED: &str = "not obtained from a booted app: ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Obtained,
    NoSnapshot,
    EntryWithoutValue,
}

impl Outcome {
    fn reason(self) -> Option<String> {
        let outcome = match self {
            Outcome::Obtained => return None,
            Outcome::NoSnapshot => "no snapshot of that name",
            Outcome::EntryWithoutValue => "an entry without a value",
        };
        Some(format!("{NOT_OBTAINED}{outcome}"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Treatment {
    Files,
    App,
    Listed(String),
    NotInGolden,
}

impl Treatment {
    fn line(&self, registry: &str) -> String {
        match self {
            Treatment::Files => format!("treatment: {registry} compared from its files"),
            Treatment::App => format!("treatment: {registry} compared from the booted app"),
            Treatment::Listed(reason) => format!("treatment: {registry} {reason}"),
            Treatment::NotInGolden => format!("treatment: {registry} not in the golden"),
        }
    }
}

#[derive(Debug, Default)]
struct Listing {
    treatments: BTreeMap<String, Treatment>,
    problems: Vec<String>,
}

fn check_listing(
    list: &[String],
    golden: &BTreeSet<String>,
    rows: &BTreeSet<String>,
    lines: &[(String, String)],
    outcomes: &BTreeMap<String, Outcome>,
    required: &[&str],
) -> Listing {
    let mut listing = Listing::default();
    let in_list: BTreeSet<&str> = list.iter().map(String::as_str).collect();

    for registry in list {
        let treatment = if !golden.contains(registry) {
            if lines
                .iter()
                .any(|(r, reason)| r == registry && reason == NOT_SYNCHRONIZED)
            {
                Treatment::Listed(NOT_SYNCHRONIZED.to_string())
            } else {
                listing.problems.push(format!(
                    "{registry} is not in the golden; it must be captured"
                ));
                Treatment::NotInGolden
            }
        } else if rows.contains(registry) {
            Treatment::Files
        } else {
            let outcome = outcomes.get(registry).unwrap_or_else(|| {
                panic!("no attempt to read {registry} from the booted app was made")
            });
            match outcome.reason() {
                None => Treatment::App,
                Some(reason) => Treatment::Listed(reason),
            }
        };
        listing.treatments.insert(registry.clone(), treatment);
    }

    for registry in rows {
        if !in_list.contains(registry.as_str()) {
            listing.problems.push(format!(
                "{registry} has a row and is not in the snapshot list"
            ));
        } else if !golden.contains(registry) {
            listing
                .problems
                .push(format!("{registry} has a row and the golden lacks it"));
        }
        if registry.starts_with("minecraft:worldgen/") {
            listing.problems.push(format!(
                "{registry} has a row; a registry under worldgen is compared from the booted app"
            ));
        }
    }
    for registry in required {
        if !rows.contains(*registry) {
            listing.problems.push(format!(
                "{registry} is always compared from its files and has no row"
            ));
        }
    }

    let mut seen = BTreeSet::new();
    for (registry, reason) in lines {
        if !seen.insert(registry.as_str()) {
            listing.problems.push(format!(
                "{registry} is on the fixture of registries not compared more than once"
            ));
            continue;
        }
        if !in_list.contains(registry.as_str()) {
            listing.problems.push(format!(
                "{registry} is on the fixture of registries not compared and names no registry of the snapshot list"
            ));
            continue;
        }
        if required.contains(&registry.as_str()) {
            listing.problems.push(format!(
                "{registry} is always compared and cannot be on the fixture of registries not compared"
            ));
        }
        if reason != NOT_SYNCHRONIZED && !reason.starts_with(NOT_OBTAINED) {
            listing.problems.push(format!(
                "{registry}: the reason `{reason}` has neither admitted form"
            ));
            continue;
        }
        match &listing.treatments[registry] {
            Treatment::Listed(expected) if expected == reason => {}
            Treatment::Listed(expected) => listing.problems.push(format!(
                "{registry}: the line states `{reason}` but this run's outcome is `{expected}`"
            )),
            Treatment::Files => listing.problems.push(format!(
                "{registry} is on the fixture of registries not compared and the test compares it from its files; it must be compared"
            )),
            Treatment::App => listing.problems.push(format!(
                "{registry} is on the fixture of registries not compared and the booted app holds it; it must be compared"
            )),
            Treatment::NotInGolden => listing.problems.push(format!(
                "{registry} is on the fixture of registries not compared and is not in the golden; it must be captured"
            )),
        }
    }
    for (registry, treatment) in &listing.treatments {
        if let Treatment::Listed(reason) = treatment
            && !seen.contains(registry.as_str())
        {
            listing.problems.push(format!(
                "{registry} is not compared and has no line on the fixture of registries not compared; add `{registry} {reason}`"
            ));
        }
    }
    listing
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Difference {
    path: String,
    ours: &'static str,
    game: &'static str,
}

fn kind(tag: Option<&NbtTag>) -> &'static str {
    match tag {
        None => "absent",
        Some(NbtTag::End) => "end",
        Some(NbtTag::Byte(_)) => "byte",
        Some(NbtTag::Short(_)) => "short",
        Some(NbtTag::Int(_)) => "int",
        Some(NbtTag::Long(_)) => "long",
        Some(NbtTag::Float(_)) => "float",
        Some(NbtTag::Double(_)) => "double",
        Some(NbtTag::ByteArray(_)) => "byte_array",
        Some(NbtTag::String(_)) => "string",
        Some(NbtTag::List(_)) => "list",
        Some(NbtTag::Compound(_)) => "compound",
        Some(NbtTag::IntArray(_)) => "int_array",
        Some(NbtTag::LongArray(_)) => "long_array",
    }
}

fn same_leaf(ours: &NbtTag, game: &NbtTag) -> bool {
    match (ours, game) {
        (NbtTag::End, NbtTag::End) => true,
        (NbtTag::Byte(a), NbtTag::Byte(b)) => a == b,
        (NbtTag::Short(a), NbtTag::Short(b)) => a == b,
        (NbtTag::Int(a), NbtTag::Int(b)) => a == b,
        (NbtTag::Long(a), NbtTag::Long(b)) => a == b,
        (NbtTag::Float(a), NbtTag::Float(b)) => a.to_bits() == b.to_bits(),
        (NbtTag::Double(a), NbtTag::Double(b)) => a.to_bits() == b.to_bits(),
        (NbtTag::ByteArray(a), NbtTag::ByteArray(b)) => a == b,
        (NbtTag::String(a), NbtTag::String(b)) => a == b,
        (NbtTag::IntArray(a), NbtTag::IntArray(b)) => a == b,
        (NbtTag::LongArray(a), NbtTag::LongArray(b)) => a == b,
        _ => false,
    }
}

fn child<'a>(
    compound: &'a mcrs_minecraft_nbt::compound::NbtCompound,
    key: &str,
) -> Option<&'a NbtTag> {
    compound
        .child_tags
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value)
}

fn visit(path: String, ours: Option<&NbtTag>, game: Option<&NbtTag>, out: &mut Vec<Difference>) {
    match (ours, game) {
        (Some(NbtTag::Compound(a)), Some(NbtTag::Compound(b))) => {
            let keys: BTreeSet<&String> = a
                .child_tags
                .iter()
                .chain(&b.child_tags)
                .map(|(key, _)| key)
                .collect();
            for key in keys {
                visit(format!("{path}.{key}"), child(a, key), child(b, key), out);
            }
        }
        (Some(NbtTag::List(a)), Some(NbtTag::List(b))) => {
            for index in 0..a.len().max(b.len()) {
                visit(format!("{path}[{index}]"), a.get(index), b.get(index), out);
            }
        }
        (a, b) => {
            let differs = match (a, b) {
                (Some(a), Some(b)) => kind(Some(a)) != kind(Some(b)) || !same_leaf(a, b),
                _ => true,
            };
            if differs {
                out.push(Difference {
                    path,
                    ours: kind(a),
                    game: kind(b),
                });
            }
        }
    }
}

fn walk(ours: &NbtTag, game: &NbtTag) -> Vec<Difference> {
    let mut out = Vec::new();
    visit("$".to_string(), Some(ours), Some(game), &mut out);
    out.sort();
    out
}

fn compound(entries: Vec<(&str, NbtTag)>) -> NbtTag {
    NbtTag::Compound(mcrs_minecraft_nbt::compound::NbtCompound {
        child_tags: entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    })
}

fn difference(path: &str, ours: &'static str, game: &'static str) -> Difference {
    Difference {
        path: path.to_string(),
        ours,
        game,
    }
}

#[test]
fn a_number_of_another_width_is_a_difference() {
    let ours = compound(vec![("a", NbtTag::Int(5))]);
    let game = compound(vec![("a", NbtTag::Long(5))]);
    assert_eq!(walk(&ours, &game), vec![difference("$.a", "int", "long")]);
}

#[test]
fn a_field_on_one_side_only_is_a_difference() {
    let ours = compound(vec![("a", NbtTag::Int(1))]);
    let game = compound(vec![("a", NbtTag::Int(1)), ("b", NbtTag::Int(2))]);
    assert_eq!(walk(&ours, &game), vec![difference("$.b", "absent", "int")]);
    assert_eq!(walk(&game, &ours), vec![difference("$.b", "int", "absent")]);
}

#[test]
fn compound_key_order_is_not_a_difference() {
    let ours = compound(vec![("a", NbtTag::Int(1)), ("b", NbtTag::Int(2))]);
    let game = compound(vec![("b", NbtTag::Int(2)), ("a", NbtTag::Int(1))]);
    assert_eq!(walk(&ours, &game), Vec::new());
}

#[test]
fn list_order_is_a_difference() {
    let ours = NbtTag::List(vec![NbtTag::Int(1), NbtTag::Int(2)]);
    let game = NbtTag::List(vec![NbtTag::Int(2), NbtTag::Int(1)]);
    assert!(!walk(&ours, &game).is_empty());
}

fn names(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| name.to_string()).collect()
}

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| name.to_string()).collect()
}

fn line(registry: &str, reason: &str) -> (String, String) {
    (registry.to_string(), reason.to_string())
}

#[test]
fn a_registry_the_booted_app_holds_cannot_be_listed() {
    let list = names(&["minecraft:a", "minecraft:b", "minecraft:c"]);
    let golden = set(&["minecraft:a", "minecraft:b", "minecraft:c"]);
    let rows = set(&["minecraft:a"]);
    let attempts = |b: Outcome, c: Outcome| {
        BTreeMap::from([
            ("minecraft:b".to_string(), b),
            ("minecraft:c".to_string(), c),
        ])
    };
    let check = |lines: &[(String, String)], b: Outcome, c: Outcome| {
        check_listing(
            &list,
            &golden,
            &rows,
            lines,
            &attempts(b, c),
            &["minecraft:a"],
        )
    };

    let held = check(
        &[line(
            "minecraft:b",
            "not obtained from a booted app: no snapshot of that name",
        )],
        Outcome::Obtained,
        Outcome::Obtained,
    );
    assert!(
        held.problems
            .iter()
            .any(|p| p.contains("minecraft:b") && p.contains("must be compared")),
        "{:?}",
        held.problems
    );

    let unlisted = check(&[], Outcome::NoSnapshot, Outcome::Obtained);
    assert!(
        unlisted.problems.iter().any(|p| p.contains("minecraft:b")),
        "{:?}",
        unlisted.problems
    );

    let other = check(
        &[line(
            "minecraft:b",
            "not obtained from a booted app: an entry without a value",
        )],
        Outcome::NoSnapshot,
        Outcome::Obtained,
    );
    assert!(
        other
            .problems
            .iter()
            .any(|p| p.contains("minecraft:b") && p.contains("no snapshot of that name")),
        "{:?}",
        other.problems
    );

    let accepted = check(
        &[line(
            "minecraft:b",
            "not obtained from a booted app: no snapshot of that name",
        )],
        Outcome::NoSnapshot,
        Outcome::Obtained,
    );
    assert!(accepted.problems.is_empty(), "{:?}", accepted.problems);
    assert_eq!(
        accepted.treatments["minecraft:b"].line("minecraft:b"),
        "treatment: minecraft:b not obtained from a booted app: no snapshot of that name"
    );
}

const SECTION_UNTYPED_COPIES: &str = "[registries whose untyped copies are replaced]";
const SECTION_OTHERS: &str = "[other compared registries]";

fn required() -> Vec<&'static str> {
    UNTYPED_COPIES
        .iter()
        .copied()
        .chain(["minecraft:dialog"])
        .collect()
}

struct Row {
    registry: String,
    directory: &'static str,
    tag: fn(&str) -> Result<NbtTag, String>,
}

fn typed<T: DeserializeOwned + Serialize>(text: &str) -> Result<NbtTag, String> {
    let value: T = serde_json::from_str(text).map_err(|e| e.to_string())?;
    mcrs_minecraft_nbt::to_nbt_tag(&value).map_err(|e| e.to_string())
}

fn dialog(text: &str) -> Result<NbtTag, String> {
    let raw = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let dialog = mcrs_minecraft_world::dialog::Dialog { raw, dialogs: None };
    mcrs_minecraft_nbt::to_nbt_tag(&dialog).map_err(|e| e.to_string())
}

fn row(directory: &'static str, tag: fn(&str) -> Result<NbtTag, String>) -> Row {
    Row {
        registry: format!("minecraft:{directory}"),
        directory,
        tag,
    }
}

fn rows() -> Vec<Row> {
    use mcrs_minecraft_world::{
        banner_pattern::BannerPattern, block_transformer::BlockTransformer, chat_type::ChatType,
        damage_type::DamageType, decorated_pot_pattern::DecoratedPotPattern,
        instrument::Instrument, item::asset::TrimMaterial, item::asset::TrimPattern,
        jukebox_song::JukeboxSong, painting_variant::PaintingVariant, test_types::TestEnvironment,
        test_types::TestInstance, variant::*,
    };
    vec![
        row("banner_pattern", typed::<BannerPattern>),
        row("block_transformer", typed::<BlockTransformer>),
        row("cat_sound_variant", typed::<CatSoundVariant>),
        row("cat_variant", typed::<CatVariant>),
        row("chat_type", typed::<ChatType>),
        row("chicken_sound_variant", typed::<ChickenSoundVariant>),
        row("chicken_variant", typed::<ChickenVariant>),
        row("cow_sound_variant", typed::<CowSoundVariant>),
        row("cow_variant", typed::<CowVariant>),
        row("damage_type", typed::<DamageType>),
        row("decorated_pot_pattern", typed::<DecoratedPotPattern>),
        row("dialog", dialog),
        row("frog_variant", typed::<FrogVariant>),
        row("instrument", typed::<Instrument>),
        row("jukebox_song", typed::<JukeboxSong>),
        row("painting_variant", typed::<PaintingVariant>),
        row("pig_sound_variant", typed::<PigSoundVariant>),
        row("pig_variant", typed::<PigVariant>),
        row("test_environment", typed::<TestEnvironment>),
        row("test_instance", typed::<TestInstance>),
        row("trim_material", typed::<TrimMaterial>),
        row("trim_pattern", typed::<TrimPattern>),
        row("wolf_sound_variant", typed::<WolfSoundVariant>),
        row("wolf_variant", typed::<WolfVariant>),
        row(
            "world_clock",
            typed::<mcrs_minecraft_environment::world_clock::WorldClock>,
        ),
        row("zombie_nautilus_variant", typed::<ZombieNautilusVariant>),
    ]
}

fn shipped_files(directory: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) {
    let entries =
        std::fs::read_dir(directory).unwrap_or_else(|e| panic!("{}: {e}", directory.display()));
    for entry in entries {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if path.is_dir() {
            shipped_files(&path, &format!("{prefix}{name}/"), out);
        } else if let Some(stem) = name.strip_suffix(".json") {
            out.push((format!("minecraft:{prefix}{stem}"), path));
        }
    }
}

fn from_files(row: &Row) -> BTreeMap<String, NbtTag> {
    let directory = crate_path("../../assets/minecraft").join(row.directory);
    let mut files = Vec::new();
    shipped_files(&directory, "", &mut files);
    files
        .into_iter()
        .map(|(entry, path)| {
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let tag = (row.tag)(&text).unwrap_or_else(|e| panic!("{} {entry}: {e}", row.registry));
            (entry, tag)
        })
        .collect()
}

fn run_to_playing() -> App {
    std::env::set_current_dir(workspace_root()).unwrap();

    let mut app = App::new();
    app.add_plugins(TaskPoolPlugin {
        task_pool_options: bevy_app::TaskPoolOptions::with_num_threads(2),
    });
    app.add_plugins(StatesPlugin);
    bevy_asset::AssetApp::register_asset_source(
        &mut app,
        bevy_asset::io::AssetSourceId::Default,
        mcrs_minecraft_worldgen::bevy::asset_source("assets"),
    );
    app.add_plugins(AssetPlugin {
        watch_for_changes_override: Some(false),
        ..Default::default()
    });
    app.add_plugins(mcrs_minecraft_assets::MinecraftCorePlugin);
    app.add_plugins(MinecraftWorldPlugin);
    app.finish();
    app.cleanup();

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        app.update();
        if *app.world().resource::<State<AppState>>().get() == AppState::Playing {
            return app;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "never reached Playing"
        );
    }
}

fn workspace_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

fn from_app(app: &App, registry: &str) -> (Outcome, BTreeMap<String, NbtTag>) {
    let access = app.world().resource::<RegistryAccess>();
    let Some(snapshot) = access
        .iter()
        .find(|snapshot| snapshot.registry_key() == registry)
    else {
        return (Outcome::NoSnapshot, BTreeMap::new());
    };
    let mut entries = BTreeMap::new();
    for entry in snapshot.iter_entries() {
        match &entry.data {
            Some(tag) => {
                entries.insert(entry.location.to_string(), tag.clone());
            }
            None => return (Outcome::EntryWithoutValue, BTreeMap::new()),
        }
    }
    (Outcome::Obtained, entries)
}

fn render(registry: &str, entry: &str, difference: &Difference) -> String {
    let Difference { path, ours, game } = difference;
    if ours == game {
        format!("{registry} {path} ours={ours} game={game} differs in {entry}")
    } else {
        format!("{registry} {path} ours={ours} game={game}")
    }
}

fn compare_registry(
    registry: &str,
    ours: &BTreeMap<String, NbtTag>,
    game: &BTreeMap<String, NbtTag>,
) -> BTreeSet<String> {
    let entries: BTreeSet<&String> = ours.keys().chain(game.keys()).collect();
    let mut lines = BTreeSet::new();
    for entry in entries {
        match (ours.get(entry), game.get(entry)) {
            (Some(ours), Some(game)) => {
                for difference in walk(ours, game) {
                    lines.insert(render(registry, entry, &difference));
                }
            }
            (ours, game) => {
                lines.insert(format!(
                    "{registry} entry {entry} ours={} game={}",
                    kind(ours),
                    kind(game)
                ));
            }
        }
    }
    lines
}

fn sections(text: &str) -> BTreeMap<String, Vec<String>> {
    let mut sections: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut current = None;
    for line in text.lines().filter(|line| !line.is_empty()) {
        if line.starts_with('[') {
            current = Some(line.to_string());
            sections.entry(line.to_string()).or_default();
        } else {
            sections
                .get_mut(
                    current
                        .as_deref()
                        .expect("a line before any section header"),
                )
                .unwrap()
                .push(line.to_string());
        }
    }
    sections
}

#[test]
fn every_synced_registry_is_compared_or_listed() {
    let list = snapshot_list();
    let golden: BTreeSet<String> = golden().registries.keys().cloned().collect();
    let rows: BTreeSet<String> = rows().into_iter().map(|row| row.registry).collect();
    let app = run_to_playing();

    let outcomes: BTreeMap<String, Outcome> = list
        .iter()
        .filter(|registry| golden.contains(*registry) && !rows.contains(*registry))
        .map(|registry| (registry.clone(), from_app(&app, registry).0))
        .collect();

    let listing = check_listing(
        &list,
        &golden,
        &rows,
        &not_compared_lines(),
        &outcomes,
        &required(),
    );
    for registry in &list {
        println!("{}", listing.treatments[registry].line(registry));
    }
    assert!(
        listing.problems.is_empty(),
        "{}",
        listing.problems.join("\n")
    );
}

#[test]
fn the_synced_values_differ_from_the_game_as_recorded() {
    let list = snapshot_list();
    let golden = golden();
    let rows = rows();
    let app = run_to_playing();

    let mut untyped_copies = BTreeSet::new();
    let mut others = BTreeSet::new();
    for registry in &list {
        let Some(game) = golden.registries.get(registry) else {
            continue;
        };
        let ours = match rows.iter().find(|row| &row.registry == registry) {
            Some(row) => from_files(row),
            None => match from_app(&app, registry) {
                (Outcome::Obtained, entries) => entries,
                _ => continue,
            },
        };
        let lines = compare_registry(registry, &ours, game);
        if UNTYPED_COPIES.contains(&registry.as_str()) {
            if lines.is_empty() {
                untyped_copies.insert(format!("{registry} no differences"));
            }
            untyped_copies.extend(lines);
        } else {
            others.extend(lines);
        }
    }

    let computed = BTreeMap::from([
        (
            SECTION_UNTYPED_COPIES.to_string(),
            untyped_copies.into_iter().collect::<Vec<_>>(),
        ),
        (
            SECTION_OTHERS.to_string(),
            others.into_iter().collect::<Vec<_>>(),
        ),
    ]);
    for (section, lines) in &computed {
        println!("difference: {section}");
        for line in lines {
            println!("difference: {line}");
        }
    }
    assert_eq!(
        sections(&read("tests/fixtures/registry_value_differences.txt")),
        computed,
        "the recorded differences and the computed ones disagree"
    );
}
