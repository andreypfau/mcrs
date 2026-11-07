use mcrs_minecraft_item::Text;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_particle::ParticleOptions;
use mcrs_minecraft_registries::Named;
use serde::de::DeserializeOwned;
use serde::{Serialize, Serializer};

use super::color;
use super::kind::{AttributeType, info};
use super::mob_spawns::MobSpawnSettings;
use super::value::{
    AmbientParticle, AmbientSounds, BackgroundMusic, BedRule, BedRuleCondition, MoonPhase, TriState,
};
use crate::activity_keys::Activity;
use crate::environment_attribute_keys::EnvironmentAttribute;

/// A value of one attribute. `Float` holds both `float` and `angle_degrees`.
#[derive(Debug, Clone, PartialEq)]
pub enum AttributeValue {
    Boolean(bool),
    TriState(TriState),
    Float(f32),
    Integer(i32),
    Rgb([f32; 3]),
    Argb([f32; 4]),
    MoonPhase(MoonPhase),
    Activity(Activity),
    BedRule(BedRule),
    Particle(ParticleOptions<Named>),
    BackgroundMusic(BackgroundMusic),
    AmbientSounds(AmbientSounds),
    MobSpawnSettings(MobSpawnSettings),
    AmbientParticles(Vec<AmbientParticle>),
}

/// The four places a value is interpolated, each with its own function per type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interpolation {
    Keyframe,
    Weather,
    Spatial,
    PartialTick,
}

fn bed_rule(can_set_spawn: BedRuleCondition, destroy_on_leave: bool) -> BedRule {
    BedRule {
        can_sleep: BedRuleCondition::WhenDark,
        can_set_spawn,
        destroy_on_use: false,
        destroy_on_leave,
        error_message: Some(Text::translate("block.minecraft.bed.no_sleep", Vec::new())),
    }
}

/// The value of `attribute` before any layer.
#[must_use]
pub fn default_value(attribute: EnvironmentAttribute) -> AttributeValue {
    use AttributeValue as V;
    use EnvironmentAttribute::*;
    match attribute {
        VisualFogColor | VisualSkyColor | VisualAmbientLightColor => V::Rgb(color::rgb(0x000000)),
        VisualWaterFogColor => V::Rgb(color::rgb(0x050533)),
        VisualBlockLightTint => V::Rgb(color::rgb(0xFFD88C)),
        VisualSkyLightColor => V::Rgb(color::rgb(0xFFFFFF)),
        VisualNightVisionColor => V::Rgb(color::rgb(0x999999)),
        VisualSunriseSunsetColor | VisualCloudColor => V::Argb(color::argb(0x0000_0000)),
        VisualFogStartDistance
        | VisualSunAngle
        | VisualMoonAngle
        | VisualStarAngle
        | VisualStarBrightness
        | GameplaySurfaceSlimeSpawnChance
        | GameplayCatWakingUpGiftChance => V::Float(0.0),
        VisualFogEndDistance => V::Float(1024.0),
        VisualSkyFogEndDistance => V::Float(512.0),
        VisualCloudFogEndDistance => V::Float(2048.0),
        VisualWaterFogStartDistance => V::Float(-8.0),
        VisualWaterFogEndDistance => V::Float(96.0),
        VisualCloudHeight => V::Float(192.33),
        VisualSkyLightFactor | AudioMusicVolume => V::Float(1.0),
        GameplaySkyLightLevel => V::Float(15.0),
        GameplayTurtleEggHatchChance => V::Float(0.002),
        GameplayCreatureWorldGenSpawnProbability => V::Float(0.1),
        VisualMoonPhase => V::MoonPhase(MoonPhase::FullMoon),
        VisualHasSkyOccluder
        | AudioFireflyBushSounds
        | GameplayWaterEvaporates
        | GameplayRespawnAnchorWorks
        | GameplayNetherPortalSpawnsPiglin
        | GameplayFastLava
        | GameplayIncreasedFireBurnout
        | GameplaySnowGolemMelts
        | GameplayCreakingActive
        | GameplayBeesStayInHive
        | GameplayMonstersBurn => V::Boolean(false),
        GameplayCanStartRaid | GameplayPiglinsZombify | GameplayCanPillagerPatrolSpawn => {
            V::Boolean(true)
        }
        VisualDefaultDripstoneParticle => V::Particle(ParticleOptions::DrippingDripstoneWater),
        VisualAmbientParticles => V::AmbientParticles(Vec::new()),
        AudioBackgroundMusic => V::BackgroundMusic(BackgroundMusic::default()),
        AudioAmbientSounds => V::AmbientSounds(AmbientSounds::default()),
        GameplayBedRule => V::BedRule(bed_rule(BedRuleCondition::Always, false)),
        GameplayStrawBedRule => V::BedRule(bed_rule(BedRuleCondition::Never, true)),
        GameplayEyeblossomOpen => V::TriState(TriState::Default),
        GameplayNaturalMobSpawns => V::MobSpawnSettings(MobSpawnSettings::default()),
        GameplayVillagerActivity | GameplayBabyVillagerActivity => V::Activity(Activity::Idle),
    }
}

