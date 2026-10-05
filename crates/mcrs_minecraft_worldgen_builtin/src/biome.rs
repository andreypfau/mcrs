mod defaults;
mod end;
mod nether;
mod overworld;

use crate::keys::{Id, SoundKey, carver, placed, sound};
use mcrs_minecraft_biome::{Biome, BiomeGeneration as Generation, GrassColorModifier};
use mcrs_minecraft_core::codec::{HexRgb, NonNegativeInt};
use mcrs_minecraft_core::value_provider::IntProvider;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation, rl};
use mcrs_minecraft_environment::attribute::id::*;
use mcrs_minecraft_environment::attribute::{MobSpawnSettings, Operation};
use mcrs_minecraft_keys::EntityType;
use mcrs_minecraft_worldgen_structure::MobCategory;
use serde::Serialize;

#[derive(Clone, Copy)]
pub struct Mob {
    id: ResourceKey<EntityType, &'static str>,
    category: MobCategory,
}

pub mod mob {
    use super::Mob;
    use mcrs_minecraft_core::{ResourceKey, rl};
    use mcrs_minecraft_worldgen_structure::MobCategory::*;

    macro_rules! mobs {
        ($($name:ident = $id:literal, $category:ident;)*) => {
            $(pub const $name: Mob = Mob {
                id: ResourceKey::new(rl!($id)),
                category: $category,
            };)*

            #[cfg(test)]
            pub const ALL: &[Mob] = &[$($name),*];
        };
    }

    mobs! {
        ARMADILLO = "minecraft:armadillo", Creature;
        AXOLOTL = "minecraft:axolotl", Axolotls;
        BAT = "minecraft:bat", Ambient;
        BOGGED = "minecraft:bogged", Monster;
        CAMEL = "minecraft:camel", Creature;
        CAVE_SPIDER = "minecraft:cave_spider", Monster;
        CHICKEN = "minecraft:chicken", Creature;
        COD = "minecraft:cod", WaterAmbient;
        COW = "minecraft:cow", Creature;
        CREEPER = "minecraft:creeper", Monster;
        DOLPHIN = "minecraft:dolphin", WaterCreature;
        DONKEY = "minecraft:donkey", Creature;
        DROWNED = "minecraft:drowned", Monster;
        ENDERMAN = "minecraft:enderman", Monster;
        FOX = "minecraft:fox", Creature;
        FROG = "minecraft:frog", Creature;
        GHAST = "minecraft:ghast", Monster;
        GLOW_SQUID = "minecraft:glow_squid", UndergroundWaterCreature;
        GOAT = "minecraft:goat", Creature;
        HOGLIN = "minecraft:hoglin", Monster;
        HORSE = "minecraft:horse", Creature;
        HUSK = "minecraft:husk", Monster;
        LLAMA = "minecraft:llama", Creature;
        MAGMA_CUBE = "minecraft:magma_cube", Monster;
        MOOSHROOM = "minecraft:mooshroom", Creature;
        NAUTILUS = "minecraft:nautilus", WaterCreature;
        OCELOT = "minecraft:ocelot", Creature;
        PANDA = "minecraft:panda", Creature;
        PARCHED = "minecraft:parched", Monster;
        PARROT = "minecraft:parrot", Creature;
        PIG = "minecraft:pig", Creature;
        PIGLIN = "minecraft:piglin", Monster;
        POLAR_BEAR = "minecraft:polar_bear", Creature;
        PUFFERFISH = "minecraft:pufferfish", WaterAmbient;
        RABBIT = "minecraft:rabbit", Creature;
        SALMON = "minecraft:salmon", WaterAmbient;
        SHEEP = "minecraft:sheep", Creature;
        SKELETON = "minecraft:skeleton", Monster;
        SLIME = "minecraft:slime", Monster;
        SPIDER = "minecraft:spider", Monster;
        SQUID = "minecraft:squid", WaterCreature;
        STRAY = "minecraft:stray", Monster;
        STRIDER = "minecraft:strider", Creature;
        SULFUR_CUBE = "minecraft:sulfur_cube", Monster;
        TROPICAL_FISH = "minecraft:tropical_fish", WaterAmbient;
        TURTLE = "minecraft:turtle", Creature;
        WITCH = "minecraft:witch", Monster;
        WOLF = "minecraft:wolf", Creature;
        ZOMBIE = "minecraft:zombie", Monster;
        ZOMBIE_HORSE = "minecraft:zombie_horse", Monster;
        ZOMBIE_VILLAGER = "minecraft:zombie_villager", Monster;
        ZOMBIFIED_PIGLIN = "minecraft:zombified_piglin", Monster;
    }

