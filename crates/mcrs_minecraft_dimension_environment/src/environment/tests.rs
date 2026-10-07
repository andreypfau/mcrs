use mcrs_minecraft_core::mth::wrap_degrees;
use std::sync::{Arc, LazyLock};

use serde_json::json;

use super::*;
use mcrs_minecraft_core::{ResourceLocation, TagKey};
use mcrs_minecraft_registry::Tags;
use mcrs_minecraft_registry::tags::{TagRules, TagSource, build_tags};
use mcrs_minecraft_worldgen_testing::assets_dir;

use crate::dimension_type::{DimensionTypeEnvironment, DimensionTypeFile};
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_environment::attribute::attribute;
use mcrs_minecraft_environment::world_clock::WorldClock;
use mcrs_minecraft_environment::world_clock::{ClockState, WorldClocks};

const NOON: i64 = 6000;
const MIDNIGHT: i64 = 18000;

fn dimension_type(name: &str) -> (DimensionType, DimensionTypeEnvironment) {
    let bytes = std::fs::read(assets_dir().join("minecraft/dimension_type").join(name)).unwrap();
    mcrs_minecraft_worldgen_testing::dimension_type_set()
        .scope(|| serde_json::from_slice::<DimensionTypeFile>(&bytes))
        .unwrap()
        .split()
}

static CLOCKS: LazyLock<RegistrySet> = LazyLock::new(|| {
    mcrs_minecraft_worldgen_testing::shipped_registry_set::<WorldClock>("world_clock")
});

fn clock_registry() -> Registry<WorldClock> {
    CLOCKS.registry().unwrap()
}

fn timeline(name: &str) -> Timeline {
    let bytes = std::fs::read(assets_dir().join("minecraft/timeline").join(name)).unwrap();
    CLOCKS.scope(|| serde_json::from_slice(&bytes).unwrap())
}

