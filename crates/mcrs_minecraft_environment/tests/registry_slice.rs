use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_minecraft_core::rl;
use mcrs_minecraft_environment::timeline::{TimeMarker, Tracks};
use mcrs_minecraft_environment::world_clock::WorldClock;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_registry::{Entries, Id, Registry, RegistrySet, ScopeError};
use mcrs_minecraft_worldgen_testing::{assets_dir, json_files};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
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
