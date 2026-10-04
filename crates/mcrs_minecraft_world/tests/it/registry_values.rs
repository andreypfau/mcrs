use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use bevy_app::App;
use mcrs_minecraft_assets::{PackSource, RegistryAccess};
use mcrs_minecraft_nbt::snbt::parse_tag;
use mcrs_minecraft_nbt::tag::NbtTag;

const UNTYPED_COPIES: [&str; 6] = [
    "minecraft:banner_pattern",
    "minecraft:instrument",
    "minecraft:jukebox_song",
    "minecraft:painting_variant",
    "minecraft:trim_material",
    "minecraft:trim_pattern",
];

const SYNCHRONIZED_BY_THE_GAME: [&str; 32] = [
    "minecraft:banner_pattern",
    "minecraft:block_transformer",
    "minecraft:cat_sound_variant",
    "minecraft:cat_variant",
    "minecraft:chat_type",
    "minecraft:chicken_sound_variant",
    "minecraft:chicken_variant",
    "minecraft:cow_sound_variant",
    "minecraft:cow_variant",
    "minecraft:damage_type",
    "minecraft:decorated_pot_pattern",
    "minecraft:dialog",
    "minecraft:dimension_type",
    "minecraft:enchantment",
    "minecraft:frog_variant",
    "minecraft:instrument",
    "minecraft:jukebox_song",
    "minecraft:painting_variant",
    "minecraft:pig_sound_variant",
    "minecraft:pig_variant",
    "minecraft:sulfur_cube_archetype",
    "minecraft:test_environment",
    "minecraft:test_instance",
    "minecraft:timeline",
    "minecraft:trim_material",
    "minecraft:trim_pattern",
    "minecraft:wolf_sound_variant",
    "minecraft:wolf_variant",
    "minecraft:world_clock",
    "minecraft:worldgen/biome",
    "minecraft:worldgen/block_state_provider",
    "minecraft:zombie_nautilus_variant",
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

const SECTION_UNTYPED_COPIES: &str = "[registries whose untyped copies are replaced]";
const SECTION_OTHERS: &str = "[other compared registries]";

fn from_app(app: &App, registry: &str) -> Option<BTreeMap<String, NbtTag>> {
    let access = app.world().resource::<RegistryAccess>();
    let snapshot = access
        .iter()
        .find(|snapshot| snapshot.registry_key() == registry)?;
    let vanilla = PackSource::vanilla_core();
    snapshot
        .iter_entries()
        .filter(|entry| {
            entry.pack_source.as_ref().is_some_and(|source| {
                source.namespace == vanilla.namespace && source.id == vanilla.id
            })
        })
        .map(|entry| Some((entry.location.to_string(), entry.data.clone()?)))
        .collect()
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

pub fn the_synced_values_differ_from_the_game_as_recorded(app: &App) {
    let golden = golden();

    let mut untyped_copies = BTreeSet::new();
    let mut others = BTreeSet::new();
    for (registry, game) in &golden.registries {
        let Some(ours) = from_app(app, registry) else {
            continue;
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

pub fn the_app_projects_exactly_the_registries_the_game_synchronizes(app: &App) {
    let projected: BTreeSet<&str> = app
        .world()
        .resource::<RegistryAccess>()
        .iter()
        .map(|snapshot| snapshot.registry_key())
        .collect();
    assert_eq!(projected, BTreeSet::from(SYNCHRONIZED_BY_THE_GAME));
}
