//! The two blocks the sky renderer consumes: what is baked into the shader for
//! as long as the dimension lasts, and what is written to one uniform per
//! frame.
//!
//! Which visual attribute lands in which block is derived, never listed: an
//! attribute is dimension-constant exactly when nothing below the biome layer
//! can move it, so a datapack that gives `cloud_height` a track moves it into
//! the per-frame block without a line changing here.

use bevy::prelude::*;
use bitflags::bitflags;
use serde_json::Value;

use mcrs_vanilla::attribute::AttributeValue;
use mcrs_vanilla::dimension::dimension_type::Skybox;
use mcrs_vanilla::environment::{EnvironmentAttributes, EnvironmentContext};

/// A visual attribute the renderer carries as GPU state.
///
/// The two attributes left out — `ambient_particles` and
/// `default_dripstone_particle` — are spawn tables, not shader inputs; they
/// ride along in [`SkyStatic`] instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkyField {
    SkyColor,
    FogColor,
    CloudColor,
    SkyLightColor,
    SkyLightFactor,
    SunriseSunsetColor,
    StarBrightness,
    SunAngle,
    MoonAngle,
    StarAngle,
    MoonPhase,
    AmbientLightColor,
    BlockLightTint,
    NightVisionColor,
    WaterFogColor,
    CloudFogEndDistance,
    CloudHeight,
    FogEndDistance,
    FogStartDistance,
    SkyFogEndDistance,
    WaterFogEndDistance,
    WaterFogStartDistance,
}

impl SkyField {
    pub const ALL: [SkyField; 22] = [
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
        SkyField::AmbientLightColor,
        SkyField::BlockLightTint,
        SkyField::NightVisionColor,
        SkyField::WaterFogColor,
        SkyField::CloudFogEndDistance,
        SkyField::CloudHeight,
        SkyField::FogEndDistance,
        SkyField::FogStartDistance,
        SkyField::SkyFogEndDistance,
        SkyField::WaterFogEndDistance,
        SkyField::WaterFogStartDistance,
    ];

    pub fn attribute(self) -> &'static str {
        match self {
            SkyField::SkyColor => "minecraft:visual/sky_color",
            SkyField::FogColor => "minecraft:visual/fog_color",
            SkyField::CloudColor => "minecraft:visual/cloud_color",
            SkyField::SkyLightColor => "minecraft:visual/sky_light_color",
            SkyField::SkyLightFactor => "minecraft:visual/sky_light_factor",
            SkyField::SunriseSunsetColor => "minecraft:visual/sunrise_sunset_color",
            SkyField::StarBrightness => "minecraft:visual/star_brightness",
            SkyField::SunAngle => "minecraft:visual/sun_angle",
            SkyField::MoonAngle => "minecraft:visual/moon_angle",
            SkyField::StarAngle => "minecraft:visual/star_angle",
            SkyField::MoonPhase => "minecraft:visual/moon_phase",
            SkyField::AmbientLightColor => "minecraft:visual/ambient_light_color",
            SkyField::BlockLightTint => "minecraft:visual/block_light_tint",
            SkyField::NightVisionColor => "minecraft:visual/night_vision_color",
            SkyField::WaterFogColor => "minecraft:visual/water_fog_color",
            SkyField::CloudFogEndDistance => "minecraft:visual/cloud_fog_end_distance",
            SkyField::CloudHeight => "minecraft:visual/cloud_height",
            SkyField::FogEndDistance => "minecraft:visual/fog_end_distance",
            SkyField::FogStartDistance => "minecraft:visual/fog_start_distance",
            SkyField::SkyFogEndDistance => "minecraft:visual/sky_fog_end_distance",
            SkyField::WaterFogEndDistance => "minecraft:visual/water_fog_end_distance",
            SkyField::WaterFogStartDistance => "minecraft:visual/water_fog_start_distance",
        }
    }
}

/// One field's value: a scalar, or a packed `0xAARRGGBB` colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SkyValue {
    Scalar(f32),
    Color(u32),
}

