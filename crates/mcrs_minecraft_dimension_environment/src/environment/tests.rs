use serde::de::DeserializeOwned;
use serde_json::json;

use super::*;
use mcrs_minecraft_registries::{Named, Resolve, Resolver};
use mcrs_minecraft_resource::{Registered, TagId, TagKeyBuf};
use mcrs_minecraft_worldgen_testing::vanilla_dir;

use crate::dimension_type::{DimensionTypeEnvironment, DimensionTypeFile};
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_environment::environment_attribute_keys::EnvironmentAttribute as A;

const NOON: i64 = 6000;
const MIDNIGHT: i64 = 18000;

/// Numbers every key by its place among the shipped world clocks, the only
/// registry these values name an entry of, and every tag as the first.
struct ShippedClocks(Vec<String>);

impl ShippedClocks {
    fn new() -> Self {
        let mut names: Vec<String> = std::fs::read_dir(vanilla_dir().join("world_clock"))
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                format!("minecraft:{}", path.file_stem().unwrap().to_str().unwrap())
            })
            .collect();
        names.sort();
        ShippedClocks(names)
    }
}

impl Resolver for ShippedClocks {
    fn id<R: Registered>(
        &self,
        key: &ResourceKeyBuf<R>,
    ) -> Result<Id<R>, mcrs_minecraft_registries::ResolveError> {
        let at = self
            .0
            .iter()
            .position(|name| name == key.location().as_str());
        at.and_then(|at| Id::checked(at as u16, self.0.len()))
            .ok_or_else(|| format!("{} is not a shipped clock", key.location()).into())
    }

    fn tag<R: Registered>(
        &self,
        _: &TagKeyBuf<R>,
    ) -> Result<TagId<R>, mcrs_minecraft_registries::ResolveError> {
        Ok(TagId::checked(0, 1).unwrap())
    }
}

fn resolved<T: Resolve>(named: T::Named) -> T {
    T::resolve(named, &ShippedClocks::new()).unwrap()
}

fn read<T: Resolve>(dir: &str, name: &str) -> T
where
    T::Named: DeserializeOwned,
{
    let bytes = std::fs::read(vanilla_dir().join(dir).join(name)).unwrap();
    resolved(serde_json::from_slice(&bytes).unwrap())
}

fn clock_count() -> usize {
    ShippedClocks::new().0.len()
}

fn dimension_type(name: &str) -> (DimensionType, DimensionTypeEnvironment) {
    read::<DimensionTypeFile>("dimension_type", name).split()
}

fn timeline(name: &str) -> Timeline {
    read("timeline", name)
}

/// The timelines a tag names, read from the shipped tag files and flattened
/// through `#` references, in the order the tag file lists them.
fn tagged_timelines(tag: &str) -> Vec<Timeline> {
    fn collect(tag: &str, out: &mut Vec<String>) {
        let path = vanilla_dir()
            .join("tags/timeline")
            .join(format!("{}.json", tag.trim_start_matches("minecraft:")));
        let file: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for value in file["values"].as_array().unwrap() {
            let entry = value.as_str().unwrap();
            match entry.strip_prefix('#') {
                Some(nested) => collect(nested, out),
                None => out.push(entry.trim_start_matches("minecraft:").to_owned()),
            }
        }
    }
    let mut names = Vec::new();
    collect(tag, &mut names);
    names
        .iter()
        .map(|name| timeline(&format!("{name}.json")))
        .collect()
}

fn overworld_timeline(tracks: serde_json::Value) -> Timeline {
    resolved(
        serde_json::from_value::<Timeline<Named>>(json!({
            "clock": "minecraft:overworld",
            "period_ticks": 24000,
            "tracks": tracks,
        }))
        .unwrap(),
    )
}

fn dimension_key(id: &str) -> ResourceKeyBuf<Dimension> {
    ResourceKeyBuf::from_location(id.parse().unwrap())
}

fn build(file: &str, timelines: &[Timeline]) -> DimensionAttributes {
    let (dimension_type, environment) = dimension_type(file);
    let borrowed: Vec<&Timeline> = timelines.iter().collect();
    let shape = DimensionEnvironment::of(
        &dimension_key("minecraft:overworld"),
        &dimension_type,
        &environment,
    );
    DimensionAttributes::build(&shape, &borrowed, clock_count()).unwrap()
}

fn overworld() -> DimensionAttributes {
    build("overworld.json", &tagged_timelines("in_overworld"))
}

fn ticks_at(attributes: &DimensionAttributes, total_ticks: i64) -> Vec<i64> {
    vec![total_ticks; attributes.clocks().len()]
}

fn float(attributes: &DimensionAttributes, attribute: A, ticks: &[i64]) -> f32 {
    match attributes
        .value(attribute, &EnvironmentContext { ticks })
        .unwrap()
    {
        AttributeValue::Float(value) => value,
        other => panic!("{attribute:?} is not a float: {other:?}"),
    }
}

#[test]
fn an_attribute_no_layer_names_holds_its_default() {
    let attributes = overworld();
    let ticks = ticks_at(&attributes, NOON);
    let gray = 153.0 / 255.0;
    assert_eq!(
        attributes.value(
            A::VisualNightVisionColor,
            &EnvironmentContext { ticks: &ticks }
        ),
        Some(AttributeValue::Rgb([gray, gray, gray]))
    );
}