fn decode<T: DeserializeOwned>(tag: &NbtTag) -> Result<T, String> {
    mcrs_minecraft_nbt::from_tag(tag.clone()).map_err(|error| error.to_string())
}

fn boolean(tag: &NbtTag) -> Result<bool, String> {
    match tag {
        NbtTag::Byte(0) => Ok(false),
        NbtTag::Byte(1) => Ok(true),
        _ => Err("expected a boolean".to_owned()),
    }
}

pub(crate) fn read_float(tag: &NbtTag) -> Result<f32, String> {
    color::float(tag).ok_or_else(|| "expected a number".to_owned())
}

fn read_integer(tag: &NbtTag) -> Result<i32, String> {
    match tag {
        NbtTag::Byte(value) => Ok(i32::from(*value)),
        NbtTag::Short(value) => Ok(i32::from(*value)),
        NbtTag::Int(value) => Ok(*value),
        _ => Err("expected an int".to_owned()),
    }
}

fn tri_state(tag: &NbtTag) -> Result<TriState, String> {
    match tag {
        NbtTag::String(text) if text == "default" => Ok(TriState::Default),
        NbtTag::String(text) if text == "true" => Ok(TriState::True),
        NbtTag::String(text) if text == "false" => Ok(TriState::False),
        other => boolean(other)
            .map(|value| {
                if value {
                    TriState::True
                } else {
                    TriState::False
                }
            })
            .map_err(|_| "expected a boolean or `default`".to_owned()),
    }
}

impl AttributeValue {
    /// Reads the value form of `kind`. A float is not checked against a range here.
    pub(crate) fn read(kind: AttributeType, tag: &NbtTag) -> Result<AttributeValue, String> {
        use AttributeType as T;
        Ok(match kind {
            T::Boolean => AttributeValue::Boolean(boolean(tag)?),
            T::TriState => AttributeValue::TriState(tri_state(tag)?),
            T::Float | T::AngleDegrees => AttributeValue::Float(read_float(tag)?),
            T::Integer => AttributeValue::Integer(read_integer(tag)?),
            T::RgbColor => AttributeValue::Rgb(color::read_rgb(tag)?),
            T::ArgbColor => AttributeValue::Argb(color::read_argb(tag)?),
            T::MoonPhase => AttributeValue::MoonPhase(decode(tag)?),
            T::Activity => AttributeValue::Activity(decode(tag)?),
            T::BedRule => AttributeValue::BedRule(decode(tag)?),
            T::Particle => AttributeValue::Particle(decode(tag)?),
            T::BackgroundMusic => AttributeValue::BackgroundMusic(decode(tag)?),
            T::AmbientSounds => AttributeValue::AmbientSounds(decode(tag)?),
            T::MobSpawnSettings => AttributeValue::MobSpawnSettings(decode(tag)?),
            T::AmbientParticles => AttributeValue::AmbientParticles(decode(tag)?),
        })
    }

    /// A colour as `0xAARRGGBB`, each byte as a colour writes it; an RGB colour is opaque.
    #[must_use]
    pub fn packed_argb(&self) -> Option<u32> {
        let pack = |[red, green, blue, alpha]: [f32; 4]| {
            u32::from_be_bytes([
                color::byte(alpha),
                color::byte(red),
                color::byte(green),
                color::byte(blue),
            ])
        };
        match self {
            AttributeValue::Rgb([red, green, blue]) => Some(pack([*red, *green, *blue, 1.0])),
            AttributeValue::Argb(channels) => Some(pack(*channels)),
            _ => None,
        }
    }