    #[cfg(test)]
    mod tests {
        use super::ALL;
        use crate::keys::report;

        #[test]
        fn every_mob_is_an_entity_type_of_the_report() {
            let ids: Vec<_> = ALL.iter().map(|mob| mob.id).collect();
            assert_eq!(report::missing(&ids), Vec::<String>::new());
        }

        #[test]
        fn no_entity_type_is_listed_twice() {
            let ids: Vec<_> = ALL.iter().map(|mob| mob.id).collect();
            assert_eq!(report::repeated(&ids), Vec::<String>::new());
        }
    }
}

#[derive(Default)]
pub struct Mobs(MobSpawnSettings);

impl Mobs {
    pub fn none() -> Self {
        Mobs(MobSpawnSettings::no_spawns())
    }

    pub fn spawn(&mut self, mob: Mob, weight: i32, min: i32, max: i32) -> &mut Self {
        self.spawn_as(mob, mob.category, weight, min, max)
    }

    pub fn spawn_as(
        &mut self,
        mob: Mob,
        category: MobCategory,
        weight: i32,
        min: i32,
        max: i32,
    ) -> &mut Self {
        let weight = NonNegativeInt::new(weight).expect("a spawn weight is not negative");
        self.0.add_spawn(
            category,
            *mob.id.location(),
            weight,
            IntProvider::between(min, max),
        );
        self
    }

    pub fn cost(&mut self, mob: Mob, charge: f64, energy_budget: f64) -> &mut Self {
        self.0.add_cost(*mob.id.location(), charge, energy_budget);
        self
    }
}

#[derive(Serialize)]
pub struct Music {
    sound: SoundKey,
    min_delay: i32,
    max_delay: i32,
}

impl Music {
    pub fn game(sound: SoundKey) -> Self {
        Music {
            sound,
            min_delay: 12000,
            max_delay: 24000,
        }
    }
}

#[derive(Serialize, Default)]
pub struct BackgroundMusic {
    #[serde(skip_serializing_if = "Option::is_none")]
    default: Option<Music>,
    #[serde(skip_serializing_if = "Option::is_none")]
    creative: Option<Music>,
    #[serde(skip_serializing_if = "Option::is_none")]
    underwater: Option<Music>,
}

impl BackgroundMusic {
    pub fn of(sound: SoundKey) -> Self {
        BackgroundMusic {
            default: Some(Music::game(sound)),
            ..Default::default()
        }
    }

    pub fn overworld_with_underwater() -> Self {
        BackgroundMusic {
            default: Some(Music::game(sound::MUSIC_GAME)),
            creative: Some(Music::game(sound::MUSIC_CREATIVE)),
            underwater: Some(Music::game(sound::MUSIC_UNDER_WATER)),
        }
    }
}

pub trait BiomeMusic: Sized {
    fn background_music(self, music: BackgroundMusic) -> Self;

    fn music(self, sound: SoundKey) -> Self {
        self.background_music(BackgroundMusic::of(sound))
    }
}

impl BiomeMusic for Biome {
    fn background_music(self, music: BackgroundMusic) -> Self {
        self.modified(BACKGROUND_MUSIC, Operation::Override, music)
    }
}

