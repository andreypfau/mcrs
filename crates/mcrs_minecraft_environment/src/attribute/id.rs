use mcrs_minecraft_keys as keys;
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

        #[cfg(test)]
        const ALL: &[StaticResourceLocation] = &[$($name.id),*];
    };
}

attributes! {
    SKY_COLOR: HexRgb = keys::environment_attribute::VISUAL_SKY_COLOR;
    FOG_COLOR: HexRgb = keys::environment_attribute::VISUAL_FOG_COLOR;
    WATER_FOG_COLOR: HexRgb = keys::environment_attribute::VISUAL_WATER_FOG_COLOR;
    WATER_FOG_END_DISTANCE: f32 = keys::environment_attribute::VISUAL_WATER_FOG_END_DISTANCE;
    AMBIENT_PARTICLES: Opaque = keys::environment_attribute::VISUAL_AMBIENT_PARTICLES;
    BACKGROUND_MUSIC: Opaque = keys::environment_attribute::AUDIO_BACKGROUND_MUSIC;
    MUSIC_VOLUME: f32 = keys::environment_attribute::AUDIO_MUSIC_VOLUME;
    AMBIENT_SOUNDS: Opaque = keys::environment_attribute::AUDIO_AMBIENT_SOUNDS;
    INCREASED_FIRE_BURNOUT: bool = keys::environment_attribute::GAMEPLAY_INCREASED_FIRE_BURNOUT;
    SNOW_GOLEM_MELTS: bool = keys::environment_attribute::GAMEPLAY_SNOW_GOLEM_MELTS;
    CAN_PILLAGER_PATROL_SPAWN: bool = keys::environment_attribute::GAMEPLAY_CAN_PILLAGER_PATROL_SPAWN;
    CREATURE_WORLD_GEN_SPAWN_PROBABILITY: f32 =
        keys::environment_attribute::GAMEPLAY_CREATURE_WORLD_GEN_SPAWN_PROBABILITY;
    NATURAL_MOB_SPAWNS: MobSpawnSettings = keys::environment_attribute::GAMEPLAY_NATURAL_MOB_SPAWNS;
}

#[cfg(test)]
mod tests {
    use super::ALL;
    use crate::attribute::attribute;

    #[test]
    fn every_typed_id_names_a_registered_attribute() {
        for id in ALL {
            assert!(attribute(id.as_str()).is_some(), "{id}");
        }
    }
}
