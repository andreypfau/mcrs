use std::path::PathBuf;

use serde_json::json;

use super::sky::{SkyEffects, SkyField, SkyFrame, SkyLayout, SkyValue};
use super::*;
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

/// `#minecraft:in_overworld`, flattened in the order the tag files list it.
fn overworld_timelines() -> Vec<Timeline> {
    ["villager_schedule.json", "day.json", "moon.json", "early_game.json"]
        .map(timeline)
        .into()
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
    let timelines = overworld_timelines();
    (build("minecraft:overworld", "overworld.json", &timelines), timelines)
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
            ClockState { total_ticks, partial_tick, ..ClockState::default() },
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
    EnvironmentContext { position: DVec3::ZERO, ticks, biomes, weather }
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
    assert_eq!(attribute(ambient).unwrap().default, AttributeValue::Color(0xFF00_0000));
    assert_eq!(color(&attributes, ambient, &ctx), 0xFF0A_0A0A);
}

#[test]
fn layer_three_is_the_biome_and_beats_the_dimension() {
    let (attributes, _timelines) = overworld();
    let swamp = biomes(json!({"minecraft:visual/sky_color": "#6a7039"}));
    let ticks = ticks_at(&attributes, NOON, 0.0);
    let ctx = context(&ticks, &swamp, Weather::default());

    assert_eq!(color(&attributes, "minecraft:visual/sky_color", &ctx), 0xFF6A_7039);
}

#[test]
fn layer_four_is_the_timeline_and_beats_the_biome() {
    let (attributes, _timelines) = overworld();
    let swamp = biomes(json!({"minecraft:visual/sky_color": "#6a7039"}));

    let noon = ticks_at(&attributes, NOON, 0.0);
    let midnight = ticks_at(&attributes, MIDNIGHT, 0.0);
    let sky = "minecraft:visual/sky_color";

    // the day track multiplies by white at noon and by black at midnight
    assert_eq!(color(&attributes, sky, &context(&noon, &swamp, Weather::default())), 0xFF6A_7039);
    assert_eq!(
        color(&attributes, sky, &context(&midnight, &swamp, Weather::default())),
        0xFF00_0000
    );
}

#[test]
fn layer_five_is_weather_and_beats_the_timeline() {
    let (attributes, _timelines) = overworld();
    let empty = SpatialAttributeInterpolator::default();
    let ticks = ticks_at(&attributes, NOON, 0.0);
    let sky = "minecraft:visual/sky_color";

    let clear = color(&attributes, sky, &context(&ticks, &empty, Weather::default()));
    let raining = color(
        &attributes,
        sky,
        &context(&ticks, &empty, Weather { rain: 1.0, thunder: 0.0 }),
    );
    let thundering = color(
        &attributes,
        sky,
        &context(&ticks, &empty, Weather { rain: 1.0, thunder: 1.0 }),
    );

    assert_eq!(clear, 0xFF78_A7FF);
    assert_ne!(raining, clear, "rain blends the sky towards grey");
    assert_ne!(thundering, raining, "thunder blends it further");

    let grey = |packed: u32| {
        let channel = |shift: u32| (packed >> shift & 0xFF) as i32;
        (channel(16) - channel(0)).abs()
    };
    assert!(grey(thundering) < grey(raining), "thunder is the greyer of the two");
}

