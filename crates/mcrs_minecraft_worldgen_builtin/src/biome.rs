mod defaults;
mod end;
mod nether;
mod overworld;

use crate::keys::{Id, carver, placed};
use mcrs_minecraft_biome::{
    Biome, BiomeEffects, BiomeGeneration as Generation, GrassColorModifier,
};
use mcrs_minecraft_core::codec::HexRgb;
use mcrs_minecraft_core::value_provider::IntProvider;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation, rl};
use mcrs_minecraft_environment::attribute::id::*;
use mcrs_minecraft_environment::attribute::{MobSpawnSettings, Operation};
use mcrs_minecraft_worldgen_structure::MobCategory;
use serde::Serialize;

pub type BiomeKey = ResourceKey<Biome, &'static str>;

pub const NORMAL_WATER_COLOR: i32 = Biome::NORMAL_WATER_COLOR;

#[derive(Clone, Copy)]
pub struct Mob {
    id: Id,
    category: MobCategory,
}

pub mod mob {
    use super::{Id, Mob};
    use mcrs_minecraft_core::rl;
    use mcrs_minecraft_worldgen_structure::MobCategory::{self, *};

    const fn mob(id: Id, category: MobCategory) -> Mob {
        Mob { id, category }
    }

    pub const ARMADILLO: Mob = mob(rl!("minecraft:armadillo"), Creature);
    pub const AXOLOTL: Mob = mob(rl!("minecraft:axolotl"), Axolotls);
    pub const BAT: Mob = mob(rl!("minecraft:bat"), Ambient);
    pub const BOGGED: Mob = mob(rl!("minecraft:bogged"), Monster);
    pub const CAMEL: Mob = mob(rl!("minecraft:camel"), Creature);
    pub const CAVE_SPIDER: Mob = mob(rl!("minecraft:cave_spider"), Monster);
    pub const CHICKEN: Mob = mob(rl!("minecraft:chicken"), Creature);
    pub const COD: Mob = mob(rl!("minecraft:cod"), WaterAmbient);
    pub const COW: Mob = mob(rl!("minecraft:cow"), Creature);
    pub const CREEPER: Mob = mob(rl!("minecraft:creeper"), Monster);
    pub const DOLPHIN: Mob = mob(rl!("minecraft:dolphin"), WaterCreature);
    pub const DONKEY: Mob = mob(rl!("minecraft:donkey"), Creature);
    pub const DROWNED: Mob = mob(rl!("minecraft:drowned"), Monster);
    pub const ENDERMAN: Mob = mob(rl!("minecraft:enderman"), Monster);
    pub const FOX: Mob = mob(rl!("minecraft:fox"), Creature);
    pub const FROG: Mob = mob(rl!("minecraft:frog"), Creature);
    pub const GHAST: Mob = mob(rl!("minecraft:ghast"), Monster);
    pub const GLOW_SQUID: Mob = mob(rl!("minecraft:glow_squid"), UndergroundWaterCreature);
    pub const GOAT: Mob = mob(rl!("minecraft:goat"), Creature);
    pub const HOGLIN: Mob = mob(rl!("minecraft:hoglin"), Monster);
    pub const HORSE: Mob = mob(rl!("minecraft:horse"), Creature);
    pub const HUSK: Mob = mob(rl!("minecraft:husk"), Monster);
    pub const LLAMA: Mob = mob(rl!("minecraft:llama"), Creature);
    pub const MAGMA_CUBE: Mob = mob(rl!("minecraft:magma_cube"), Monster);
    pub const MOOSHROOM: Mob = mob(rl!("minecraft:mooshroom"), Creature);
    pub const NAUTILUS: Mob = mob(rl!("minecraft:nautilus"), WaterCreature);
    pub const OCELOT: Mob = mob(rl!("minecraft:ocelot"), Creature);
    pub const PANDA: Mob = mob(rl!("minecraft:panda"), Creature);
    pub const PARCHED: Mob = mob(rl!("minecraft:parched"), Monster);
    pub const PARROT: Mob = mob(rl!("minecraft:parrot"), Creature);
    pub const PIG: Mob = mob(rl!("minecraft:pig"), Creature);
    pub const PIGLIN: Mob = mob(rl!("minecraft:piglin"), Monster);
    pub const POLAR_BEAR: Mob = mob(rl!("minecraft:polar_bear"), Creature);
    pub const PUFFERFISH: Mob = mob(rl!("minecraft:pufferfish"), WaterAmbient);
    pub const RABBIT: Mob = mob(rl!("minecraft:rabbit"), Creature);
    pub const SALMON: Mob = mob(rl!("minecraft:salmon"), WaterAmbient);
    pub const SHEEP: Mob = mob(rl!("minecraft:sheep"), Creature);
    pub const SKELETON: Mob = mob(rl!("minecraft:skeleton"), Monster);
    pub const SLIME: Mob = mob(rl!("minecraft:slime"), Monster);
    pub const SPIDER: Mob = mob(rl!("minecraft:spider"), Monster);
    pub const SQUID: Mob = mob(rl!("minecraft:squid"), WaterCreature);
    pub const STRAY: Mob = mob(rl!("minecraft:stray"), Monster);
    pub const STRIDER: Mob = mob(rl!("minecraft:strider"), Creature);
    pub const SULFUR_CUBE: Mob = mob(rl!("minecraft:sulfur_cube"), Monster);
    pub const TROPICAL_FISH: Mob = mob(rl!("minecraft:tropical_fish"), WaterAmbient);
    pub const TURTLE: Mob = mob(rl!("minecraft:turtle"), Creature);
    pub const WITCH: Mob = mob(rl!("minecraft:witch"), Monster);
    pub const WOLF: Mob = mob(rl!("minecraft:wolf"), Creature);
    pub const ZOMBIE: Mob = mob(rl!("minecraft:zombie"), Monster);
    pub const ZOMBIE_HORSE: Mob = mob(rl!("minecraft:zombie_horse"), Monster);
    pub const ZOMBIE_VILLAGER: Mob = mob(rl!("minecraft:zombie_villager"), Monster);
    pub const ZOMBIFIED_PIGLIN: Mob = mob(rl!("minecraft:zombified_piglin"), Monster);
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
        self.0
            .add_spawn(category, mob.id, weight, IntProvider::between(min, max));
        self
    }

    pub fn cost(&mut self, mob: Mob, charge: f64, energy_budget: f64) -> &mut Self {
        self.0.add_cost(mob.id, charge, energy_budget);
        self
    }
}

