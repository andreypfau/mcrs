use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_minecraft_core::rl;
use mcrs_minecraft_environment::timeline::{TimeMarker, Tracks};
use mcrs_minecraft_environment::world_clock::WorldClock;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_registry::{Entries, Id, Registry, RegistryError, RegistrySet, ScopeError};
use mcrs_minecraft_worldgen_testing::{assets_dir, json_files};
use serde::de::{self, DeserializeOwned, SeqAccess, Visitor, value};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

struct WorldClockKey;

impl RegistryKey for WorldClockKey {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:world_clock");
}

struct TimelineKey;

impl RegistryKey for TimelineKey {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:timeline");
}

// Does not run the period check of the library's `Timeline`, which is private to
// it: the row proves registry references, not timeline validation.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TimelineRow {
    clock: Id<WorldClockKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    period_ticks: Option<u32>,
    #[serde(default)]
    tracks: Tracks,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    time_markers: HashMap<String, TimeMarker>,
}

#[derive(Debug, PartialEq)]
enum TimelineSet {
    Tag(ResourceLocation<Arc<str>>),
    One(Id<TimelineKey>),
    List(Vec<Id<TimelineKey>>),
}

impl<'de> Deserialize<'de> for TimelineSet {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SetVisitor;

        impl<'de> Visitor<'de> for SetVisitor {
            type Value = TimelineSet;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a tag, an entry, or a list of entries")
            }

            fn visit_str<E: de::Error>(self, text: &str) -> Result<TimelineSet, E> {
                let Some(tag) = text.strip_prefix('#') else {
                    return Id::deserialize(value::StrDeserializer::new(text))
                        .map(TimelineSet::One);
                };
                let tag = ResourceLocation::read(tag).map_err(E::custom)?;
                let known = Registry::<TimelineKey>::in_scope("TimelineSet", |registry| {
                    registry.has_tag(tag.as_str())
                })
                .map_err(E::custom)?;
                if !known {
                    return Err(E::custom(format_args!(
                        "Missing tag: '{tag}' in '{}'",
                        TimelineKey::KEY
                    )));
                }
                Ok(TimelineSet::Tag(tag))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<TimelineSet, A::Error> {
                Vec::deserialize(value::SeqAccessDeserializer::new(seq)).map(TimelineSet::List)
            }
        }

        deserializer.deserialize_any(SetVisitor)
    }
}

impl Serialize for TimelineSet {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            TimelineSet::Tag(tag) => serializer.collect_str(&format_args!("#{tag}")),
            TimelineSet::One(id) => id.serialize(serializer),
            TimelineSet::List(ids) => ids.serialize(serializer),
        }
    }
}

#[derive(Deserialize)]
struct DimensionProbe {
    timelines: TimelineSet,
    #[serde(default)]
    default_clock: Option<Id<WorldClockKey>>,
}

fn corpus(dir: &str) -> PathBuf {
    assets_dir().join("minecraft").join(dir)
}

fn files(dir: &str) -> Vec<PathBuf> {
    json_files(&corpus(dir))
}

fn name_of(dir: &str, file: &Path) -> ResourceLocation<Arc<str>> {
    let relative = file
        .strip_prefix(corpus(dir))
        .expect("a corpus file is under its directory")
        .with_extension("");
    ResourceLocation::minecraft(&relative.to_string_lossy().replace('\\', "/"))
}

fn names(dir: &str) -> Vec<ResourceLocation<Arc<str>>> {
    files(dir).iter().map(|file| name_of(dir, file)).collect()
}

fn read_value(file: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(file).unwrap()).unwrap()
}

fn read_all<T: DeserializeOwned>(dir: &str) -> Vec<T> {
    files(dir)
        .iter()
        .map(|file| {
            serde_json::from_slice(&std::fs::read(file).unwrap())
                .unwrap_or_else(|error| panic!("{}: {error}", file.display()))
        })
        .collect()
}

struct Slice {
    clocks: Registry<WorldClockKey>,
    timelines: Registry<TimelineKey>,
    set: RegistrySet,
}

fn slice() -> Slice {
    let clocks = Registry::<WorldClockKey>::new(names("world_clock"), std::iter::empty()).unwrap();
    let timelines =
        Registry::<TimelineKey>::new(names("timeline"), names("tags/timeline")).unwrap();
    let set = RegistrySet::new()
        .with(clocks.clone())
        .unwrap()
        .with(timelines.clone())
        .unwrap();
    Slice {
        clocks,
        timelines,
        set,
    }
}