#[test]
fn a_dimension_without_weather_has_no_weather_layer() {
    let end = build("minecraft:the_end", "the_end.json", &[]);
    let empty = SpatialAttributeInterpolator::default();
    let ticks = ticks_at(&end, 0, 0.0);

    let sky = "minecraft:visual/sky_color";
    assert_eq!(
        color(&end, sky, &context(&ticks, &empty, Weather { rain: 1.0, thunder: 1.0 })),
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
    assert_eq!(color(&attributes, "minecraft:visual/sky_color", &ctx), 0xFF6A_7039);
}

// ── End to end through a clock ───────────────────────────────────────────────

#[test]
fn overworld_sky_color_at_noon_is_the_dimension_colour_through_the_day_track() {
    let (attributes, _timelines) = overworld();
    let empty = SpatialAttributeInterpolator::default();

    let mut clocks = WorldClocks::default();
    clocks.insert(
        ResourceLocation::parse("minecraft:overworld").unwrap(),
        ClockState { total_ticks: NOON, ..ClockState::default() },
    );
    let mut ticks = Vec::new();
    attributes.clock_ticks(&clocks, &mut ticks);
    assert_eq!(ticks, vec![NOON as f64]);

    let ctx = context(&ticks, &empty, Weather::default());
    assert_eq!(color(&attributes, "minecraft:visual/sky_color", &ctx), 0xFF78_A7FF);

    // and the same track takes it to black at midnight
    clocks.get_mut("minecraft:overworld").unwrap().total_ticks = MIDNIGHT;
    attributes.clock_ticks(&clocks, &mut ticks);
    assert_eq!(
        color(&attributes, "minecraft:visual/sky_color", &context(&ticks, &empty, Weather::default())),
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
            float(&attributes, sun, &context(&ticks, &empty, Weather::default()))
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
            assert!(delta > -jitter, "the sun went backwards: {} -> {}", pair[0], pair[1]);
            assert!(delta < 5.0, "the sun jumped: {} -> {}", pair[0], pair[1]);
            pair[1] < pair[0] - jitter
        })
        .count();
    assert_eq!(crossings, 1, "the sweep must cross 360 -> 0 exactly once");
    assert!(angles[0] > 300.0 && *angles.last().unwrap() < 60.0, "{angles:?}");
}

#[test]
fn evaluation_is_a_pure_function_of_its_context() {
    let (attributes, _timelines) = overworld();
    let empty = SpatialAttributeInterpolator::default();
    let sun = "minecraft:visual/sun_angle";

    let early = ticks_at(&attributes, 0, 0.0);
    let late = ticks_at(&attributes, 3000, 0.0);
    let first = float(&attributes, sun, &context(&early, &empty, Weather::default()));
    let moved = float(&attributes, sun, &context(&late, &empty, Weather::default()));
    let again = float(&attributes, sun, &context(&early, &empty, Weather::default()));

    assert_ne!(first, moved, "a later tick must give a different angle");
    assert_eq!(first, again, "going back to the earlier tick must give the earlier angle");
}

// ── The two sky blocks ───────────────────────────────────────────────────────

#[test]
fn the_dimension_constant_set_comes_from_the_loaded_timelines() {
    let (attributes, _timelines) = overworld();
    let layout = SkyLayout::derive(&attributes);

    let frame: Vec<SkyField> = layout.frame_fields().collect();
    let constant: Vec<SkyField> = layout.constant_fields().collect();
    assert_eq!(frame.len() + constant.len(), SkyField::ALL.len());
    assert_eq!(frame.len(), 11, "{frame:?}");
    assert_eq!(constant.len(), 11, "the 13 constants less the two particle payloads");

    for field in [
        SkyField::SkyColor,
        SkyField::FogColor,
        SkyField::CloudColor,
        SkyField::SkyLightColor,
        SkyField::SkyLightFactor,
        SkyField::SunriseSunsetColor,
        SkyField::StarBrightness,
        SkyField::SunAngle,
        SkyField::MoonAngle,
        SkyField::StarAngle,
        SkyField::MoonPhase,
    ] {
        assert!(frame.contains(&field), "{field:?} has a track and belongs in the frame block");
    }
    assert!(constant.contains(&SkyField::CloudHeight));
}

#[test]
fn a_track_for_cloud_height_moves_it_into_the_frame_block() {
    let mut timelines = overworld_timelines();
    timelines.push(
        serde_json::from_value(json!({
            "clock": "minecraft:overworld",
            "period_ticks": 24000,
            "tracks": {
                "minecraft:visual/cloud_height": {
                    "keyframes": [{"ticks": 0, "value": 192.33}, {"ticks": 12000, "value": 128.0}],
                },
            },
        }))
        .unwrap(),
    );
    let attributes = build("minecraft:overworld", "overworld.json", &timelines);
    let layout = SkyLayout::derive(&attributes);

    let frame: Vec<SkyField> = layout.frame_fields().collect();
    assert!(frame.contains(&SkyField::CloudHeight), "{frame:?}");
    assert!(!layout.constant_fields().any(|field| field == SkyField::CloudHeight));
    assert_eq!(frame.len(), 12);
}