    /// The value after the last layer: a float outside the range of `attribute` becomes the
    /// nearer bound.
    #[must_use]
    pub fn sanitized(self, attribute: EnvironmentAttribute) -> AttributeValue {
        match (self, info(attribute).range) {
            (AttributeValue::Float(value), Some(range)) => {
                AttributeValue::Float(range.clamp(value))
            }
            (value, _) => value,
        }
    }

    /// The interpolation of `kind` for `place`, from `from` at 0 to `to` at 1.
    #[must_use]
    pub fn interpolate(
        kind: AttributeType,
        place: Interpolation,
        t: f32,
        from: &AttributeValue,
        to: &AttributeValue,
    ) -> AttributeValue {
        use AttributeValue as V;
        match (from, to) {
            (V::Float(a), V::Float(b)) => match (kind, place) {
                (AttributeType::AngleDegrees, Interpolation::PartialTick) => {
                    V::Float(degrees(t, *a, *b))
                }
                _ => V::Float(lerp(t, *a, *b)),
            },
            (V::Integer(a), V::Integer(b)) => V::Integer(integer(t, *a, *b)),
            (V::Rgb(a), V::Rgb(b)) => V::Rgb(channels(t, *a, *b)),
            (V::Argb(a), V::Argb(b)) => V::Argb(channels(t, *a, *b)),
            (V::AmbientParticles(a), V::AmbientParticles(b)) => cross_fade(t, a, b),
            _ => {
                let threshold = match place {
                    Interpolation::Keyframe => 1.0,
                    Interpolation::Spatial => 0.5,
                    Interpolation::Weather | Interpolation::PartialTick => 0.0,
                };
                if t >= threshold {
                    to.clone()
                } else {
                    from.clone()
                }
            }
        }
    }
}

#[must_use]
pub(crate) fn lerp(t: f32, a: f32, b: f32) -> f32 {
    a + t * (b - a)
}

pub(crate) fn channels<const N: usize>(t: f32, a: [f32; N], b: [f32; N]) -> [f32; N] {
    let mut out = a;
    for (slot, target) in out.iter_mut().zip(b) {
        *slot = lerp(t, *slot, target);
    }
    out
}

pub(crate) fn integer(t: f32, a: i32, b: i32) -> i32 {
    let delta = b.wrapping_sub(a);
    a.wrapping_add((t * delta as f32).floor() as i32)
}

fn wrap_degrees(x: f32) -> f32 {
    let mut rest = x % 360.0;
    if rest >= 180.0 {
        rest -= 360.0;
    }
    if rest < -180.0 {
        rest += 360.0;
    }
    rest
}

fn degrees(t: f32, a: f32, b: f32) -> f32 {
    let delta = wrap_degrees(b - a);
    if delta.abs() >= 90.0 {
        b
    } else {
        a + t * delta
    }
}

fn cross_fade(t: f32, a: &[AmbientParticle], b: &[AmbientParticle]) -> AttributeValue {
    let faded = if t == 0.0 {
        a.to_vec()
    } else if t == 1.0 {
        b.to_vec()
    } else {
        let scaled = |particles: &[AmbientParticle], factor: f32| {
            particles
                .iter()
                .map(|particle| AmbientParticle {
                    particle: particle.particle.clone(),
                    probability: particle.probability * factor,
                })
                .collect::<Vec<_>>()
        };
        let mut out = scaled(a, 1.0 - t);
        out.extend(scaled(b, t));
        out
    };
    AttributeValue::AmbientParticles(faded)
}

impl Serialize for AttributeValue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            AttributeValue::Boolean(value) => serializer.serialize_bool(*value),
            AttributeValue::TriState(value) => value.serialize(serializer),
            AttributeValue::Float(value) => serializer.serialize_f32(*value),
            AttributeValue::Integer(value) => serializer.serialize_i32(*value),
            AttributeValue::Rgb(value) => serializer.serialize_str(&color::rgb_hex(*value)),
            AttributeValue::Argb(value) => serializer.serialize_str(&color::argb_hex(*value)),
            AttributeValue::MoonPhase(value) => value.serialize(serializer),
            AttributeValue::Activity(value) => value.serialize(serializer),
            AttributeValue::BedRule(value) => value.serialize(serializer),
            AttributeValue::Particle(value) => value.serialize(serializer),
            AttributeValue::BackgroundMusic(value) => value.serialize(serializer),
            AttributeValue::AmbientSounds(value) => value.serialize(serializer),
            AttributeValue::MobSpawnSettings(value) => value.serialize(serializer),
            AttributeValue::AmbientParticles(value) => value.serialize(serializer),
        }
    }
}