#[test]
fn the_dimension_type_entry_replaces_the_default() {
    let attributes = overworld();
    let ticks = ticks_at(&attributes, NOON);
    let ten = 10.0 / 255.0;
    assert_eq!(
        attributes.value(
            A::VisualAmbientLightColor,
            &EnvironmentContext { ticks: &ticks }
        ),
        Some(AttributeValue::Rgb([ten, ten, ten]))
    );
}

#[test]
fn a_dimension_with_no_timelines_holds_its_values_at_every_tick() {
    let attributes = build("overworld.json", &[]);
    assert!(attributes.clocks().is_empty());
    assert_eq!(
        float(&attributes, A::GameplaySkyLightLevel, &[]),
        float(&attributes, A::GameplaySkyLightLevel, &[MIDNIGHT])
    );
}

#[test]
fn the_timeline_listed_last_wins_the_attribute_they_share() {
    let overriding = |level: f32| {
        overworld_timeline(json!({
            "minecraft:gameplay/sky_light_level": {
                "keyframes": [{ "ticks": 0, "value": level }],
            },
        }))
    };
    let attributes = build("overworld.json", &[overriding(7.0), overriding(3.0)]);
    assert_eq!(float(&attributes, A::GameplaySkyLightLevel, &[0]), 3.0);
}

#[test]
fn the_range_clamps_once_after_the_last_layer() {
    let shift = |modifier: &str, amount: f32| {
        overworld_timeline(json!({
            "minecraft:gameplay/sky_light_level": {
                "modifier": modifier,
                "keyframes": [{ "ticks": 0, "value": amount }],
            },
        }))
    };
    let raised = build("overworld.json", &[shift("add", 10.0)]);
    assert_eq!(float(&raised, A::GameplaySkyLightLevel, &[0]), 15.0);

    // A clamp between the layers would leave 15 - 10.
    let raised_then_lowered = build(
        "overworld.json",
        &[shift("add", 10.0), shift("subtract", 10.0)],
    );
    assert_eq!(
        float(&raised_then_lowered, A::GameplaySkyLightLevel, &[0]),
        15.0
    );
}

#[test]
fn a_track_reads_whole_ticks_and_a_frame_moves_between_two_tick_values() {
    let ramp = overworld_timeline(json!({
        "minecraft:visual/cloud_height": {
            "keyframes": [{ "ticks": 0, "value": 0.0 }, { "ticks": 100, "value": 100.0 }],
        },
    }));
    let attributes = build("overworld.json", &[ramp]);
    assert_eq!(float(&attributes, A::VisualCloudHeight, &[10]), 10.0);

    let stack = attributes.stack(A::VisualCloudHeight).unwrap();
    let frame = stack.frame_value(
        &EnvironmentContext { ticks: &[10] },
        &EnvironmentContext { ticks: &[11] },
        0.25,
    );
    assert_eq!(frame, AttributeValue::Float(10.25));
}

#[test]
fn the_sun_moves_forward_between_the_ticks_around_its_wrap() {
    let attributes = overworld();
    let stack = attributes.stack(A::VisualSunAngle).unwrap();
    let ticks = |total| ticks_at(&attributes, total);
    let (before, after) = (ticks(NOON - 1), ticks(NOON));
    let previous = float(&attributes, A::VisualSunAngle, &before);
    let current = float(&attributes, A::VisualSunAngle, &after);
    let AttributeValue::Float(halfway) = stack.frame_value(
        &EnvironmentContext { ticks: &before },
        &EnvironmentContext { ticks: &after },
        0.5,
    ) else {
        panic!("the sun angle is a float");
    };
    let forward = (current - previous).rem_euclid(360.0);
    assert!(forward < 1.0, "{previous} -> {current}");
    let moved = (halfway - previous).rem_euclid(360.0);
    assert!(
        (moved - forward / 2.0).abs() < 1.0e-3,
        "{previous} -> {halfway} -> {current}"
    );
}

#[test]
fn a_dimension_type_outside_the_games_bounds_fails() {
    let overworld: serde_json::Value = serde_json::from_slice(
        &std::fs::read(vanilla_dir().join("dimension_type/overworld.json")).unwrap(),
    )
    .unwrap();
    let with = |changes: serde_json::Value| {
        let mut file = overworld.clone();
        for (key, value) in changes.as_object().unwrap() {
            match value {
                serde_json::Value::Null => {
                    file.as_object_mut().unwrap().remove(key);
                }
                value => file[key] = value.clone(),
            }
        }
        serde_json::from_value::<DimensionTypeFile<Named>>(file)
    };
    for refused in [
        json!({"weather": true}),
        json!({"has_ender_dragon_fight": null}),
        json!({"coordinate_scale": 0.0}),
        json!({"coordinate_scale": 3.1e7}),
        json!({"min_y": -2048}),
        json!({"min_y": -56}),
        json!({"min_y": 1792}),
        json!({"height": 8, "logical_height": 8}),
        json!({"height": 4080, "min_y": -2032}),
        json!({"height": 392}),
        json!({"logical_height": 400}),
        json!({"logical_height": -1}),
        json!({"monster_spawn_block_light_limit": 16}),
        json!({"monster_spawn_light_level": {"type": "minecraft:uniform", "min_inclusive": 0, "max_inclusive": 16}}),
    ] {
        assert!(with(refused.clone()).is_err(), "{refused} loads");
    }
    for accepted in [
        json!({"min_y": -2032, "height": 4064, "logical_height": 4064}),
        json!({"coordinate_scale": 1.0e-5}),
        json!({"coordinate_scale": 3.0e7}),
    ] {
        if let Err(error) = with(accepted.clone()) {
            panic!("{accepted}: {error}");
        }
    }
}
