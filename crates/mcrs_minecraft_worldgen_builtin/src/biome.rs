mod defaults;
mod end;
mod nether;
mod overworld;

use mcrs_minecraft_biome::{
    Biome, BiomeDraft as Draft, BiomeGeneration as Generation, GrassColorModifier,
};
use mcrs_minecraft_core::codec::{HexRgb, NonNegativeInt};
use mcrs_minecraft_core::value_provider::IntProvider;
use mcrs_minecraft_core::{RegistryKey, ResourceKey, ResourceLocation};
use mcrs_minecraft_environment::attribute::id::*;
use mcrs_minecraft_environment::attribute::{MobSpawnSettings, Operation};
use mcrs_minecraft_keys::{EntityType, SoundEvent, biome, carver, placed_feature, sound_event};
use mcrs_minecraft_registry::{Built, Id, RegistrySet};
use mcrs_minecraft_worldgen_structure::MobCategory;
use serde::Serialize;

type BiomeRow = (
    ResourceKey<mcrs_minecraft_keys::Biome, &'static str>,
    fn() -> Draft,
);

#[derive(Clone, Copy)]
pub struct Mob {
    id: ResourceKey<EntityType, &'static str>,
    category: MobCategory,
}

pub mod mob {
    use super::Mob;
    use mcrs_minecraft_core::ResourceKey;
    use mcrs_minecraft_keys as keys;
    use mcrs_minecraft_worldgen_structure::MobCategory::*;

    macro_rules! mobs {
        ($($name:ident = $id:expr, $category:ident;)*) => {
            $(pub const $name: Mob = Mob {
                id: ResourceKey::new($id.location()),
                category: $category,
            };)*

            #[cfg(test)]
            pub const ALL: &[Mob] = &[$($name),*];
        };
    }

    mobs! {
        ARMADILLO = keys::entity_type::ARMADILLO, Creature;
        AXOLOTL = keys::entity_type::AXOLOTL, Axolotls;
        BAT = keys::entity_type::BAT, Ambient;
        BOGGED = keys::entity_type::BOGGED, Monster;
        CAMEL = keys::entity_type::CAMEL, Creature;
        CAVE_SPIDER = keys::entity_type::CAVE_SPIDER, Monster;
        CHICKEN = keys::entity_type::CHICKEN, Creature;
        COD = keys::entity_type::COD, WaterAmbient;
        COW = keys::entity_type::COW, Creature;
        CREEPER = keys::entity_type::CREEPER, Monster;
        DOLPHIN = keys::entity_type::DOLPHIN, WaterCreature;
        DONKEY = keys::entity_type::DONKEY, Creature;
        DROWNED = keys::entity_type::DROWNED, Monster;
        ENDERMAN = keys::entity_type::ENDERMAN, Monster;
        FOX = keys::entity_type::FOX, Creature;
        FROG = keys::entity_type::FROG, Creature;
        GHAST = keys::entity_type::GHAST, Monster;
        GLOW_SQUID = keys::entity_type::GLOW_SQUID, UndergroundWaterCreature;
        GOAT = keys::entity_type::GOAT, Creature;
        HOGLIN = keys::entity_type::HOGLIN, Monster;
        HORSE = keys::entity_type::HORSE, Creature;
        HUSK = keys::entity_type::HUSK, Monster;
        LLAMA = keys::entity_type::LLAMA, Creature;
        MAGMA_CUBE = keys::entity_type::MAGMA_CUBE, Monster;
        MOOSHROOM = keys::entity_type::MOOSHROOM, Creature;
        NAUTILUS = keys::entity_type::NAUTILUS, WaterCreature;
        OCELOT = keys::entity_type::OCELOT, Creature;
        PANDA = keys::entity_type::PANDA, Creature;
        PARCHED = keys::entity_type::PARCHED, Monster;
        PARROT = keys::entity_type::PARROT, Creature;
        PIG = keys::entity_type::PIG, Creature;
        PIGLIN = keys::entity_type::PIGLIN, Monster;
        POLAR_BEAR = keys::entity_type::POLAR_BEAR, Creature;
        PUFFERFISH = keys::entity_type::PUFFERFISH, WaterAmbient;
        RABBIT = keys::entity_type::RABBIT, Creature;
        SALMON = keys::entity_type::SALMON, WaterAmbient;
        SHEEP = keys::entity_type::SHEEP, Creature;
        SKELETON = keys::entity_type::SKELETON, Monster;
        SLIME = keys::entity_type::SLIME, Monster;
        SPIDER = keys::entity_type::SPIDER, Monster;
        SQUID = keys::entity_type::SQUID, WaterCreature;
        STRAY = keys::entity_type::STRAY, Monster;
        STRIDER = keys::entity_type::STRIDER, Creature;
        SULFUR_CUBE = keys::entity_type::SULFUR_CUBE, Monster;
        TROPICAL_FISH = keys::entity_type::TROPICAL_FISH, WaterAmbient;
        TURTLE = keys::entity_type::TURTLE, Creature;
        WITCH = keys::entity_type::WITCH, Monster;
        WOLF = keys::entity_type::WOLF, Creature;
        ZOMBIE = keys::entity_type::ZOMBIE, Monster;
        ZOMBIE_HORSE = keys::entity_type::ZOMBIE_HORSE, Monster;
        ZOMBIE_VILLAGER = keys::entity_type::ZOMBIE_VILLAGER, Monster;
        ZOMBIFIED_PIGLIN = keys::entity_type::ZOMBIFIED_PIGLIN, Monster;
    }

