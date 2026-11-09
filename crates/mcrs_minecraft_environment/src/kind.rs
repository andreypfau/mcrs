use crate::environment_attribute_keys::EnvironmentAttribute;

/// The kind of value an attribute holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttributeType {
    Boolean,
    TriState,
    Float,
    AngleDegrees,
    Integer,
    RgbColor,
    ArgbColor,
    MoonPhase,
    Activity,
    BedRule,
    Particle,
    BackgroundMusic,
    AmbientSounds,
    MobSpawnSettings,
    AmbientParticles,
}

/// The closed interval a float attribute keeps after the last layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min: f32,
    pub max: f32,
}

impl Bounds {
    const UNIT: Bounds = Bounds { min: 0.0, max: 1.0 };
    const NON_NEGATIVE: Bounds = Bounds {
        min: 0.0,
        max: f32::INFINITY,
    };

    #[must_use]
    pub fn contains(self, value: f32) -> bool {
        self.min <= value && value <= self.max
    }

    /// The nearer bound for a value outside; a NaN stays NaN.
    #[must_use]
    pub fn clamp(self, value: f32) -> f32 {
        if value < self.min {
            self.min
        } else if value > self.max {
            self.max
        } else {
            value
        }
    }
}

/// What the attribute table states about one attribute.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttributeInfo {
    pub kind: AttributeType,
    pub range: Option<Bounds>,
    pub syncable: bool,
    pub positional: bool,
    pub spatially_interpolated: bool,
}

const fn row(
    kind: AttributeType,
    range: Option<Bounds>,
    syncable: bool,
    positional: bool,
    spatially_interpolated: bool,
) -> AttributeInfo {
    AttributeInfo {
        kind,
        range,
        syncable,
        positional,
        spatially_interpolated,
    }
}

/// The row of `attribute` in the attribute table.
#[must_use]
pub const fn info(attribute: EnvironmentAttribute) -> AttributeInfo {
    use AttributeType as T;
    use EnvironmentAttribute::*;
    const ANY: Option<Bounds> = None;
    const UNIT: Option<Bounds> = Some(Bounds::UNIT);
    const NON_NEGATIVE: Option<Bounds> = Some(Bounds::NON_NEGATIVE);
    match attribute {
        VisualFogColor
        | VisualWaterFogColor
        | VisualSkyColor
        | VisualBlockLightTint
        | VisualSkyLightColor
        | VisualNightVisionColor
        | VisualAmbientLightColor => row(T::RgbColor, ANY, true, true, true),
        VisualSunriseSunsetColor | VisualCloudColor => row(T::ArgbColor, ANY, true, true, true),
        VisualFogStartDistance | VisualWaterFogStartDistance | VisualCloudHeight => {
            row(T::Float, ANY, true, true, true)
        }
        VisualFogEndDistance
        | VisualSkyFogEndDistance
        | VisualCloudFogEndDistance
        | VisualWaterFogEndDistance => row(T::Float, NON_NEGATIVE, true, true, true),
        VisualSunAngle | VisualMoonAngle | VisualStarAngle => {
            row(T::AngleDegrees, ANY, true, true, true)
        }
        VisualMoonPhase => row(T::MoonPhase, ANY, true, true, false),
        VisualStarBrightness | VisualSkyLightFactor => row(T::Float, UNIT, true, true, true),
        VisualHasSkyOccluder
        | AudioFireflyBushSounds
        | GameplayWaterEvaporates
        | GameplayPiglinsZombify
        | GameplayCreakingActive => row(T::Boolean, ANY, true, true, false),
        VisualDefaultDripstoneParticle => row(T::Particle, ANY, true, true, false),
        VisualAmbientParticles => row(T::AmbientParticles, ANY, true, true, false),
        AudioBackgroundMusic => row(T::BackgroundMusic, ANY, true, true, false),
        AudioMusicVolume => row(T::Float, UNIT, true, true, false),
        AudioAmbientSounds => row(T::AmbientSounds, ANY, true, true, false),
        GameplaySkyLightLevel => row(
            T::Float,
            Some(Bounds {
                min: 0.0,
                max: 15.0,
            }),
            true,
            false,
            false,
        ),
        GameplayFastLava => row(T::Boolean, ANY, true, false, false),
        GameplayCanStartRaid
        | GameplayRespawnAnchorWorks
        | GameplayNetherPortalSpawnsPiglin
        | GameplayIncreasedFireBurnout
        | GameplaySnowGolemMelts
        | GameplayBeesStayInHive
        | GameplayMonstersBurn
        | GameplayCanPillagerPatrolSpawn => row(T::Boolean, ANY, false, true, false),
        GameplayBedRule | GameplayStrawBedRule => row(T::BedRule, ANY, false, true, false),
        GameplayEyeblossomOpen => row(T::TriState, ANY, false, true, false),
        GameplayTurtleEggHatchChance
        | GameplaySurfaceSlimeSpawnChance
        | GameplayCatWakingUpGiftChance => row(T::Float, UNIT, false, true, false),
        GameplayNaturalMobSpawns => row(T::MobSpawnSettings, ANY, false, true, false),
        GameplayCreatureWorldGenSpawnProbability => row(
            T::Float,
            Some(Bounds {
                min: 0.0,
                max: 0.9999999,
            }),
            false,
            true,
            false,
        ),
        GameplayVillagerActivity | GameplayBabyVillagerActivity => {
            row(T::Activity, ANY, false, true, false)
        }
    }
}

/// Whether the attribute that `id` names is sent to clients. An unknown id is not.
#[must_use]
pub fn is_syncable(id: &str) -> bool {
    EnvironmentAttribute::read(id).is_some_and(|attribute| info(attribute).syncable)
}
