use mcrs_minecraft_core::Registered;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_core::mth::wrap_degrees;

use serde::de::DeserializeOwned;
use serde_json::json;

use super::*;
use mcrs_minecraft_core::TagId;
use mcrs_minecraft_registries::{Named, Resolve, Resolver};
use mcrs_minecraft_worldgen_testing::vanilla_dir;

use crate::dimension_type::{DimensionTypeEnvironment, DimensionTypeFile};
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_environment::attribute::attribute;
use mcrs_minecraft_environment::world_clock::{ClockState, WorldClocks};

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
        key: &ResourceKey<R>,
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
        _: &mcrs_minecraft_core::TagKey<R>,
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

fn dimension_key(id: &str) -> ResourceKey<Dimension> {
    ResourceKey::from_location(id.parse().unwrap())
}

fn shape<'a>(
    id: &str,
    (dimension_type, environment): &'a (DimensionType, DimensionTypeEnvironment),
) -> DimensionEnvironment<'a> {
    DimensionEnvironment::of(&dimension_key(id), dimension_type, environment)
}

fn build(id: &str, file: &str, timelines: &[Timeline]) -> EnvironmentAttributes {
    let proto = dimension_type(file);
    let borrowed: Vec<&Timeline> = timelines.iter().collect();
    EnvironmentAttributes::build(&shape(id, &proto), &borrowed, clock_count()).unwrap()
}

fn overworld() -> (EnvironmentAttributes, Vec<Timeline>) {
    let timelines = tagged_timelines("in_overworld");
    (
        build("minecraft:overworld", "overworld.json", &timelines),
        timelines,
    )
}

/// The tick each clock stands at, read the way a frame reads it.
fn ticks_at(attributes: &EnvironmentAttributes, total_ticks: i64, partial_tick: f32) -> Vec<f64> {
    let mut clocks = WorldClocks::default();
    for clock in attributes.clocks() {
        clocks.insert(
            *clock,
            ClockState {
                total_ticks,
                partial_tick,
                ..ClockState::default()
            },
        );
    }
    let mut ticks = Vec::new();
    attributes.clock_ticks(&clocks, &mut ticks);
    ticks
}

fn context(ticks: &[f64]) -> EnvironmentContext<'_> {
    EnvironmentContext { ticks }
}

fn color(attributes: &EnvironmentAttributes, id: &str, ctx: &EnvironmentContext) -> u32 {
    match attributes.value(id, ctx).unwrap() {
        AttributeValue::Color(packed) => packed,
        other => panic!("{id} is not a colour: {other:?}"),
    }
}

fn float(attributes: &EnvironmentAttributes, id: &str, ctx: &EnvironmentContext) -> f32 {
    match attributes.value(id, ctx).unwrap() {
        AttributeValue::Float(value) => value,
        other => panic!("{id} is not a float: {other:?}"),
    }
}

// ── The five layers, each overriding what is beneath it ──────────────────────

#[test]
fn layer_one_is_the_registered_default() {
    let (attributes, _timelines) = overworld();
    let ticks = ticks_at(&attributes, NOON, 0.0);
    let ctx = context(&ticks);

    // nothing in the overworld touches night_vision_color
    let spec = attribute("minecraft:visual/night_vision_color").unwrap();
    assert_eq!(spec.default, AttributeValue::Color(0xFF99_9999));
    assert_eq!(color(&attributes, spec.id, &ctx), 0xFF99_9999);
}

#[test]
fn layer_two_is_the_dimension_and_beats_the_default() {
    let (attributes, _timelines) = overworld();
    let ticks = ticks_at(&attributes, NOON, 0.0);
    let ctx = context(&ticks);

    let ambient = "minecraft:visual/ambient_light_color";
    assert_eq!(
        attribute(ambient).unwrap().default,
        AttributeValue::Color(0xFF00_0000)
    );
    assert_eq!(color(&attributes, ambient, &ctx), 0xFF0A_0A0A);
}

// ── Positional layers ────────────────────────────────────────────────────────

// ── End to end through a clock ───────────────────────────────────────────────

// ── Sub-tick smoothing ───────────────────────────────────────────────────────

#[test]
fn the_sun_crosses_the_wrap_without_reversing() {
    let (attributes, _timelines) = overworld();
    let sun = "minecraft:visual/sun_angle";

    // eight sub-tick steps either side of tick 6000, where the track's single
    // revolution meets its own start
    let angles: Vec<f32> = (0..160)
        .map(|step| {
            let total_ticks = NOON - 10 + step / 8;
            let partial_tick = (step % 8) as f32 / 8.0;
            let ticks = ticks_at(&attributes, total_ticks, partial_tick);
            float(&attributes, sun, &context(&ticks))
        })
        .collect();

    // The bezier ease solves its parameter to a tolerance, so consecutive
    // samples can disagree by a few thousandths of a degree; anything larger
    // going backwards would be the sun spinning the wrong way.
    let jitter = 0.01;
    let crossings = angles
        .windows(2)
        .filter(|pair| {
            let delta = wrap_degrees(pair[1] - pair[0]);
            assert!(
                delta > -jitter,
                "the sun went backwards: {} -> {}",
                pair[0],
                pair[1]
            );
            assert!(delta < 5.0, "the sun jumped: {} -> {}", pair[0], pair[1]);
            pair[1] < pair[0] - jitter
        })
        .count();
    assert_eq!(crossings, 1, "the sweep must cross 360 -> 0 exactly once");
    assert!(
        angles[0] > 300.0 && *angles.last().unwrap() < 60.0,
        "{angles:?}"
    );
}

#[test]
fn a_dimension_with_no_timelines_still_builds() {
    let attributes = build("minecraft:overworld", "overworld.json", &[]);
    assert!(attributes.clocks().is_empty());

    let ticks = ticks_at(&attributes, NOON, 0.0);
    assert!(ticks.is_empty());
    let ctx = context(&ticks);

    // With no track the attribute holds the dimension constant at every tick.
    let at_noon = float(&attributes, "minecraft:gameplay/sky_light_level", &ctx);
    let ticks = ticks_at(&attributes, MIDNIGHT, 0.0);
    let ctx = context(&ticks);
    assert_eq!(
        at_noon,
        float(&attributes, "minecraft:gameplay/sky_light_level", &ctx)
    );
}

/// Two timelines writing one attribute stack in the order the dimension's
/// tag lists them: the last one wins.
#[test]
fn the_timeline_listed_last_wins_the_attribute_they_share() {
    fn overriding(level: f32) -> Timeline {
        resolved(
            serde_json::from_value::<Timeline<Named>>(json!({
                "clock": "minecraft:overworld",
                "period_ticks": 24000,
                "tracks": {
                    "minecraft:gameplay/sky_light_level": {
                        "modifier": "override",
                        "keyframes": [{ "ticks": 0, "value": level }],
                    },
                },
            }))
            .unwrap(),
        )
    }

    let (zulu, alpha) = (overriding(0.75), overriding(0.25));
    let proto = dimension_type("overworld.json");
    let attributes = EnvironmentAttributes::build(
        &shape("minecraft:overworld", &proto),
        &[&zulu, &alpha],
        clock_count(),
    )
    .unwrap();

    let ticks = ticks_at(&attributes, 0, 0.0);
    let ctx = context(&ticks);
    assert_eq!(
        float(&attributes, "minecraft:gameplay/sky_light_level", &ctx),
        0.25
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
