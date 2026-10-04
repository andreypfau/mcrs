use std::marker::PhantomData;

use mcrs_minecraft_core::codec::HexRgb;
use mcrs_minecraft_core::{StaticResourceLocation, rl};

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
    ($($name:ident: $value:ty = $id:literal;)*) => {
        $(pub const $name: Attribute<$value> = Attribute {
            id: rl!($id),
            value: PhantomData,
        };)*

        #[cfg(test)]
        const ALL: &[StaticResourceLocation] = &[$($name.id),*];
    };
}

attributes! {
    SKY_COLOR: HexRgb = "minecraft:visual/sky_color";
    FOG_COLOR: HexRgb = "minecraft:visual/fog_color";
    WATER_FOG_COLOR: HexRgb = "minecraft:visual/water_fog_color";
    WATER_FOG_END_DISTANCE: f32 = "minecraft:visual/water_fog_end_distance";
    AMBIENT_PARTICLES: Opaque = "minecraft:visual/ambient_particles";
    BACKGROUND_MUSIC: Opaque = "minecraft:audio/background_music";
    MUSIC_VOLUME: f32 = "minecraft:audio/music_volume";
    AMBIENT_SOUNDS: Opaque = "minecraft:audio/ambient_sounds";
    INCREASED_FIRE_BURNOUT: bool = "minecraft:gameplay/increased_fire_burnout";
    SNOW_GOLEM_MELTS: bool = "minecraft:gameplay/snow_golem_melts";
    CAN_PILLAGER_PATROL_SPAWN: bool = "minecraft:gameplay/can_pillager_patrol_spawn";
    CREATURE_WORLD_GEN_SPAWN_PROBABILITY: f32 =
        "minecraft:gameplay/creature_world_gen_spawn_probability";
    NATURAL_MOB_SPAWNS: MobSpawnSettings = "minecraft:gameplay/natural_mob_spawns";
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