impl SkyValue {
    pub fn color(self) -> u32 {
        match self {
            SkyValue::Color(packed) => packed,
            SkyValue::Scalar(value) => value as u32,
        }
    }
}

const MOON_PHASES: [&str; 8] = [
    "full_moon",
    "waning_gibbous",
    "third_quarter",
    "waning_crescent",
    "new_moon",
    "waxing_crescent",
    "first_quarter",
    "waxing_gibbous",
];

fn sky_value(value: &AttributeValue) -> SkyValue {
    match value {
        AttributeValue::Float(v) => SkyValue::Scalar(*v),
        AttributeValue::Color(packed) => SkyValue::Color(*packed),
        AttributeValue::Integer(v) => SkyValue::Scalar(*v as f32),
        AttributeValue::Opaque(Value::String(name)) => SkyValue::Scalar(
            MOON_PHASES
                .iter()
                .position(|phase| phase == name)
                .unwrap_or(0) as f32,
        ),
        _ => SkyValue::Scalar(0.0),
    }
}

bitflags! {
    /// Which of the sky draws exist at all. A draw whose effect is off is not
    /// skipped at runtime — its pipeline is never built.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct SkyEffects: u8 {
        const DISC = 1;
        const TWILIGHT = 1 << 1;
        const CELESTIAL = 1 << 2;
        const STARS = 1 << 3;
        const CLOUDS = 1 << 4;
    }
}

/// The pipeline specialization key: everything that decides which shaders get
/// compiled for this dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SkyKey {
    pub skybox: Skybox,
    pub effects: SkyEffects,
}

impl SkyKey {
    pub fn draws(self) -> u32 {
        self.effects.bits().count_ones()
    }
}

/// Where each GPU-facing visual attribute lives, and which stack produces it.
///
/// Built once per dimension. The stack indices are what keep a frame from
/// looking an attribute up by id.
#[derive(Debug, Clone)]
pub struct SkyLayout {
    frame: Vec<(SkyField, usize)>,
    constant: Vec<(SkyField, usize)>,
    ambient_particles: usize,
    dripstone_particle: usize,
}

impl SkyLayout {
    pub fn derive(attributes: &EnvironmentAttributes) -> Self {
        let index = |id: &str| {
            EnvironmentAttributes::index(id).expect("every sky field names a registered attribute")
        };
        let (frame, constant) = SkyField::ALL
            .into_iter()
            .map(|field| (field, index(field.attribute())))
            .partition(|(_, stack)| attributes.stack(*stack).is_dynamic());

        SkyLayout {
            frame,
            constant,
            ambient_particles: index("minecraft:visual/ambient_particles"),
            dripstone_particle: index("minecraft:visual/default_dripstone_particle"),
        }
    }

    /// The meaning of each slot of [`SkyFrame::values`], in order. Only the
    /// split between the two blocks is asserted on; a frame reads by index.
    #[cfg(test)]
    pub fn frame_fields(&self) -> impl Iterator<Item = SkyField> + '_ {
        self.frame.iter().map(|(field, _)| *field)
    }

    #[cfg(test)]
    pub fn constant_fields(&self) -> impl Iterator<Item = SkyField> + '_ {
        self.constant.iter().map(|(field, _)| *field)
    }

    /// The one evaluation pass a frame runs. Nothing it produces is kept.
    pub fn evaluate(
        &self,
        attributes: &EnvironmentAttributes,
        ctx: &EnvironmentContext,
        frame: &mut SkyFrame,
    ) {
        frame.camera = ctx.position.as_vec3().to_array();
        frame.rain = ctx.weather.rain;
        frame.thunder = ctx.weather.thunder;
        frame.values.clear();
        frame.values.extend(
            self.frame
                .iter()
                .map(|(_, stack)| sky_value(&attributes.stack(*stack).evaluate(ctx))),
        );
    }

    /// The block baked into the shader, rebuilt only when the dimension — or
    /// the biome weights the positional layer reads — changes.
    pub fn constants(
        &self,
        attributes: &EnvironmentAttributes,
        ctx: &EnvironmentContext,
    ) -> SkyStatic {
        let skybox = attributes.skybox();
        let values: Vec<(SkyField, SkyValue)> = self
            .constant
            .iter()
            .map(|(field, stack)| (*field, sky_value(&attributes.stack(*stack).evaluate(ctx))))
            .collect();

        let list = |stack: usize| match attributes.stack(stack).evaluate(ctx) {
            AttributeValue::List(items) => items,
            other => vec![opaque(other)],
        };

        SkyStatic {
            key: SkyKey {
                skybox,
                effects: effects(skybox, attributes, ctx),
            },
            values,
            ambient_particles: list(self.ambient_particles),
            dripstone_particle: opaque(attributes.stack(self.dripstone_particle).evaluate(ctx)),
        }
    }
}