pub fn biome(
    has_precipitation: bool,
    temperature: f32,
    downfall: f32,
    mobs: Mobs,
    generation: Generation,
) -> Biome {
    Biome::new(has_precipitation, temperature, downfall, mobs.0, generation)
}

#[derive(Serialize)]
pub struct Music {
    sound: Id,
    min_delay: i32,
    max_delay: i32,
}

impl Music {
    pub fn game(sound: Id) -> Self {
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
    pub fn of(sound: Id) -> Self {
        BackgroundMusic {
            default: Some(Music::game(sound)),
            ..Default::default()
        }
    }

    pub fn overworld_with_underwater() -> Self {
        BackgroundMusic {
            default: Some(Music::game(rl!("minecraft:music.game"))),
            creative: Some(Music::game(rl!("minecraft:music.creative"))),
            underwater: Some(Music::game(rl!("minecraft:music.under_water"))),
        }
    }
}

pub trait BiomeMusic {
    fn music(self, sound: Id) -> Self;
}

impl BiomeMusic for Biome {
    fn music(self, sound: Id) -> Self {
        self.with(BACKGROUND_MUSIC, BackgroundMusic::of(sound))
    }
}

#[rustfmt::skip]
const BIOMES: &[(BiomeKey, fn() -> Biome)] = {
    use end as e;
    use nether as n;
    use overworld as o;
    const fn key(id: Id) -> BiomeKey {
        ResourceKey::new(id)
    }
    &[
        (key(rl!("minecraft:the_void")), o::the_void),
        (key(rl!("minecraft:plains")), || o::plains(false, false, false)),
        (key(rl!("minecraft:sunflower_plains")), || o::plains(true, false, false)),
        (key(rl!("minecraft:snowy_plains")), || o::plains(false, true, false)),
        (key(rl!("minecraft:ice_spikes")), || o::plains(false, true, true)),
        (key(rl!("minecraft:desert")), o::desert),
        (key(rl!("minecraft:swamp")), o::swamp),
        (key(rl!("minecraft:mangrove_swamp")), o::mangrove_swamp),
        (key(rl!("minecraft:forest")), || o::forest(false, false, false)),
        (key(rl!("minecraft:flower_forest")), || o::forest(false, false, true)),
        (key(rl!("minecraft:birch_forest")), || o::forest(true, false, false)),
        (key(rl!("minecraft:dappled_forest")), o::dappled_forest),
        (key(rl!("minecraft:dark_forest")), || o::dark_forest(false)),
        (key(rl!("minecraft:pale_garden")), || o::dark_forest(true)),
        (key(rl!("minecraft:old_growth_birch_forest")), || o::forest(true, true, false)),
        (key(rl!("minecraft:old_growth_pine_taiga")), || o::old_growth_taiga(false)),
        (key(rl!("minecraft:old_growth_spruce_taiga")), || o::old_growth_taiga(true)),
        (key(rl!("minecraft:taiga")), || o::taiga(false)),
        (key(rl!("minecraft:snowy_taiga")), || o::taiga(true)),
        (key(rl!("minecraft:savanna")), || o::savanna(false, false)),
        (key(rl!("minecraft:savanna_plateau")), || o::savanna(false, true)),
        (key(rl!("minecraft:windswept_hills")), || o::windswept_hills(false)),
        (key(rl!("minecraft:windswept_gravelly_hills")), || o::windswept_hills(false)),
        (key(rl!("minecraft:windswept_forest")), || o::windswept_hills(true)),
        (key(rl!("minecraft:windswept_savanna")), || o::savanna(true, false)),
        (key(rl!("minecraft:jungle")), o::jungle),
        (key(rl!("minecraft:sparse_jungle")), o::sparse_jungle),
        (key(rl!("minecraft:bamboo_jungle")), o::bamboo_jungle),
        (key(rl!("minecraft:badlands")), || o::badlands(false)),
        (key(rl!("minecraft:eroded_badlands")), || o::badlands(false)),
        (key(rl!("minecraft:wooded_badlands")), || o::badlands(true)),
        (key(rl!("minecraft:meadow")), || o::meadow_or_cherry_grove(false)),
        (key(rl!("minecraft:cherry_grove")), || o::meadow_or_cherry_grove(true)),
        (key(rl!("minecraft:grove")), o::grove),
        (key(rl!("minecraft:snowy_slopes")), o::snowy_slopes),
        (key(rl!("minecraft:frozen_peaks")), || o::peaks(rl!("minecraft:music.overworld.frozen_peaks"))),
        (key(rl!("minecraft:jagged_peaks")), || o::peaks(rl!("minecraft:music.overworld.jagged_peaks"))),
        (key(rl!("minecraft:stony_peaks")), o::stony_peaks),
        (key(rl!("minecraft:river")), || o::river(false)),
        (key(rl!("minecraft:frozen_river")), || o::river(true)),
        (key(rl!("minecraft:beach")), || o::beach(false, false)),
        (key(rl!("minecraft:snowy_beach")), || o::beach(true, false)),
        (key(rl!("minecraft:stony_shore")), || o::beach(false, true)),
        (key(rl!("minecraft:warm_ocean")), o::warm_ocean),
        (key(rl!("minecraft:lukewarm_ocean")), || o::lukewarm_ocean(false)),
        (key(rl!("minecraft:deep_lukewarm_ocean")), || o::lukewarm_ocean(true)),
        (key(rl!("minecraft:ocean")), || o::ocean(false)),
        (key(rl!("minecraft:deep_ocean")), || o::ocean(true)),
        (key(rl!("minecraft:cold_ocean")), || o::cold_ocean(false)),
        (key(rl!("minecraft:deep_cold_ocean")), || o::cold_ocean(true)),
        (key(rl!("minecraft:frozen_ocean")), || o::frozen_ocean(false)),
        (key(rl!("minecraft:deep_frozen_ocean")), || o::frozen_ocean(true)),
        (key(rl!("minecraft:mushroom_fields")), o::mushroom_fields),
        (key(rl!("minecraft:dripstone_caves")), o::dripstone_caves),
        (key(rl!("minecraft:lush_caves")), o::lush_caves),
        (key(rl!("minecraft:deep_dark")), o::deep_dark),
        (key(rl!("minecraft:sulfur_caves")), o::sulfur_caves),
        (key(rl!("minecraft:nether_wastes")), n::nether_wastes),
        (key(rl!("minecraft:warped_forest")), n::warped_forest),
        (key(rl!("minecraft:crimson_forest")), n::crimson_forest),
        (key(rl!("minecraft:soul_sand_valley")), n::soul_sand_valley),
        (key(rl!("minecraft:basalt_deltas")), n::basalt_deltas),
        (key(rl!("minecraft:the_end")), e::the_end),
        (key(rl!("minecraft:end_highlands")), e::end_highlands),
        (key(rl!("minecraft:end_midlands")), || e::end_base(Generation::default())),
        (key(rl!("minecraft:small_end_islands")), e::small_end_islands),
        (key(rl!("minecraft:end_barrens")), || e::end_base(Generation::default())),
    ]
};

pub fn all() -> impl Iterator<Item = (ResourceLocation, fn() -> Biome)> {
    BIOMES
        .iter()
        .map(|(key, build)| ((*key.location()).into(), *build))
}

pub fn build(id: &ResourceLocation) -> Option<Biome> {
    let (_, build) = BIOMES.iter().find(|(key, _)| key.location() == id)?;
    Some(build())
}
