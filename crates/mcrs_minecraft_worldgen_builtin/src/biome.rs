mod defaults;
mod end;
mod nether;
mod overworld;

use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_biome::GrassColorModifier;
use mcrs_minecraft_biome::biome_keys;
use mcrs_minecraft_biome::carver_keys;
use mcrs_minecraft_biome::placed_feature_keys;
use mcrs_minecraft_biome_file::{BiomeDraft as Draft, BiomeGeneration as Generation};
use mcrs_minecraft_core::codec::{Bounded, HexRgb};
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::spawn::MobCategory;
use mcrs_minecraft_environment::attribute_id::*;
use mcrs_minecraft_environment::{MobSpawnSettings, Operation};
use mcrs_minecraft_resource::{ResourceKey, ResourceLocationBuf};
use mcrs_minecraft_sound::sound_event_keys::SoundEvent;
use mcrs_minecraft_value_provider::IntProvider;
use serde::Serialize;

type BiomeRow = (ResourceKey<'static, Biome>, fn() -> Draft);

#[derive(Clone, Copy)]
pub struct Mob {
    id: ResourceKey<'static, EntityType>,
    category: MobCategory,
}

pub mod mob {
    use super::Mob;
    use mcrs_minecraft_entity::spawn::MobCategory::*;
    use mcrs_minecraft_resource::ResourceKey;

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
        ARMADILLO = mcrs_minecraft_entity::entity_type_keys::EntityType::Armadillo, Creature;
        AXOLOTL = mcrs_minecraft_entity::entity_type_keys::EntityType::Axolotl, Axolotls;
        BAT = mcrs_minecraft_entity::entity_type_keys::EntityType::Bat, Ambient;
        BOGGED = mcrs_minecraft_entity::entity_type_keys::EntityType::Bogged, Monster;
        CAMEL = mcrs_minecraft_entity::entity_type_keys::EntityType::Camel, Creature;
        CAVE_SPIDER = mcrs_minecraft_entity::entity_type_keys::EntityType::CaveSpider, Monster;
        CHICKEN = mcrs_minecraft_entity::entity_type_keys::EntityType::Chicken, Creature;
        COD = mcrs_minecraft_entity::entity_type_keys::EntityType::Cod, WaterAmbient;
        COW = mcrs_minecraft_entity::entity_type_keys::EntityType::Cow, Creature;
        CREEPER = mcrs_minecraft_entity::entity_type_keys::EntityType::Creeper, Monster;
        DOLPHIN = mcrs_minecraft_entity::entity_type_keys::EntityType::Dolphin, WaterCreature;
        DONKEY = mcrs_minecraft_entity::entity_type_keys::EntityType::Donkey, Creature;
        DROWNED = mcrs_minecraft_entity::entity_type_keys::EntityType::Drowned, Monster;
        ENDERMAN = mcrs_minecraft_entity::entity_type_keys::EntityType::Enderman, Monster;
        FOX = mcrs_minecraft_entity::entity_type_keys::EntityType::Fox, Creature;
        FROG = mcrs_minecraft_entity::entity_type_keys::EntityType::Frog, Creature;
        FROSTBITE = mcrs_minecraft_entity::entity_type_keys::EntityType::Frostbite, Monster;
        GHAST = mcrs_minecraft_entity::entity_type_keys::EntityType::Ghast, Monster;
        GLOW_SQUID = mcrs_minecraft_entity::entity_type_keys::EntityType::GlowSquid, UndergroundWaterCreature;
        GOAT = mcrs_minecraft_entity::entity_type_keys::EntityType::Goat, Creature;
        HOGLIN = mcrs_minecraft_entity::entity_type_keys::EntityType::Hoglin, Monster;
        HORSE = mcrs_minecraft_entity::entity_type_keys::EntityType::Horse, Creature;
        HUSK = mcrs_minecraft_entity::entity_type_keys::EntityType::Husk, Monster;
        LLAMA = mcrs_minecraft_entity::entity_type_keys::EntityType::Llama, Creature;
        MAGMA_CUBE = mcrs_minecraft_entity::entity_type_keys::EntityType::MagmaCube, Monster;
        MOOSHROOM = mcrs_minecraft_entity::entity_type_keys::EntityType::Mooshroom, Creature;
        NAUTILUS = mcrs_minecraft_entity::entity_type_keys::EntityType::Nautilus, WaterCreature;
        OCELOT = mcrs_minecraft_entity::entity_type_keys::EntityType::Ocelot, Creature;
        PANDA = mcrs_minecraft_entity::entity_type_keys::EntityType::Panda, Creature;
        PARCHED = mcrs_minecraft_entity::entity_type_keys::EntityType::Parched, Monster;
        PARROT = mcrs_minecraft_entity::entity_type_keys::EntityType::Parrot, Creature;
        PIG = mcrs_minecraft_entity::entity_type_keys::EntityType::Pig, Creature;
        PIGLIN = mcrs_minecraft_entity::entity_type_keys::EntityType::Piglin, Monster;
        POLAR_BEAR = mcrs_minecraft_entity::entity_type_keys::EntityType::PolarBear, Creature;
        PUFFERFISH = mcrs_minecraft_entity::entity_type_keys::EntityType::Pufferfish, WaterAmbient;
        RABBIT = mcrs_minecraft_entity::entity_type_keys::EntityType::Rabbit, Creature;
        SALMON = mcrs_minecraft_entity::entity_type_keys::EntityType::Salmon, WaterAmbient;
        SHEEP = mcrs_minecraft_entity::entity_type_keys::EntityType::Sheep, Creature;
        SKELETON = mcrs_minecraft_entity::entity_type_keys::EntityType::Skeleton, Monster;
        SLIME = mcrs_minecraft_entity::entity_type_keys::EntityType::Slime, Monster;
        SPIDER = mcrs_minecraft_entity::entity_type_keys::EntityType::Spider, Monster;
        SQUID = mcrs_minecraft_entity::entity_type_keys::EntityType::Squid, WaterCreature;
        STRAY = mcrs_minecraft_entity::entity_type_keys::EntityType::Stray, Monster;
        STRIDER = mcrs_minecraft_entity::entity_type_keys::EntityType::Strider, Creature;
        SULFUR_CUBE = mcrs_minecraft_entity::entity_type_keys::EntityType::SulfurCube, Monster;
        TROPICAL_FISH = mcrs_minecraft_entity::entity_type_keys::EntityType::TropicalFish, WaterAmbient;
        TURTLE = mcrs_minecraft_entity::entity_type_keys::EntityType::Turtle, Creature;
        WITCH = mcrs_minecraft_entity::entity_type_keys::EntityType::Witch, Monster;
        WOLF = mcrs_minecraft_entity::entity_type_keys::EntityType::Wolf, Creature;
        ZOMBIE = mcrs_minecraft_entity::entity_type_keys::EntityType::Zombie, Monster;
        ZOMBIE_HORSE = mcrs_minecraft_entity::entity_type_keys::EntityType::ZombieHorse, Monster;
        ZOMBIE_VILLAGER = mcrs_minecraft_entity::entity_type_keys::EntityType::ZombieVillager, Monster;
        ZOMBIFIED_PIGLIN = mcrs_minecraft_entity::entity_type_keys::EntityType::ZombifiedPiglin, Monster;
    }

    #[cfg(test)]
    mod tests {
        use super::ALL;
        use std::collections::BTreeSet;

        #[test]
        fn no_entity_type_is_listed_twice() {
            let mut seen = BTreeSet::new();
            let repeated: Vec<_> = ALL
                .iter()
                .map(|mob| mob.id)
                .filter(|name| !seen.insert(*name))
                .collect();
            assert!(repeated.is_empty(), "{repeated:?}");
        }
    }
}