#[rustfmt::skip]
const BIOMES: &[(Id, fn() -> Biome)] = {
    use end as e;
    use nether as n;
    use overworld as o;
    &[
        (rl!("minecraft:the_void"), o::the_void),
        (rl!("minecraft:plains"), || o::plains(false, false, false)),
        (rl!("minecraft:sunflower_plains"), || o::plains(true, false, false)),
        (rl!("minecraft:snowy_plains"), || o::plains(false, true, false)),
        (rl!("minecraft:ice_spikes"), || o::plains(false, true, true)),
        (rl!("minecraft:desert"), o::desert),
        (rl!("minecraft:swamp"), o::swamp),
        (rl!("minecraft:mangrove_swamp"), o::mangrove_swamp),
        (rl!("minecraft:forest"), || o::forest(false, false, false)),
        (rl!("minecraft:flower_forest"), || o::forest(false, false, true)),
        (rl!("minecraft:birch_forest"), || o::forest(true, false, false)),
        (rl!("minecraft:dappled_forest"), o::dappled_forest),
        (rl!("minecraft:dark_forest"), || o::dark_forest(false)),
        (rl!("minecraft:pale_garden"), || o::dark_forest(true)),
        (rl!("minecraft:old_growth_birch_forest"), || o::forest(true, true, false)),
        (rl!("minecraft:old_growth_pine_taiga"), || o::old_growth_taiga(false)),
        (rl!("minecraft:old_growth_spruce_taiga"), || o::old_growth_taiga(true)),
        (rl!("minecraft:taiga"), || o::taiga(false)),
        (rl!("minecraft:snowy_taiga"), || o::taiga(true)),
        (rl!("minecraft:savanna"), || o::savanna(false, false)),
        (rl!("minecraft:savanna_plateau"), || o::savanna(false, true)),
        (rl!("minecraft:windswept_hills"), || o::windswept_hills(false)),
        (rl!("minecraft:windswept_gravelly_hills"), || o::windswept_hills(false)),
        (rl!("minecraft:windswept_forest"), || o::windswept_hills(true)),
        (rl!("minecraft:windswept_savanna"), || o::savanna(true, false)),
        (rl!("minecraft:jungle"), || o::jungle(false)),
        (rl!("minecraft:sparse_jungle"), o::sparse_jungle),
        (rl!("minecraft:bamboo_jungle"), || o::jungle(true)),
        (rl!("minecraft:badlands"), || o::badlands(false)),
        (rl!("minecraft:eroded_badlands"), || o::badlands(false)),
        (rl!("minecraft:wooded_badlands"), || o::badlands(true)),
        (rl!("minecraft:meadow"), || o::meadow_or_cherry_grove(false)),
        (rl!("minecraft:cherry_grove"), || o::meadow_or_cherry_grove(true)),
        (rl!("minecraft:grove"), o::grove),
        (rl!("minecraft:snowy_slopes"), o::snowy_slopes),
        (rl!("minecraft:frozen_peaks"), || o::peaks(sound::MUSIC_OVERWORLD_FROZEN_PEAKS)),
        (rl!("minecraft:jagged_peaks"), || o::peaks(sound::MUSIC_OVERWORLD_JAGGED_PEAKS)),
        (rl!("minecraft:stony_peaks"), o::stony_peaks),
        (rl!("minecraft:river"), || o::river(false)),
        (rl!("minecraft:frozen_river"), || o::river(true)),
        (rl!("minecraft:beach"), || o::beach(false, false)),
        (rl!("minecraft:snowy_beach"), || o::beach(true, false)),
        (rl!("minecraft:stony_shore"), || o::beach(false, true)),
        (rl!("minecraft:warm_ocean"), o::warm_ocean),
        (rl!("minecraft:lukewarm_ocean"), || o::lukewarm_ocean(false)),
        (rl!("minecraft:deep_lukewarm_ocean"), || o::lukewarm_ocean(true)),
        (rl!("minecraft:ocean"), || o::ocean(false)),
        (rl!("minecraft:deep_ocean"), || o::ocean(true)),
        (rl!("minecraft:cold_ocean"), || o::cold_ocean(false)),
        (rl!("minecraft:deep_cold_ocean"), || o::cold_ocean(true)),
        (rl!("minecraft:frozen_ocean"), || o::frozen_ocean(false)),
        (rl!("minecraft:deep_frozen_ocean"), || o::frozen_ocean(true)),
        (rl!("minecraft:mushroom_fields"), o::mushroom_fields),
        (rl!("minecraft:dripstone_caves"), o::dripstone_caves),
        (rl!("minecraft:lush_caves"), o::lush_caves),
        (rl!("minecraft:deep_dark"), o::deep_dark),
        (rl!("minecraft:sulfur_caves"), o::sulfur_caves),
        (rl!("minecraft:nether_wastes"), n::nether_wastes),
        (rl!("minecraft:warped_forest"), n::warped_forest),
        (rl!("minecraft:crimson_forest"), n::crimson_forest),
        (rl!("minecraft:soul_sand_valley"), n::soul_sand_valley),
        (rl!("minecraft:basalt_deltas"), n::basalt_deltas),
        (rl!("minecraft:the_end"), e::the_end),
        (rl!("minecraft:end_highlands"), e::end_highlands),
        (rl!("minecraft:end_midlands"), || e::end_base(Generation::default())),
        (rl!("minecraft:small_end_islands"), e::small_end_islands),
        (rl!("minecraft:end_barrens"), || e::end_base(Generation::default())),
    ]
};

pub fn all() -> impl Iterator<Item = (ResourceLocation, fn() -> Biome)> {
    BIOMES.iter().map(|(id, build)| ((*id).into(), *build))
}

// chisle: a linear scan of 67 rows per biome read. A sorted table and a binary
// search lift it if the table grows.
pub fn build(id: &ResourceLocation) -> Option<Biome> {
    let (_, build) = BIOMES.iter().find(|(key, _)| key == id)?;
    Some(build())
}
