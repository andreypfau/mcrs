//! The two blocks the sky renderer consumes: what is baked into the shader for
//! as long as the dimension lasts, and what is written to one uniform per
//! frame.
//!
//! Which visual attribute lands in which block is derived, never listed: an
//! attribute is dimension-constant exactly when nothing below the biome layer
//! can move it, so a datapack that gives `cloud_height` a track moves it into
//! the per-frame block without a line changing here.

use bevy_ecs::prelude::*;
use bitflags::bitflags;
use serde_json::Value;

use super::{EnvironmentAttributes, EnvironmentContext};
use crate::attribute::AttributeValue;
use crate::dimension::dimension_type::Skybox;

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
            MOON_PHASES.iter().position(|phase| phase == name).unwrap_or(0) as f32,
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

    /// The meaning of each slot of [`SkyFrame::values`], in order.
    pub fn frame_fields(&self) -> impl Iterator<Item = SkyField> + '_ {
        self.frame.iter().map(|(field, _)| *field)
    }

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
            key: SkyKey { skybox, effects: effects(skybox, attributes, ctx) },
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
        let slot = layout.frame.iter().position(|(candidate, _)| *candidate == field)?;
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
        self.values.iter().find(|(candidate, _)| *candidate == field).map(|(_, value)| *value)
    }
}