fn opaque(value: AttributeValue) -> Value {
    match value {
        AttributeValue::Opaque(value) => value,
        AttributeValue::List(items) => Value::Array(items),
        _ => Value::Null,
    }
}

/// A draw exists when this dimension can make it visible at all.
///
/// The sun, the moon, its twilight and the stars belong to the overworld
/// skybox; clouds need a cloud colour that is not fully transparent, which is
/// what leaves the Nether and the End without them.
fn effects(
    skybox: Skybox,
    attributes: &EnvironmentAttributes,
    ctx: &EnvironmentContext,
) -> SkyEffects {
    let mut effects = SkyEffects::DISC;
    if skybox == Skybox::Overworld {
        effects |= SkyEffects::TWILIGHT | SkyEffects::CELESTIAL | SkyEffects::STARS;
        let cloud_color = EnvironmentAttributes::index(SkyField::CloudColor.attribute())
            .map(|stack| sky_value(&attributes.stack(stack).evaluate(ctx)).color())
            .unwrap_or(0);
        if cloud_color >> 24 != 0 {
            effects |= SkyEffects::CLOUDS;
        }
    }
    effects
}

/// The per-frame uniform: one write, no allocation, no memoized value.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct SkyFrame {
    pub camera: [f32; 3],
    pub rain: f32,
    pub thunder: f32,
    pub values: Vec<SkyValue>,
}

impl SkyFrame {
    pub fn get(&self, layout: &SkyLayout, field: SkyField) -> Option<SkyValue> {
        let slot = layout
            .frame
            .iter()
            .position(|(candidate, _)| *candidate == field)?;
        self.values.get(slot).copied()
    }
}

/// Everything constant for as long as the player stays in one dimension.
#[derive(Debug, Clone, PartialEq)]
pub struct SkyStatic {
    pub key: SkyKey,
    pub values: Vec<(SkyField, SkyValue)>,
    pub ambient_particles: Vec<Value>,
    pub dripstone_particle: Value,
}

impl SkyStatic {
    pub fn get(&self, field: SkyField) -> Option<SkyValue> {
        self.values
            .iter()
            .find(|(candidate, _)| *candidate == field)
            .map(|(_, value)| *value)
    }
}

#[cfg(test)]
mod tests {
    use bevy::math::DVec3;
    use serde_json::json;

    use mcrs_vanilla::attribute::{AttributeValue, EnvironmentAttributeMap};
    use mcrs_vanilla::environment::{
        DimensionEnvironment, EnvironmentAttributes, EnvironmentContext,
        SpatialAttributeInterpolator, Weather,
    };
    use mcrs_vanilla::timeline::Timeline;
    use mcrs_vanilla::world_clock::{ClockState, WorldClocks};

    use super::*;

    const NOON: i64 = 6000;

    /// The dimension fields an environment needs, read straight from the asset.
    /// `ProtoDimensionType` is private to `mcrs_vanilla`, and widening its API for
    /// a test would be the wrong trade.
    #[derive(serde::Deserialize)]
    struct Dimension {
        has_skylight: bool,
        has_ceiling: bool,
        #[serde(default)]
        skybox: Skybox,
        #[serde(default)]
        attributes: EnvironmentAttributeMap,
    }