#[test]
fn one_pass_fills_the_frame_block_in_layout_order() {
    let (attributes, _timelines) = overworld();
    let layout = SkyLayout::derive(&attributes);
    let empty = SpatialAttributeInterpolator::default();
    let ticks = ticks_at(&attributes, NOON, 0.0);
    let weather = Weather { rain: 0.5, thunder: 0.25 };

    let mut frame = SkyFrame::default();
    let ctx = EnvironmentContext {
        position: DVec3::new(8.0, 64.0, -8.0),
        ticks: &ticks,
        biomes: &empty,
        weather,
    };
    layout.evaluate(&attributes, &ctx, &mut frame);

    assert_eq!(frame.values.len(), layout.frame_fields().count());
    assert_eq!(frame.camera, [8.0, 64.0, -8.0]);
    assert_eq!((frame.rain, frame.thunder), (0.5, 0.25));
    assert_eq!(frame.get(&layout, SkyField::SunAngle), Some(SkyValue::Scalar(0.0)));

    // a second pass over the same context reuses the buffer and lands on the
    // same values: nothing is carried between frames
    let before = frame.values.clone();
    layout.evaluate(&attributes, &ctx, &mut frame);
    assert_eq!(frame.values, before);
}

#[test]
fn the_overworld_draws_the_whole_sky() {
    let (attributes, _timelines) = overworld();
    let layout = SkyLayout::derive(&attributes);
    let empty = SpatialAttributeInterpolator::default();
    let ticks = ticks_at(&attributes, NOON, 0.0);

    let constants = layout.constants(&attributes, &context(&ticks, &empty, Weather::default()));
    assert_eq!(constants.key.skybox, Skybox::Overworld);
    assert_eq!(constants.key.effects, SkyEffects::all());
    assert_eq!(constants.key.draws(), 5);
    assert_eq!(constants.get(SkyField::CloudHeight), Some(SkyValue::Scalar(192.33)));
}

#[test]
fn the_nether_has_no_sun_moon_stars_or_clouds() {
    let nether_timelines = [timeline("villager_schedule.json")];
    let borrowed: Vec<&Timeline> = nether_timelines.iter().collect();
    let proto = dimension_type("the_nether.json");
    let attributes =
        EnvironmentAttributes::build(&shape("minecraft:the_nether", &proto), &borrowed).unwrap();

    let layout = SkyLayout::derive(&attributes);
    let empty = SpatialAttributeInterpolator::default();
    let ticks = ticks_at(&attributes, NOON, 0.0);
    let ctx = context(&ticks, &empty, Weather::default());
    let constants = layout.constants(&attributes, &ctx);

    assert_eq!(constants.key.skybox, Skybox::None);
    assert_eq!(constants.key.effects, SkyEffects::DISC);
    assert_eq!(constants.key.draws(), 1);

    let overworld_draws = {
        let (overworld, _timelines) = overworld();
        SkyLayout::derive(&overworld)
            .constants(&overworld, &context(&ticks_at(&overworld, NOON, 0.0), &empty, Weather::default()))
            .key
            .draws()
    };
    assert_eq!(overworld_draws - constants.key.draws(), 4);

    for id in [
        "minecraft:visual/sun_angle",
        "minecraft:visual/moon_angle",
        "minecraft:visual/star_angle",
        "minecraft:visual/star_brightness",
    ] {
        assert_eq!(float(&attributes, id, &ctx), 0.0, "{id} must be inert in the nether");
    }
    assert_eq!(
        color(&attributes, "minecraft:visual/cloud_color", &ctx) >> 24,
        0,
        "a fully transparent cloud colour is what removes the cloud draw"
    );
}

#[test]
fn a_composed_value_is_clamped_back_into_its_range() {
    let (attributes, _timelines) = overworld();
    let empty = SpatialAttributeInterpolator::default();
    let ticks = ticks_at(&attributes, MIDNIGHT, 0.0);
    let ctx = context(&ticks, &empty, Weather { rain: 1.0, thunder: 1.0 });

    let factor = float(&attributes, "minecraft:visual/sky_light_factor", &ctx);
    assert!((0.0..=1.0).contains(&factor), "sky_light_factor is a unit float, got {factor}");

    let level = float(&attributes, "minecraft:gameplay/sky_light_level", &ctx);
    assert!((0.0..=15.0).contains(&level), "sky_light_level is [0; 15], got {level}");
}
