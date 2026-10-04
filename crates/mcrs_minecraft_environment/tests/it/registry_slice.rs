use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bevy_ecs::prelude::*;
use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_minecraft_core::rl;
use mcrs_minecraft_environment::timeline::{TimeMarker, Tracks};
use mcrs_minecraft_environment::world_clock::WorldClock;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_registry::{Entries, Id, Registry, RegistrySet};
use mcrs_minecraft_worldgen_testing::{assets_dir, json_files};
use serde::de::{self, DeserializeOwned, SeqAccess, Visitor, value};
use serde::{Deserialize, Deserializer, Serialize};
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
fn an_empty_world_clock_round_trips_as_an_empty_object() {
    let Slice { set, .. } = slice();
    for file in files("world_clock") {
        let bytes = std::fs::read(&file).unwrap();
        let clock: WorldClock = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(write(&set, &clock), "{}");
        assert_eq!(read_value(&file), serde_json::json!({}));
    }
}