    fn dimension_type(name: &str) -> Dimension {
        let bytes = std::fs::read(
            crate::asset_corpus()
                .join("minecraft")
                .join("dimension_type")
                .join(name),
        )
        .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn timeline(name: &str) -> Timeline {
        let bytes = std::fs::read(
            crate::asset_corpus()
                .join("minecraft")
                .join("timeline")
                .join(name),
        )
        .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    /// The timelines a tag names, read from the shipped tag files and flattened
    /// through `#` references, in the order the tag file lists them.
    fn tagged_timelines(tag: &str) -> Vec<Timeline> {
        fn collect(tag: &str, out: &mut Vec<String>) {
            let path = crate::asset_corpus()
                .join("minecraft")
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

    fn shape<'a>(id: &'a str, proto: &'a Dimension) -> DimensionEnvironment<'a> {
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

    /// The tick each clock stands at, read the way a frame reads it.
    fn ticks_at(
        attributes: &EnvironmentAttributes,
        total_ticks: i64,
        partial_tick: f32,
    ) -> Vec<f64> {
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

    #[test]
    fn the_dimension_constant_set_comes_from_the_loaded_timelines() {
        let (attributes, _timelines) = overworld();
        let layout = SkyLayout::derive(&attributes);

        let frame: Vec<SkyField> = layout.frame_fields().collect();
        let constant: Vec<SkyField> = layout.constant_fields().collect();
        assert_eq!(frame.len() + constant.len(), SkyField::ALL.len());
        assert_eq!(frame.len(), 11, "{frame:?}");
        assert_eq!(
            constant.len(),
            11,
            "the 13 constants less the two particle payloads"
        );

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
            assert!(
                frame.contains(&field),
                "{field:?} has a track and belongs in the frame block"
            );
        }
        assert!(constant.contains(&SkyField::CloudHeight));
    }

    #[test]
    fn a_track_for_cloud_height_moves_it_into_the_frame_block() {
        let mut timelines = tagged_timelines("in_overworld");
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
        assert!(
            !layout
                .constant_fields()
                .any(|field| field == SkyField::CloudHeight)
        );
        assert_eq!(frame.len(), 12);
    }

    #[test]
    fn one_pass_fills_the_frame_block_in_layout_order() {
        let (attributes, _timelines) = overworld();
        let layout = SkyLayout::derive(&attributes);
        let empty = SpatialAttributeInterpolator::default();
        let ticks = ticks_at(&attributes, NOON, 0.0);
        let weather = Weather {
            rain: 0.5,
            thunder: 0.25,
        };

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
        assert_eq!(
            frame.get(&layout, SkyField::SunAngle),
            Some(SkyValue::Scalar(0.0))
        );

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
        assert_eq!(
            constants.get(SkyField::CloudHeight),
            Some(SkyValue::Scalar(192.33))
        );
    }

    #[test]
    fn the_nether_has_no_sun_moon_stars_or_clouds() {
        let nether_timelines = [timeline("villager_schedule.json")];
        let borrowed: Vec<&Timeline> = nether_timelines.iter().collect();
        let proto = dimension_type("the_nether.json");
        let attributes =
            EnvironmentAttributes::build(&shape("minecraft:the_nether", &proto), &borrowed)
                .unwrap();

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
                .constants(
                    &overworld,
                    &context(&ticks_at(&overworld, NOON, 0.0), &empty, Weather::default()),
                )
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
            assert_eq!(
                float(&attributes, id, &ctx),
                0.0,
                "{id} must be inert in the nether"
            );
        }
        assert_eq!(
            color(&attributes, "minecraft:visual/cloud_color", &ctx) >> 24,
            0,
            "a fully transparent cloud colour is what removes the cloud draw"
        );
    }
}