    #[cfg(test)]
    mod tests {
        use super::ALL;
        use mcrs_minecraft_core::ResourceKey;
        use mcrs_minecraft_core::registry_key::RegistryKey;
        use mcrs_minecraft_registry::static_report::shipped_report;
        use std::collections::BTreeSet;

        mod report {
            use super::*;

            pub fn missing<T: RegistryKey>(keys: &[ResourceKey<T, &'static str>]) -> Vec<String> {
                let table = shipped_report().table(T::KEY.as_str());
                keys.iter()
                    .map(|key| key.as_str())
                    .filter(|name| table.is_none_or(|table| table.number(name).is_none()))
                    .map(str::to_owned)
                    .collect()
            }

            pub fn repeated<T>(keys: &[ResourceKey<T, &'static str>]) -> Vec<String> {
                let mut seen = BTreeSet::new();
                keys.iter()
                    .map(|key| key.as_str())
                    .filter(|name| !seen.insert(*name))
                    .map(str::to_owned)
                    .collect()
            }
        }

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
    sound: &'static str,
    min_delay: i32,
    max_delay: i32,
}

impl Music {
    pub fn game(sound: Id<SoundEvent>) -> Self {
        Music {
            sound: sound.name(),
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
    pub fn of(sound: Id<SoundEvent>) -> Self {
        BackgroundMusic {
            default: Some(Music::game(sound)),
            ..Default::default()
        }
    }

    pub fn overworld_with_underwater() -> Self {
        BackgroundMusic {
            default: Some(Music::game(sound_event::MUSIC_GAME)),
            creative: Some(Music::game(sound_event::MUSIC_CREATIVE)),
            underwater: Some(Music::game(sound_event::MUSIC_UNDER_WATER)),
        }
    }
}

pub trait BiomeMusic: Sized {
    fn background_music(self, music: BackgroundMusic) -> Self;

    fn music(self, sound: Id<SoundEvent>) -> Self {
        self.background_music(BackgroundMusic::of(sound))
    }
}

impl BiomeMusic for Draft {
    fn background_music(self, music: BackgroundMusic) -> Self {
        self.modified(BACKGROUND_MUSIC, Operation::Override, music)
    }
}

#[rustfmt::skip]
const BIOMES: &[BiomeRow] = {
    use end as e;
    use nether as n;
    use overworld as o;
    &[
        (biome::THE_VOID, o::the_void),
        (biome::PLAINS, || o::plains(false, false, false)),
        (biome::SUNFLOWER_PLAINS, || o::plains(true, false, false)),
        (biome::SNOWY_PLAINS, || o::plains(false, true, false)),
        (biome::ICE_SPIKES, || o::plains(false, true, true)),
        (biome::DESERT, o::desert),
        (biome::SWAMP, o::swamp),
        (biome::MANGROVE_SWAMP, o::mangrove_swamp),
        (biome::FOREST, || o::forest(false, false, false)),
        (biome::FLOWER_FOREST, || o::forest(false, false, true)),
        (biome::BIRCH_FOREST, || o::forest(true, false, false)),
        (biome::DAPPLED_FOREST, o::dappled_forest),
        (biome::DARK_FOREST, || o::dark_forest(false)),
        (biome::PALE_GARDEN, || o::dark_forest(true)),
        (biome::OLD_GROWTH_BIRCH_FOREST, || o::forest(true, true, false)),
        (biome::OLD_GROWTH_PINE_TAIGA, || o::old_growth_taiga(false)),
        (biome::OLD_GROWTH_SPRUCE_TAIGA, || o::old_growth_taiga(true)),
        (biome::TAIGA, || o::taiga(false)),
        (biome::SNOWY_TAIGA, || o::taiga(true)),
        (biome::SAVANNA, || o::savanna(false, false)),
        (biome::SAVANNA_PLATEAU, || o::savanna(false, true)),
        (biome::WINDSWEPT_HILLS, || o::windswept_hills(false)),
        (biome::WINDSWEPT_GRAVELLY_HILLS, || o::windswept_hills(false)),
        (biome::WINDSWEPT_FOREST, || o::windswept_hills(true)),
        (biome::WINDSWEPT_SAVANNA, || o::savanna(true, false)),
        (biome::JUNGLE, || o::jungle(false)),
        (biome::SPARSE_JUNGLE, o::sparse_jungle),
        (biome::BAMBOO_JUNGLE, || o::jungle(true)),
        (biome::BADLANDS, || o::badlands(false)),
        (biome::ERODED_BADLANDS, || o::badlands(false)),
        (biome::WOODED_BADLANDS, || o::badlands(true)),
        (biome::MEADOW, || o::meadow_or_cherry_grove(false)),
        (biome::CHERRY_GROVE, || o::meadow_or_cherry_grove(true)),
        (biome::GROVE, o::grove),
        (biome::SNOWY_SLOPES, o::snowy_slopes),
        (biome::FROZEN_PEAKS, || o::peaks(sound_event::MUSIC_OVERWORLD_FROZEN_PEAKS)),
        (biome::JAGGED_PEAKS, || o::peaks(sound_event::MUSIC_OVERWORLD_JAGGED_PEAKS)),
        (biome::STONY_PEAKS, o::stony_peaks),
        (biome::RIVER, || o::river(false)),
        (biome::FROZEN_RIVER, || o::river(true)),
        (biome::BEACH, || o::beach(false, false)),
        (biome::SNOWY_BEACH, || o::beach(true, false)),
        (biome::STONY_SHORE, || o::beach(false, true)),
        (biome::WARM_OCEAN, o::warm_ocean),
        (biome::LUKEWARM_OCEAN, || o::lukewarm_ocean(false)),
        (biome::DEEP_LUKEWARM_OCEAN, || o::lukewarm_ocean(true)),
        (biome::OCEAN, || o::ocean(false)),
        (biome::DEEP_OCEAN, || o::ocean(true)),
        (biome::COLD_OCEAN, || o::cold_ocean(false)),
        (biome::DEEP_COLD_OCEAN, || o::cold_ocean(true)),
        (biome::FROZEN_OCEAN, || o::frozen_ocean(false)),
        (biome::DEEP_FROZEN_OCEAN, || o::frozen_ocean(true)),
        (biome::MUSHROOM_FIELDS, o::mushroom_fields),
        (biome::DRIPSTONE_CAVES, o::dripstone_caves),
        (biome::LUSH_CAVES, o::lush_caves),
        (biome::DEEP_DARK, o::deep_dark),
        (biome::SULFUR_CAVES, o::sulfur_caves),
        (biome::NETHER_WASTES, n::nether_wastes),
        (biome::WARPED_FOREST, n::warped_forest),
        (biome::CRIMSON_FOREST, n::crimson_forest),
        (biome::SOUL_SAND_VALLEY, n::soul_sand_valley),
        (biome::BASALT_DELTAS, n::basalt_deltas),
        (biome::THE_END, e::the_end),
        (biome::END_HIGHLANDS, e::end_highlands),
        (biome::END_MIDLANDS, || e::end_base(Generation::default())),
        (biome::SMALL_END_ISLANDS, e::small_end_islands),
        (biome::END_BARRENS, || e::end_base(Generation::default())),
    ]
};

pub fn names() -> Vec<ResourceLocation> {
    BIOMES
        .iter()
        .map(|(key, _)| (*key.location()).into())
        .collect()
}

pub fn build(set: &RegistrySet) -> Result<Vec<Biome>, Vec<(usize, String)>> {
    let mut built = Vec::with_capacity(BIOMES.len());
    let mut failures = Vec::new();
    for (index, (_, draft)) in BIOMES.iter().enumerate() {
        match set.scope(|| draft().resolve(set)) {
            Ok(biome) => built.push(biome),
            Err(messages) => failures.push((index, messages.join("; "))),
        }
    }
    if failures.is_empty() {
        Ok(built)
    } else {
        Err(failures)
    }
}

pub fn built() -> Built {
    Built::new(mcrs_minecraft_keys::Biome::KEY, names(), build)
}