#[derive(Default)]
pub struct Mobs(MobSpawnSettings);

impl Mobs {
    pub fn none() -> Self {
        Mobs(MobSpawnSettings::no_spawns())
    }

    pub fn spawn(&mut self, mob: Mob, weight: u16, min: i32, max: i32) -> &mut Self {
        self.spawn_as(mob, mob.category, weight, min, max)
    }

    pub fn spawn_as(
        &mut self,
        mob: Mob,
        category: MobCategory,
        weight: u16,
        min: i32,
        max: i32,
    ) -> &mut Self {
        self.0.add_spawn(
            category,
            mob.id.location(),
            Bounded(weight.into()),
            IntProvider::between(min, max),
        );
        self
    }

    pub fn cost(&mut self, mob: Mob, charge: f64, energy_budget: f64) -> &mut Self {
        self.0.add_cost(mob.id.location(), charge, energy_budget);
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
    pub fn game(sound: SoundEvent) -> Self {
        Music {
            sound: sound.as_static_str(),
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
    pub fn of(sound: SoundEvent) -> Self {
        BackgroundMusic {
            default: Some(Music::game(sound)),
            ..Default::default()
        }
    }

    pub fn overworld_with_underwater() -> Self {
        BackgroundMusic {
            default: Some(Music::game(SoundEvent::MusicGame)),
            creative: Some(Music::game(SoundEvent::MusicCreative)),
            underwater: Some(Music::game(SoundEvent::MusicUnderWater)),
        }
    }
}

pub trait BiomeMusic: Sized {
    fn background_music(self, music: BackgroundMusic) -> Self;

    fn music(self, sound: SoundEvent) -> Self {
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
        (biome_keys::THE_VOID, o::the_void),
        (biome_keys::PLAINS, || o::plains(false, false, false)),
        (biome_keys::SUNFLOWER_PLAINS, || o::plains(true, false, false)),
        (biome_keys::SNOWY_PLAINS, || o::plains(false, true, false)),
        (biome_keys::ICE_SPIKES, || o::plains(false, true, true)),
        (biome_keys::DESERT, o::desert),
        (biome_keys::SWAMP, o::swamp),
        (biome_keys::MANGROVE_SWAMP, o::mangrove_swamp),
        (biome_keys::FOREST, || o::forest(false, false, false)),
        (biome_keys::FLOWER_FOREST, || o::forest(false, false, true)),
        (biome_keys::BIRCH_FOREST, || o::forest(true, false, false)),
        (biome_keys::DAPPLED_FOREST, o::dappled_forest),
        (biome_keys::DARK_FOREST, || o::dark_forest(false)),
        (biome_keys::PALE_GARDEN, || o::dark_forest(true)),
        (biome_keys::OLD_GROWTH_BIRCH_FOREST, || o::forest(true, true, false)),
        (biome_keys::OLD_GROWTH_PINE_TAIGA, || o::old_growth_taiga(false)),
        (biome_keys::OLD_GROWTH_SPRUCE_TAIGA, || o::old_growth_taiga(true)),
        (biome_keys::TAIGA, || o::taiga(false)),
        (biome_keys::SNOWY_TAIGA, || o::taiga(true)),
        (biome_keys::SAVANNA, || o::savanna(false, false)),
        (biome_keys::SAVANNA_PLATEAU, || o::savanna(false, true)),
        (biome_keys::WINDSWEPT_HILLS, || o::windswept_hills(false)),
        (biome_keys::WINDSWEPT_GRAVELLY_HILLS, || o::windswept_hills(false)),
        (biome_keys::WINDSWEPT_FOREST, || o::windswept_hills(true)),
        (biome_keys::WINDSWEPT_SAVANNA, || o::savanna(true, false)),
        (biome_keys::JUNGLE, || o::jungle(false)),
        (biome_keys::SPARSE_JUNGLE, o::sparse_jungle),
        (biome_keys::BAMBOO_JUNGLE, || o::jungle(true)),
        (biome_keys::BADLANDS, || o::badlands(false)),
        (biome_keys::ERODED_BADLANDS, || o::badlands(false)),
        (biome_keys::WOODED_BADLANDS, || o::badlands(true)),
        (biome_keys::MEADOW, || o::meadow_or_cherry_grove(false)),
        (biome_keys::CHERRY_GROVE, || o::meadow_or_cherry_grove(true)),
        (biome_keys::GROVE, o::grove),
        (biome_keys::SNOWY_SLOPES, o::snowy_slopes),
        (biome_keys::FROZEN_PEAKS, || o::peaks(SoundEvent::MusicOverworldFrozenPeaks, 25)),
        (biome_keys::JAGGED_PEAKS, || o::peaks(SoundEvent::MusicOverworldJaggedPeaks, 10)),
        (biome_keys::STONY_PEAKS, o::stony_peaks),
        (biome_keys::RIVER, || o::river(false)),
        (biome_keys::FROZEN_RIVER, || o::river(true)),
        (biome_keys::BEACH, || o::beach(false, false)),
        (biome_keys::SNOWY_BEACH, || o::beach(true, false)),
        (biome_keys::STONY_SHORE, || o::beach(false, true)),
        (biome_keys::WARM_OCEAN, o::warm_ocean),
        (biome_keys::LUKEWARM_OCEAN, || o::lukewarm_ocean(false)),
        (biome_keys::DEEP_LUKEWARM_OCEAN, || o::lukewarm_ocean(true)),
        (biome_keys::OCEAN, || o::ocean(false)),
        (biome_keys::DEEP_OCEAN, || o::ocean(true)),
        (biome_keys::COLD_OCEAN, || o::cold_ocean(false)),
        (biome_keys::DEEP_COLD_OCEAN, || o::cold_ocean(true)),
        (biome_keys::FROZEN_OCEAN, || o::frozen_ocean(false)),
        (biome_keys::DEEP_FROZEN_OCEAN, || o::frozen_ocean(true)),
        (biome_keys::MUSHROOM_FIELDS, o::mushroom_fields),
        (biome_keys::DRIPSTONE_CAVES, o::dripstone_caves),
        (biome_keys::LUSH_CAVES, o::lush_caves),
        (biome_keys::DEEP_DARK, o::deep_dark),
        (biome_keys::SULFUR_CAVES, o::sulfur_caves),
        (biome_keys::ICE_CAVES, o::ice_caves),
        (biome_keys::NETHER_WASTES, n::nether_wastes),
        (biome_keys::WARPED_FOREST, n::warped_forest),
        (biome_keys::CRIMSON_FOREST, n::crimson_forest),
        (biome_keys::SOUL_SAND_VALLEY, n::soul_sand_valley),
        (biome_keys::BASALT_DELTAS, n::basalt_deltas),
        (biome_keys::THE_END, e::the_end),
        (biome_keys::END_HIGHLANDS, e::end_highlands),
        (biome_keys::END_MIDLANDS, || e::end_base(Generation::default())),
        (biome_keys::SMALL_END_ISLANDS, e::small_end_islands),
        (biome_keys::END_BARRENS, || e::end_base(Generation::default())),
    ]
};

/// Every biome as code describes it, its carvers and features still names.
pub fn drafts() -> impl Iterator<Item = (ResourceLocationBuf, Draft)> {
    BIOMES
        .iter()
        .map(|(key, draft)| (key.location().into(), draft()))
}
