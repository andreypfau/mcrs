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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Obtained,
    NoSnapshot,
    EntryWithoutValue,
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
fn a_field_on_one_side_only_is_a_difference() {
    let ours = compound(vec![("a", NbtTag::Int(1))]);
    let game = compound(vec![("a", NbtTag::Int(1)), ("b", NbtTag::Int(2))]);
    assert_eq!(walk(&ours, &game), vec![difference("$.b", "absent", "int")]);
    assert_eq!(walk(&game, &ours), vec![difference("$.b", "int", "absent")]);
}

const SECTION_UNTYPED_COPIES: &str = "[registries whose untyped copies are replaced]";
const SECTION_OTHERS: &str = "[other compared registries]";

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
        block_transformer::BlockTransformer, chat_type::ChatType, damage_type::DamageType,
        decorated_pot_pattern::DecoratedPotPattern, item::asset::BannerPattern,
        item::asset::Instrument, item::asset::JukeboxSong, item::asset::PaintingVariant,
        item::asset::TrimMaterial, item::asset::TrimPattern, test_types::TestEnvironment,
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
fn the_synced_values_differ_from_the_game_as_recorded() {
    let golden = golden();
    let rows = rows();
    let app = run_to_playing();

    let mut untyped_copies = BTreeSet::new();
    let mut others = BTreeSet::new();
    for (registry, game) in &golden.registries {
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

#[test]
fn the_typed_values_encode_as_the_game_does() {
    let golden = golden();
    let rows = rows();
    let mut differences = BTreeSet::new();
    for registry in UNTYPED_COPIES {
        let row = rows
            .iter()
            .find(|row| row.registry == registry)
            .unwrap_or_else(|| panic!("{registry} has no row"));
        let game = golden
            .registries
            .get(registry)
            .unwrap_or_else(|| panic!("the golden lacks {registry}"));
        differences.extend(compare_registry(registry, &from_files(row), game));
    }
    assert!(
        differences.is_empty(),
        "{}",
        differences.into_iter().collect::<Vec<_>>().join("\n")
    );
}