#[test]
fn both_registries_load_from_the_shipped_files_inside_a_scope() {
    let Slice {
        clocks,
        timelines,
        set,
    } = slice();
    RegistrySet::scope(&set, || {
        let clock_files = files("world_clock");
        let timeline_files = files("timeline");
        let clock_values = Entries::new(&clocks, read_all::<WorldClock>("world_clock")).unwrap();
        let rows = Entries::new(&timelines, read_all::<TimelineRow>("timeline")).unwrap();

        assert!(!Registry::is_empty(&clocks));
        assert!(!Registry::is_empty(&timelines));
        assert_eq!(Registry::len(&clocks), clock_files.len());
        assert_eq!(Registry::len(&timelines), timeline_files.len());

        for (position, id) in Registry::ids(&clocks).enumerate() {
            assert_eq!(Id::index(id), position);
            assert!(Entries::get(&clock_values, id).is_some());
        }
        for (position, id) in Registry::ids(&timelines).enumerate() {
            assert_eq!(Id::index(id), position);
            let row = Entries::get(&rows, id).unwrap();
            let file = read_value(&timeline_files[position]);
            let clock = Registry::key(&clocks, row.clock).unwrap();
            assert_eq!(Some(clock.as_str()), file["clock"].as_str());
            assert_eq!(
                Registry::get(&clocks, clock.as_str()),
                Some(row.clock),
                "{clock}"
            );
        }
    });
}

#[test]
fn every_shipped_timeline_round_trips_through_text_unchanged() {
    let Slice { set, .. } = slice();
    RegistrySet::scope(&set, || {
        let timeline_files = files("timeline");
        assert!(!timeline_files.is_empty());
        for file in &timeline_files {
            let bytes = std::fs::read(file).unwrap();
            let row: TimelineRow = serde_json::from_slice(&bytes).unwrap();
            let text = serde_json::to_string(&row).unwrap();
            let written: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(written, read_value(file), "{}", file.display());
            let again: TimelineRow = serde_json::from_str(&text).unwrap();
            assert_eq!(again.clock, row.clock, "{}", file.display());
        }
    });
}

#[test]
fn the_slice_encodes_to_nbt_with_the_clock_as_a_name() {
    let Slice { clocks, set, .. } = slice();
    RegistrySet::scope(&set, || {
        for file in files("timeline") {
            let value = read_value(&file);
            let row: TimelineRow = serde_json::from_value(value.clone()).unwrap();
            let nbt = mcrs_minecraft_nbt::to_nbt_compound(&row).unwrap();
            let clock = Registry::key(&clocks, row.clock).unwrap();
            assert_eq!(nbt.get_string("clock"), Some(clock.as_str()));
            match value["period_ticks"].as_u64() {
                Some(period) => assert_eq!(
                    nbt.get("period_ticks"),
                    Some(&NbtTag::Int(period as i32)),
                    "{}",
                    file.display()
                ),
                None => assert!(nbt.get("period_ticks").is_none()),
            }
        }
        let clock = mcrs_minecraft_nbt::to_nbt_compound(&WorldClock::default()).unwrap();
        assert!(clock.is_empty());
    });
}

#[test]
fn a_timeline_parsed_outside_a_scope_fails_naming_the_type() {
    let file = &files("timeline")[0];
    let error = serde_json::from_slice::<TimelineRow>(&std::fs::read(file).unwrap())
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("Id<"), "{error}");
    assert!(error.contains("minecraft:world_clock"), "{error}");
    assert!(matches!(
        Registry::<WorldClockKey>::in_scope("probe", |_| ()),
        Err(ScopeError::NoScope { .. })
    ));
}

fn parse<T: DeserializeOwned>(set: &RegistrySet, text: &str) -> Result<T, String> {
    RegistrySet::scope(set, || {
        serde_json::from_str(text).map_err(|error| error.to_string())
    })
}

fn write<T: Serialize>(set: &RegistrySet, value: &T) -> String {
    RegistrySet::scope(set, || serde_json::to_string(value).unwrap())
}

fn quoted(text: &str) -> String {
    serde_json::to_string(text).unwrap()
}

fn short(name: &ResourceLocation<Arc<str>>) -> &str {
    name.as_str().strip_prefix("minecraft:").unwrap()
}

#[test]
fn every_dimension_type_names_a_known_timeline_tag_and_clock() {
    let Slice {
        clocks,
        timelines,
        set,
    } = slice();
    let dimension_files = files("dimension_type");
    assert!(!dimension_files.is_empty());
    let mut without_clock = 0;
    RegistrySet::scope(&set, || {
        for file in &dimension_files {
            let value = read_value(file);
            let probe: DimensionProbe = serde_json::from_slice(&std::fs::read(file).unwrap())
                .unwrap_or_else(|error| panic!("{}: {error}", file.display()));
            match &probe.timelines {
                TimelineSet::Tag(tag) => {
                    assert!(Registry::has_tag(&timelines, tag.as_str()));
                    assert_eq!(
                        value["timelines"].as_str(),
                        Some(format!("#{tag}").as_str())
                    );
                }
                other => panic!("{}: expected a tag, found {other:?}", file.display()),
            }
            match (probe.default_clock, value.get("default_clock")) {
                (Some(id), Some(written)) => {
                    let name = Registry::key(&clocks, id).unwrap();
                    assert_eq!(Some(name.as_str()), written.as_str());
                }
                (None, None) => without_clock += 1,
                (found, written) => {
                    panic!("{}: read {found:?} from {written:?}", file.display())
                }
            }
        }
    });
    assert!(without_clock > 0);
}

