use std::marker::PhantomData;

use mcrs_minecraft_core::StaticResourceLocation;
use mcrs_minecraft_core::codec::HexRgb;

use super::MobSpawnSettings;

/// An attribute id together with the type an `override` of it takes.
pub struct Attribute<T> {
    pub id: StaticResourceLocation,
    value: PhantomData<fn() -> T>,
}

impl<T> Clone for Attribute<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Attribute<T> {}

/// The value of an attribute that built-in descriptions write as their own
/// serializable shape. It has no instance, so such an attribute is written
/// through its argument.
pub enum Opaque {}

macro_rules! attributes {
    ($($name:ident: $value:ty = $id:expr;)*) => {
        $(pub const $name: Attribute<$value> = Attribute {
            id: $id.location(),
            value: PhantomData,
        };)*
    };
}

attributes! {
    SKY_COLOR: HexRgb = crate::keys::EnvironmentAttribute::VisualSkyColor;
    FOG_COLOR: HexRgb = crate::keys::EnvironmentAttribute::VisualFogColor;
    WATER_FOG_COLOR: HexRgb = crate::keys::EnvironmentAttribute::VisualWaterFogColor;
    WATER_FOG_END_DISTANCE: f32 = crate::keys::EnvironmentAttribute::VisualWaterFogEndDistance;
    AMBIENT_PARTICLES: Opaque = crate::keys::EnvironmentAttribute::VisualAmbientParticles;
    BACKGROUND_MUSIC: Opaque = crate::keys::EnvironmentAttribute::AudioBackgroundMusic;
    MUSIC_VOLUME: f32 = crate::keys::EnvironmentAttribute::AudioMusicVolume;
    AMBIENT_SOUNDS: Opaque = crate::keys::EnvironmentAttribute::AudioAmbientSounds;
    INCREASED_FIRE_BURNOUT: bool = crate::keys::EnvironmentAttribute::GameplayIncreasedFireBurnout;
    SNOW_GOLEM_MELTS: bool = crate::keys::EnvironmentAttribute::GameplaySnowGolemMelts;
    CAN_PILLAGER_PATROL_SPAWN: bool = crate::keys::EnvironmentAttribute::GameplayCanPillagerPatrolSpawn;
    CREATURE_WORLD_GEN_SPAWN_PROBABILITY: f32 =
        crate::keys::EnvironmentAttribute::GameplayCreatureWorldGenSpawnProbability;
    NATURAL_MOB_SPAWNS: MobSpawnSettings = crate::keys::EnvironmentAttribute::GameplayNaturalMobSpawns;
}