/// The timelines a tag names, read from the shipped tag files and flattened
/// through `#` references, in the order the tag file lists them.
fn tagged_timelines(tag: &str) -> Vec<Timeline> {
    fn collect(tag: &str, out: &mut Vec<String>) {
        let path = assets_dir()
            .join("minecraft/tags/timeline")
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
    EnvironmentAttributes::build(&shape(id, &proto), &borrowed, &clock_registry()).unwrap()
}

fn overworld() -> (EnvironmentAttributes, Vec<Timeline>) {
    let timelines = tagged_timelines("in_overworld");
    (
        build("minecraft:overworld", "overworld.json", &timelines),
        timelines,
    )
}

fn biomes(json: serde_json::Value) -> SpatialAttributeInterpolator {
    let mut interpolator = SpatialAttributeInterpolator::default();
    interpolator.sample(
        DVec3::ZERO,
        &Arc::new(BiomeAttributes::bake(
            &serde_json::from_value(json).unwrap(),
        )),
    );
    interpolator
}

/// The tick each clock stands at, read the way a frame reads it.
fn ticks_at(attributes: &EnvironmentAttributes, total_ticks: i64, partial_tick: f32) -> Vec<f64> {
    let mut clocks = WorldClocks::default();
    for clock in attributes.clocks() {
        clocks.insert(
            clock.clone(),
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

fn context<'a>(
    ticks: &'a [f64],
    biomes: &'a SpatialAttributeInterpolator,
    weather: Weather,
) -> EnvironmentContext<'a> {
    EnvironmentContext {
        position: DVec3::ZERO,
        ticks,
        biomes,
        weather,
    }
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
    let empty = SpatialAttributeInterpolator::default();
    let ticks = ticks_at(&attributes, NOON, 0.0);
    let ctx = context(&ticks, &empty, Weather::default());

    // nothing in the overworld touches night_vision_color
    let spec = attribute("minecraft:visual/night_vision_color").unwrap();
    assert_eq!(spec.default, AttributeValue::Color(0xFF99_9999));
    assert_eq!(color(&attributes, spec.id, &ctx), 0xFF99_9999);
}

#[test]
fn layer_two_is_the_dimension_and_beats_the_default() {
    let (attributes, _timelines) = overworld();
    let empty = SpatialAttributeInterpolator::default();
    let ticks = ticks_at(&attributes, NOON, 0.0);
    let ctx = context(&ticks, &empty, Weather::default());

    let ambient = "minecraft:visual/ambient_light_color";
    assert_eq!(
        attribute(ambient).unwrap().default,
        AttributeValue::Color(0xFF00_0000)
    );
    assert_eq!(color(&attributes, ambient, &ctx), 0xFF0A_0A0A);
}

#[test]
fn layer_four_is_the_timeline_and_beats_the_biome() {
    let (attributes, _timelines) = overworld();
    let swamp = biomes(json!({"minecraft:visual/sky_color": "#6a7039"}));

    let noon = ticks_at(&attributes, NOON, 0.0);
    let midnight = ticks_at(&attributes, MIDNIGHT, 0.0);
    let sky = "minecraft:visual/sky_color";

    // the day track multiplies by white at noon and by black at midnight
    assert_eq!(
        color(
            &attributes,
            sky,
            &context(&noon, &swamp, Weather::default())
        ),
        0xFF6A_7039
    );
    assert_eq!(
        color(
            &attributes,
            sky,
            &context(&midnight, &swamp, Weather::default())
        ),
        0xFF00_0000
    );
}

#[test]
fn layer_five_is_weather_and_beats_the_timeline() {
    let (attributes, _timelines) = overworld();
    let empty = SpatialAttributeInterpolator::default();
    let ticks = ticks_at(&attributes, NOON, 0.0);
    let sky = "minecraft:visual/sky_color";

    let clear = color(
        &attributes,
        sky,
        &context(&ticks, &empty, Weather::default()),
    );
    let raining = color(
        &attributes,
        sky,
        &context(
            &ticks,
            &empty,
            Weather {
                rain: 1.0,
                thunder: 0.0,
            },
        ),
    );
    let thundering = color(
        &attributes,
        sky,
        &context(
            &ticks,
            &empty,
            Weather {
                rain: 1.0,
                thunder: 1.0,
            },
        ),
    );

    assert_eq!(clear, 0xFF78_A7FF);
    assert_ne!(raining, clear, "rain blends the sky towards grey");
    assert_ne!(thundering, raining, "thunder blends it further");

    let grey = |packed: u32| {
        let channel = |shift: u32| (packed >> shift & 0xFF) as i32;
        (channel(16) - channel(0)).abs()
    };
    assert!(
        grey(thundering) < grey(raining),
        "thunder is the greyer of the two"
    );
}

#[test]
fn a_dimension_without_weather_has_no_weather_layer() {
    let end = build("minecraft:the_end", "the_end.json", &[]);
    let empty = SpatialAttributeInterpolator::default();
    let ticks = ticks_at(&end, 0, 0.0);

    let sky = "minecraft:visual/sky_color";
    assert_eq!(
        color(
            &end,
            sky,
            &context(
                &ticks,
                &empty,
                Weather {
                    rain: 1.0,
                    thunder: 1.0
                }
            )
        ),
        color(&end, sky, &context(&ticks, &empty, Weather::default())),
    );
}

#[test]
fn weather_follows_the_dimension_key() {
    let empty = SpatialAttributeInterpolator::default();
    let light = "minecraft:gameplay/sky_light_level";
    let storm = Weather {
        rain: 1.0,
        thunder: 1.0,
    };
    let weathered = |attributes: &EnvironmentAttributes| {
        let ticks = ticks_at(attributes, NOON, 0.0);
        float(attributes, light, &context(&ticks, &empty, storm))
            != float(
                attributes,
                light,
                &context(&ticks, &empty, Weather::default()),
            )
    };

    let overworld_type = "overworld.json";
    let the_end_type = "the_end.json";
    let cases = [
        ("minecraft:the_end", overworld_type, false),
        ("minecraft:overworld", the_end_type, true),
        ("minecraft:overworld", overworld_type, true),
        ("test:extra", overworld_type, true),
    ];
    for (key, type_file, expected) in cases {
        assert_eq!(
            weathered(&build(key, type_file, &[])),
            expected,
            "a dimension keyed {key} of the type in {type_file}"
        );
    }
}

// ── Positional layers ────────────────────────────────────────────────────────

#[test]
fn a_not_positional_attribute_skips_the_positional_layers() {
    let (attributes, _timelines) = overworld();
    let loud_biome = biomes(json!({
        "minecraft:gameplay/sky_light_level": 1.0,
        "minecraft:visual/sky_color": "#6a7039",
    }));
    let ticks = ticks_at(&attributes, NOON, 0.0);
    let ctx = context(&ticks, &loud_biome, Weather::default());

    let level = attribute("minecraft:gameplay/sky_light_level").unwrap();
    assert!(!level.positional);
    assert_eq!(
        float(&attributes, level.id, &ctx),
        15.0,
        "a biome must not be able to move a not-positional attribute"
    );
    // …while the biome does reach the positional attribute beside it
    assert_eq!(
        color(&attributes, "minecraft:visual/sky_color", &ctx),
        0xFF6A_7039
    );
}

// ── End to end through a clock ───────────────────────────────────────────────

// ── Sub-tick smoothing ───────────────────────────────────────────────────────

#[test]
fn the_sun_crosses_the_wrap_without_reversing() {
    let (attributes, _timelines) = overworld();
    let empty = SpatialAttributeInterpolator::default();
    let sun = "minecraft:visual/sun_angle";

    // eight sub-tick steps either side of tick 6000, where the track's single
    // revolution meets its own start
    let angles: Vec<f32> = (0..160)
        .map(|step| {
            let total_ticks = NOON - 10 + step / 8;
            let partial_tick = (step % 8) as f32 / 8.0;
            let ticks = ticks_at(&attributes, total_ticks, partial_tick);
            float(
                &attributes,
                sun,
                &context(&ticks, &empty, Weather::default()),
            )
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
    let empty = SpatialAttributeInterpolator::default();
    let ctx = context(&ticks, &empty, Weather::default());

    // With no track the attribute holds the dimension constant at every tick.
    let at_noon = float(&attributes, "minecraft:gameplay/sky_light_level", &ctx);
    let ticks = ticks_at(&attributes, MIDNIGHT, 0.0);
    let ctx = context(&ticks, &empty, Weather::default());
    assert_eq!(
        at_noon,
        float(&attributes, "minecraft:gameplay/sky_light_level", &ctx)
    );
}

#[test]
fn a_composed_value_is_clamped_back_into_its_range() {
    let (attributes, _timelines) = overworld();
    let empty = SpatialAttributeInterpolator::default();
    let ticks = ticks_at(&attributes, MIDNIGHT, 0.0);
    let ctx = context(
        &ticks,
        &empty,
        Weather {
            rain: 1.0,
            thunder: 1.0,
        },
    );

    let factor = float(&attributes, "minecraft:visual/sky_light_factor", &ctx);
    assert!(
        (0.0..=1.0).contains(&factor),
        "sky_light_factor is a unit float, got {factor}"
    );

    let level = float(&attributes, "minecraft:gameplay/sky_light_level", &ctx);
    assert!(
        (0.0..=15.0).contains(&level),
        "sky_light_level is [0; 15], got {level}"
    );
}

/// Two timelines writing one attribute stack in the order the tag file lists
/// them. The nested tag is listed second but names the alphabetically first
/// timeline, so resolving by registry id would let the wrong one win.
#[test]
fn the_timeline_a_tag_lists_last_wins_the_attribute_they_share() {
    fn overriding(level: f32) -> Timeline {
        CLOCKS.scope(|| {
            serde_json::from_value(json!({
                "clock": "minecraft:overworld",
                "period_ticks": 24000,
                "tracks": {
                    "minecraft:gameplay/sky_light_level": {
                        "modifier": "override",
                        "keyframes": [{ "ticks": 0, "value": level }],
                    },
                },
            }))
            .unwrap()
        })
    }

    fn rl(id: &str) -> ResourceLocation<Arc<str>> {
        ResourceLocation::read(id).unwrap()
    }

    let alpha = overriding(0.25);
    let zulu = overriding(0.75);

    let registry = Registry::<mcrs_minecraft_environment::timeline::Timeline>::new(
        mcrs_minecraft_environment::keys::TIMELINE,
        [rl("test:alpha"), rl("test:zulu")],
    )
    .unwrap();
    let source = |path, bytes| TagSource {
        pack: "test",
        path,
        bytes,
    };
    let files = [
        (
            rl("test:listing"),
            vec![source(
                "test/tags/timeline/listing.json",
                br##"{"values":["test:zulu","#test:nested"]}"##,
            )],
        ),
        (
            rl("test:nested"),
            vec![source(
                "test/tags/timeline/nested.json",
                br#"{"values":["test:alpha"]}"#,
            )],
        ),
    ];
    let (table, problems) = build_tags(registry.table(), TagRules::World, &files, None);
    assert!(problems.is_empty(), "{problems:?}");
    let tags = Tags::<mcrs_minecraft_environment::timeline::Timeline>::new(Arc::new(table));
    let listing = tags
        .get(
            &TagKey::<mcrs_minecraft_environment::timeline::Timeline, _>::from_location(rl(
                "test:listing",
            )),
        )
        .unwrap();
    let order: Vec<_> = tags.members(listing).collect();
    assert_eq!(
        order,
        vec![
            registry.by_name("test:zulu").unwrap(),
            registry.by_name("test:alpha").unwrap()
        ]
    );

    let ordered: Vec<&Timeline> = order
        .iter()
        .map(|id| match registry.name(*id).unwrap().as_str() {
            "test:alpha" => &alpha,
            "test:zulu" => &zulu,
            other => panic!("unexpected member {other}"),
        })
        .collect();

    let proto = dimension_type("overworld.json");
    let attributes = EnvironmentAttributes::build(
        &shape("minecraft:overworld", &proto),
        &ordered,
        &clock_registry(),
    )
    .unwrap();

    let ticks = ticks_at(&attributes, 0, 0.0);
    let empty = SpatialAttributeInterpolator::default();
    let ctx = context(&ticks, &empty, Weather::default());
    assert_eq!(
        float(&attributes, "minecraft:gameplay/sky_light_level", &ctx),
        0.25
    );
}

#[test]
fn a_dimension_type_outside_the_games_bounds_fails() {
    let overworld: serde_json::Value = serde_json::from_slice(
        &std::fs::read(assets_dir().join("minecraft/dimension_type/overworld.json")).unwrap(),
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
        mcrs_minecraft_worldgen_testing::dimension_type_set()
            .scope(|| serde_json::from_value::<DimensionTypeFile>(file))
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