#[test]
fn a_set_naming_an_unknown_tag_fails_naming_tag_and_registry() {
    let Slice { timelines, set, .. } = slice();
    for tag in names("tags/timeline") {
        assert!(Registry::has_tag(&timelines, tag.as_str()), "{tag}");
    }
    assert!(!Registry::has_tag(&timelines, "minecraft:nowhere"));
    let error = parse::<TimelineSet>(&set, "\"#minecraft:nowhere\"").unwrap_err();
    assert!(error.contains("Missing tag"), "{error}");
    assert!(error.contains("minecraft:nowhere"), "{error}");
    assert!(error.contains("minecraft:timeline"), "{error}");
}

#[test]
fn the_written_shapes_of_a_set_round_trip() {
    let Slice { set, .. } = slice();
    let tags = names("tags/timeline");
    let entries = names("timeline");
    assert!(entries.len() >= 2);
    let texts = [
        quoted(&format!("#{}", tags[0])),
        quoted(entries[0].as_str()),
        serde_json::to_string(&[entries[0].as_str(), entries[1].as_str()]).unwrap(),
        "[]".to_owned(),
    ];
    for text in texts {
        let parsed: TimelineSet = parse(&set, &text).unwrap();
        assert_eq!(write(&set, &parsed), text);
    }
    assert_eq!(
        parse::<TimelineSet>(&set, "[]").unwrap(),
        TimelineSet::List(Vec::new())
    );
}

#[test]
fn a_tag_written_without_its_namespace_is_the_same_tag() {
    let Slice { set, .. } = slice();
    let tag = &names("tags/timeline")[0];
    let bare: TimelineSet = parse(&set, &quoted(&format!("#{}", short(tag)))).unwrap();
    let full: TimelineSet = parse(&set, &quoted(&format!("#{tag}"))).unwrap();
    assert_eq!(bare, full);
    assert_eq!(write(&set, &bare), quoted(&format!("#{tag}")));
}

#[test]
fn a_list_keeps_the_order_it_was_written_in() {
    let Slice { timelines, set, .. } = slice();
    let entries = names("timeline");
    assert!(entries.len() >= 2);
    let (first, second) = (&entries[0], &entries[1]);
    for pair in [[first, second], [second, first]] {
        let text = serde_json::to_string(&[pair[0].as_str(), pair[1].as_str()]).unwrap();
        let parsed: TimelineSet = parse(&set, &text).unwrap();
        let expected = pair.map(|name| Registry::get(&timelines, name.as_str()).unwrap());
        assert_eq!(parsed, TimelineSet::List(expected.to_vec()));
        assert_eq!(write(&set, &parsed), text);
    }
}

#[test]
fn an_entry_name_is_not_a_tag_and_a_tag_name_is_not_an_entry() {
    let Slice { timelines, set, .. } = slice();
    let entry = &names("timeline")[0];
    let tag = &names("tags/timeline")[0];

    let error = parse::<TimelineSet>(&set, &quoted(&format!("#{entry}"))).unwrap_err();
    assert!(error.contains("Missing tag"), "{error}");
    assert!(error.contains(entry.as_str()), "{error}");
    assert!(error.contains("minecraft:timeline"), "{error}");
    assert!(!Registry::has_tag(&timelines, entry.as_str()));

    let error = parse::<TimelineSet>(&set, &quoted(tag.as_str())).unwrap_err();
    assert!(error.contains(tag.as_str()), "{error}");
    assert!(error.contains("minecraft:timeline"), "{error}");
    let unknown = Registry::require(&timelines, tag.as_str()).unwrap_err();
    assert_eq!(unknown.registry, TimelineKey::KEY);
    assert_eq!(unknown.name, tag.as_str());
}

#[test]
fn a_clock_of_another_registry_is_an_error_naming_registry_and_entry() {
    let Slice { set, .. } = slice();
    let timeline = &names("timeline")[0];
    let text = format!("{{\"clock\":{}}}", quoted(timeline.as_str()));
    let error = parse::<TimelineRow>(&set, &text).err().unwrap();
    assert!(error.contains("minecraft:world_clock"), "{error}");
    assert!(error.contains(timeline.as_str()), "{error}");
}

