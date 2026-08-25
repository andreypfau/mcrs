use std::path::PathBuf;

use serde_json::json;

use super::*;
use mcrs_core::tag::file::TagEntry;

use crate::attribute::attribute;
use crate::dimension::dimension_type::ProtoDimensionType;
use crate::world_clock::{ClockState, WorldClocks};

const NOON: i64 = 6000;
const MIDNIGHT: i64 = 18000;

fn assets_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("assets/minecraft")
}

fn dimension_type(name: &str) -> ProtoDimensionType {
    let bytes = std::fs::read(assets_dir().join("dimension_type").join(name)).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn timeline(name: &str) -> Timeline {
    let bytes = std::fs::read(assets_dir().join("timeline").join(name)).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// The timelines a tag names, read from the shipped tag files and flattened
/// through `#` references, in the order the tag file lists them.
fn tagged_timelines(tag: &str) -> Vec<Timeline> {
    fn collect(tag: &str, out: &mut Vec<String>) {
        let path = assets_dir()
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

fn shape<'a>(id: &'a str, proto: &'a ProtoDimensionType) -> DimensionEnvironment<'a> {
    DimensionEnvironment {
        id,
        attributes: &proto.attributes,
        skybox: proto.skybox,
        has_skylight: proto.has_skylight,
        has_ceiling: proto.has_ceiling,
    }
}

fn build(id: &str, file: &str, timelines: &[Timeline]) -> EnvironmentAttributes {
    let proto = dimension_type(file);
    let borrowed: Vec<&Timeline> = timelines.iter().collect();
    EnvironmentAttributes::build(&shape(id, &proto), &borrowed).unwrap()
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
        &UniformBiomes(Arc::new(
            BiomeAttributes::bake(&serde_json::from_value(json).unwrap()).unwrap(),
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
fn layer_three_is_the_biome_and_beats_the_dimension() {
    let (attributes, _timelines) = overworld();
    let swamp = biomes(json!({"minecraft:visual/sky_color": "#6a7039"}));
    let ticks = ticks_at(&attributes, NOON, 0.0);
    let ctx = context(&ticks, &swamp, Weather::default());

    assert_eq!(
        color(&attributes, "minecraft:visual/sky_color", &ctx),
        0xFF6A_7039
    );
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

#[test]
fn overworld_sky_color_at_noon_is_the_dimension_colour_through_the_day_track() {
    let (attributes, _timelines) = overworld();
    let empty = SpatialAttributeInterpolator::default();

    let mut clocks = WorldClocks::default();
    clocks.insert(
        ResourceLocation::parse("minecraft:overworld").unwrap(),
        ClockState {
            total_ticks: NOON,
            ..ClockState::default()
        },
    );
    let mut ticks = Vec::new();
    attributes.clock_ticks(&clocks, &mut ticks);
    assert_eq!(ticks, vec![NOON as f64]);

    let ctx = context(&ticks, &empty, Weather::default());
    assert_eq!(
        color(&attributes, "minecraft:visual/sky_color", &ctx),
        0xFF78_A7FF
    );

    // and the same track takes it to black at midnight
    clocks.get_mut("minecraft:overworld").unwrap().total_ticks = MIDNIGHT;
    attributes.clock_ticks(&clocks, &mut ticks);
    assert_eq!(
        color(
            &attributes,
            "minecraft:visual/sky_color",
            &context(&ticks, &empty, Weather::default())
        ),
        0xFF00_0000
    );
}

// ── Sub-tick smoothing ───────────────────────────────────────────────────────

fn wrap_degrees(angle: f32) -> f32 {
    let mut wrapped = angle % 360.0;
    if wrapped >= 180.0 {
        wrapped -= 360.0;
    }
    if wrapped < -180.0 {
        wrapped += 360.0;
    }
    wrapped
}

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
fn evaluation_is_a_pure_function_of_its_context() {
    let (attributes, _timelines) = overworld();
    let empty = SpatialAttributeInterpolator::default();
    let sun = "minecraft:visual/sun_angle";

    let early = ticks_at(&attributes, 0, 0.0);
    let late = ticks_at(&attributes, 3000, 0.0);
    let first = float(
        &attributes,
        sun,
        &context(&early, &empty, Weather::default()),
    );
    let moved = float(
        &attributes,
        sun,
        &context(&late, &empty, Weather::default()),
    );
    let again = float(
        &attributes,
        sun,
        &context(&early, &empty, Weather::default()),
    );

    assert_ne!(first, moved, "a later tick must give a different angle");
    assert_eq!(
        first, again,
        "going back to the earlier tick must give the earlier angle"
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
    }

    fn rl(id: &str) -> ResourceLocation<Arc<str>> {
        ResourceLocation::parse(id).unwrap()
    }

    let alpha = overriding(0.25);
    let zulu = overriding(0.75);

    let mut tag_files = Assets::<TagFile>::default();
    let nested = tag_files.add(TagFile {
        replace: false,
        values: vec![TagEntry::Element(rl("test:alpha"))],
    });
    let listing = TagFile {
        replace: false,
        values: vec![TagEntry::Element(rl("test:zulu")), TagEntry::Tag(nested)],
    };

    let index =
        DynRegistryIndex::<Timeline>::build([rl("test:alpha"), rl("test:zulu")].into_iter());
    let order = resolve_tag_file_ordered(&listing, &tag_files, &index);
    assert_eq!(
        order,
        vec![
            index.get("test:zulu").unwrap(),
            index.get("test:alpha").unwrap()
        ]
    );

    let ordered: Vec<&Timeline> = order
        .iter()
        .map(|id| match index.location(*id).unwrap().as_str() {
            "test:alpha" => &alpha,
            "test:zulu" => &zulu,
            other => panic!("unexpected member {other}"),
        })
        .collect();

    let proto = dimension_type("overworld.json");
    let attributes =
        EnvironmentAttributes::build(&shape("minecraft:overworld", &proto), &ordered).unwrap();

    let ticks = ticks_at(&attributes, 0, 0.0);
    let empty = SpatialAttributeInterpolator::default();
    let ctx = context(&ticks, &empty, Weather::default());
    assert_eq!(
        float(&attributes, "minecraft:gameplay/sky_light_level", &ctx),
        0.25
    );
}