#[test]
fn a_clock_without_its_namespace_resolves_and_is_written_in_full() {
    let Slice { clocks, set, .. } = slice();
    let clock = &names("world_clock")[0];
    let text = format!("{{\"clock\":{}}}", quoted(short(clock)));
    let row: TimelineRow = parse(&set, &text).unwrap();
    assert_eq!(Some(row.clock), Registry::get(&clocks, clock.as_str()));
    let written: Value = serde_json::from_str(&write(&set, &row)).unwrap();
    assert_eq!(written["clock"].as_str(), Some(clock.as_str()));
}

#[test]
fn the_first_and_the_last_id_of_each_registry_round_trip() {
    let Slice {
        clocks,
        timelines,
        set,
    } = slice();
    fn check<R: RegistryKey>(
        registry: &Registry<R>,
        set: &RegistrySet,
        listed: Vec<ResourceLocation<Arc<str>>>,
    ) {
        let bounds = [
            (
                Registry::ids(registry).next().unwrap(),
                listed.first().unwrap(),
            ),
            (
                Registry::ids(registry).last().unwrap(),
                listed.last().unwrap(),
            ),
        ];
        for (id, name) in bounds {
            assert_eq!(Registry::key(registry, id), Some(name));
            let text = write(set, &id);
            assert_eq!(text, quoted(name.as_str()));
            assert_eq!(parse::<Id<R>>(set, &text), Ok(id));
        }
    }
    check(&clocks, &set, names("world_clock"));
    check(&timelines, &set, names("timeline"));
}

#[test]
fn ids_follow_the_sorted_file_list() {
    let first = slice();
    let second = slice();
    for (listed, built_first, built_second) in [
        (
            names("world_clock"),
            Registry::ids(&first.clocks)
                .map(|id| Registry::key(&first.clocks, id).cloned().unwrap())
                .collect::<Vec<_>>(),
            Registry::ids(&second.clocks)
                .map(|id| Registry::key(&second.clocks, id).cloned().unwrap())
                .collect::<Vec<_>>(),
        ),
        (
            names("timeline"),
            Registry::ids(&first.timelines)
                .map(|id| Registry::key(&first.timelines, id).cloned().unwrap())
                .collect::<Vec<_>>(),
            Registry::ids(&second.timelines)
                .map(|id| Registry::key(&second.timelines, id).cloned().unwrap())
                .collect::<Vec<_>>(),
        ),
    ] {
        assert_eq!(built_first, listed);
        assert_eq!(built_second, listed);
    }

    let held = RegistrySet::registry::<TimelineKey>(&first.set).unwrap();
    assert_eq!(Registry::len(&held), Registry::len(&first.timelines));
    assert!(RegistrySet::registry::<TimelineKey>(&RegistrySet::new()).is_none());
    assert!(RegistrySet::with(first.set.clone(), first.clocks.clone()).is_err());
}

#[test]
fn an_id_under_an_untagged_or_flattened_shape_still_resolves() {
    #[derive(Deserialize)]
    struct Holder {
        clock: Id<WorldClockKey>,
    }

    #[derive(Deserialize)]
    struct Flattened {
        #[serde(flatten)]
        holder: Holder,
        #[allow(dead_code)]
        extra: u32,
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Either {
        #[allow(dead_code)]
        Number(u32),
        Clock(Id<WorldClockKey>),
    }

    let Slice { clocks, set, .. } = slice();
    let clock = &names("world_clock")[0];
    let expected = Registry::get(&clocks, clock.as_str()).unwrap();

    let text = format!("{{\"clock\":{},\"extra\":3}}", quoted(clock.as_str()));
    let flattened: Flattened = parse(&set, &text).unwrap();
    assert_eq!(flattened.holder.clock, expected);

    match parse::<Either>(&set, &quoted(clock.as_str())).unwrap() {
        Either::Clock(id) => assert_eq!(id, expected),
        Either::Number(_) => panic!("a name is not a number"),
    }
}

#[test]
fn a_column_of_the_wrong_length_does_not_build() {
    let Slice { timelines, .. } = slice();
    let held = Registry::len(&timelines);
    let error = Entries::new(&timelines, vec![0u8; held - 1]).err().unwrap();
    assert!(
        matches!(
            error,
            RegistryError::LengthMismatch { expected, found, .. }
                if expected == held && found == held - 1
        ),
        "{error}"
    );
    assert!(error.to_string().contains("minecraft:timeline"), "{error}");
}

#[test]
fn an_empty_world_clock_round_trips_as_an_empty_object() {
    let Slice { set, .. } = slice();
    for file in files("world_clock") {
        let bytes = std::fs::read(&file).unwrap();
        let clock: WorldClock = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(write(&set, &clock), "{}");
        assert_eq!(read_value(&file), serde_json::json!({}));
    }
}
