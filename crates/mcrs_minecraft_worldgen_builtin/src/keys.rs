use mcrs_minecraft_core::{ResourceKey, rl};
use mcrs_minecraft_item::component::sound::SoundEvent;
use mcrs_minecraft_keys::{Carver, ParticleType};
use mcrs_minecraft_worldgen_feature::proto::StructureProcessorList;

pub use mcrs_minecraft_biome::PlacedFeatureKey as PlacedKey;
pub use mcrs_minecraft_core::StaticResourceLocation as Id;
pub type ProcessorsKey = ResourceKey<StructureProcessorList, &'static str>;
pub type SoundKey = ResourceKey<SoundEvent, &'static str>;
pub type CarverKey = ResourceKey<Carver, &'static str>;
pub type ParticleKey = ResourceKey<ParticleType, &'static str>;

macro_rules! keys {
    (@ $key:ty; $($name:ident = $value:expr),*) => {
        $(pub const $name: $key = $value;)*

        #[cfg(test)]
        pub const ALL: &[$key] = &[$($name),*];
    };
    ($key:ty; $($name:ident = $id:literal),* $(,)?) => {
        keys!(@ $key; $($name = ResourceKey::new(rl!($id))),*);
    };
}

macro_rules! placed {
    ($path:literal) => {
        mcrs_minecraft_core::ResourceKey::new(
            const { crate::keys::Id::new_static(concat!("minecraft:", $path)) },
        )
    };
}
pub(crate) use placed;

pub mod carver {
    use super::*;

    keys! { CarverKey;
        CAVE = "minecraft:cave",
        CAVE_EXTRA_UNDERGROUND = "minecraft:cave_extra_underground",
        CANYON = "minecraft:canyon",
        NETHER_CAVE = "minecraft:nether_cave",
    }
}

pub mod sound {
    use super::*;

    keys! { SoundKey;
        MUSIC_GAME = "minecraft:music.game",
        MUSIC_CREATIVE = "minecraft:music.creative",
        MUSIC_UNDER_WATER = "minecraft:music.under_water",
        MUSIC_OVERWORLD_FROZEN_PEAKS = "minecraft:music.overworld.frozen_peaks",
        MUSIC_OVERWORLD_JAGGED_PEAKS = "minecraft:music.overworld.jagged_peaks",
        MUSIC_OVERWORLD_FOREST = "minecraft:music.overworld.forest",
        MUSIC_OVERWORLD_BAMBOO_JUNGLE = "minecraft:music.overworld.bamboo_jungle",
        MUSIC_OVERWORLD_JUNGLE = "minecraft:music.overworld.jungle",
        MUSIC_OVERWORLD_FLOWER_FOREST = "minecraft:music.overworld.flower_forest",
        MUSIC_OVERWORLD_SWAMP = "minecraft:music.overworld.swamp",
        MUSIC_OVERWORLD_OLD_GROWTH_TAIGA = "minecraft:music.overworld.old_growth_taiga",
        MUSIC_OVERWORLD_SPARSE_JUNGLE = "minecraft:music.overworld.sparse_jungle",
        MUSIC_OVERWORLD_DESERT = "minecraft:music.overworld.desert",
        MUSIC_OVERWORLD_BADLANDS = "minecraft:music.overworld.badlands",
        MUSIC_OVERWORLD_CHERRY_GROVE = "minecraft:music.overworld.cherry_grove",
        MUSIC_OVERWORLD_MEADOW = "minecraft:music.overworld.meadow",
        MUSIC_OVERWORLD_STONY_PEAKS = "minecraft:music.overworld.stony_peaks",
        MUSIC_OVERWORLD_SNOWY_SLOPES = "minecraft:music.overworld.snowy_slopes",
        MUSIC_OVERWORLD_GROVE = "minecraft:music.overworld.grove",
        MUSIC_OVERWORLD_SULFUR_CAVES = "minecraft:music.overworld.sulfur_caves",
        MUSIC_OVERWORLD_LUSH_CAVES = "minecraft:music.overworld.lush_caves",
        MUSIC_OVERWORLD_DRIPSTONE_CAVES = "minecraft:music.overworld.dripstone_caves",
        MUSIC_OVERWORLD_DEEP_DARK = "minecraft:music.overworld.deep_dark",
    }

    pub mod nether_wastes {
        use super::*;

        keys! { SoundKey;
            MUSIC = "minecraft:music.nether.nether_wastes",
            LOOP = "minecraft:ambient.nether_wastes.loop",
            MOOD = "minecraft:ambient.nether_wastes.mood",
            ADDITIONS = "minecraft:ambient.nether_wastes.additions",
        }
    }

    pub mod soul_sand_valley {
        use super::*;

        keys! { SoundKey;
            MUSIC = "minecraft:music.nether.soul_sand_valley",
            LOOP = "minecraft:ambient.soul_sand_valley.loop",
            MOOD = "minecraft:ambient.soul_sand_valley.mood",
            ADDITIONS = "minecraft:ambient.soul_sand_valley.additions",
        }
    }

    pub mod basalt_deltas {
        use super::*;

        keys! { SoundKey;
            MUSIC = "minecraft:music.nether.basalt_deltas",
            LOOP = "minecraft:ambient.basalt_deltas.loop",
            MOOD = "minecraft:ambient.basalt_deltas.mood",
            ADDITIONS = "minecraft:ambient.basalt_deltas.additions",
        }
    }

    pub mod crimson_forest {
        use super::*;

        keys! { SoundKey;
            MUSIC = "minecraft:music.nether.crimson_forest",
            LOOP = "minecraft:ambient.crimson_forest.loop",
            MOOD = "minecraft:ambient.crimson_forest.mood",
            ADDITIONS = "minecraft:ambient.crimson_forest.additions",
        }
    }

    pub mod warped_forest {
        use super::*;

        keys! { SoundKey;
            MUSIC = "minecraft:music.nether.warped_forest",
            LOOP = "minecraft:ambient.warped_forest.loop",
            MOOD = "minecraft:ambient.warped_forest.mood",
            ADDITIONS = "minecraft:ambient.warped_forest.additions",
        }
    }

    #[cfg(test)]
    pub fn all() -> impl Iterator<Item = SoundKey> {
        ALL.iter()
            .chain(nether_wastes::ALL)
            .chain(soul_sand_valley::ALL)
            .chain(basalt_deltas::ALL)
            .chain(crimson_forest::ALL)
            .chain(warped_forest::ALL)
            .copied()
    }
}

pub mod particle {
    use super::*;

    keys! { ParticleKey;
        ASH = "minecraft:ash",
        WHITE_ASH = "minecraft:white_ash",
        CRIMSON_SPORE = "minecraft:crimson_spore",
        WARPED_SPORE = "minecraft:warped_spore",
    }
}

pub mod processors {
    use super::*;

    keys! { ProcessorsKey;
        ANCIENT_CITY_GENERIC_DEGRADATION = "minecraft:ancient_city_generic_degradation",
        ANCIENT_CITY_START_DEGRADATION = "minecraft:ancient_city_start_degradation",
        ANCIENT_CITY_WALLS_DEGRADATION = "minecraft:ancient_city_walls_degradation",
        BASTION_GENERIC_DEGRADATION = "minecraft:bastion_generic_degradation",
        BOTTOM_RAMPART = "minecraft:bottom_rampart",
        BRIDGE = "minecraft:bridge",
        ENTRANCE_REPLACEMENT = "minecraft:entrance_replacement",
        FARM_DESERT = "minecraft:farm_desert",
        FARM_PLAINS = "minecraft:farm_plains",
        FARM_SAVANNA = "minecraft:farm_savanna",
        FARM_SNOWY = "minecraft:farm_snowy",
        FARM_TAIGA = "minecraft:farm_taiga",
        HIGH_RAMPART = "minecraft:high_rampart",
        HIGH_WALL = "minecraft:high_wall",
        HOUSING = "minecraft:housing",
        MOSSIFY_10_PERCENT = "minecraft:mossify_10_percent",
        MOSSIFY_20_PERCENT = "minecraft:mossify_20_percent",
        MOSSIFY_70_PERCENT = "minecraft:mossify_70_percent",
        OUTPOST_ROT = "minecraft:outpost_rot",
        RAMPART_DEGRADATION = "minecraft:rampart_degradation",
        ROOF = "minecraft:roof",
        SIDE_WALL_DEGRADATION = "minecraft:side_wall_degradation",
        STABLE_DEGRADATION = "minecraft:stable_degradation",
        STREET_PLAINS = "minecraft:street_plains",
        STREET_SAVANNA = "minecraft:street_savanna",
        STREET_SNOWY_OR_TAIGA = "minecraft:street_snowy_or_taiga",
        TRAIL_RUINS_HOUSES_ARCHAEOLOGY = "minecraft:trail_ruins_houses_archaeology",
        TRAIL_RUINS_ROADS_ARCHAEOLOGY = "minecraft:trail_ruins_roads_archaeology",
        TRAIL_RUINS_TOWER_TOP_ARCHAEOLOGY = "minecraft:trail_ruins_tower_top_archaeology",
        TREASURE_ROOMS = "minecraft:treasure_rooms",
        TRIAL_CHAMBERS_COPPER_BULB_DEGRADATION = "minecraft:trial_chambers_copper_bulb_degradation",
        ZOMBIE_DESERT = "minecraft:zombie_desert",
        ZOMBIE_PLAINS = "minecraft:zombie_plains",
        ZOMBIE_SAVANNA = "minecraft:zombie_savanna",
        ZOMBIE_SNOWY = "minecraft:zombie_snowy",
        ZOMBIE_TAIGA = "minecraft:zombie_taiga",
    }
}

#[cfg(test)]
pub(crate) mod report {
    use mcrs_minecraft_core::ResourceKey;
    use mcrs_minecraft_core::registry_key::RegistryKey;
    use mcrs_minecraft_registry::static_report::shipped_report;
    use std::collections::BTreeSet;

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

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_worldgen_feature::proto::{Holder, PlacedFeature};
    use mcrs_minecraft_worldgen_structure::PoolElement;
    use std::path::Path;

    fn shipped(folder: &str, namespace: &str, path: &str) -> bool {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .join(namespace)
            .join("worldgen")
            .join(folder)
            .join(format!("{path}.json"))
            .is_file()
    }

    #[test]
    fn every_key_names_an_entry_the_pack_ships() {
        for key in carver::ALL {
            let id = key.location();
            assert!(shipped("carver", id.namespace(), id.path()), "{id}");
        }
        for key in processors::ALL {
            let id = key.location();
            assert!(shipped("processor_list", id.namespace(), id.path()), "{id}");
        }
    }

    fn placed_in(element: &PoolElement) -> Vec<&Holder<PlacedFeature>> {
        match element {
            PoolElement::Feature { feature, .. } => vec![feature],
            PoolElement::List { elements, .. } => elements.iter().flat_map(placed_in).collect(),
            _ => Vec::new(),
        }
    }

    #[test]
    fn every_placed_feature_a_built_in_names_is_one_the_pack_ships() {
        let biomes = crate::biomes();
        let pools = crate::template_pools();
        let by_biomes = biomes
            .values()
            .flat_map(|biome| &biome.features)
            .flat_map(|step| step.entries());
        let by_pools = pools
            .values()
            .flat_map(|pool| &pool.elements)
            .flat_map(|entry| placed_in(&entry.element));
        for holder in by_biomes.chain(by_pools) {
            let Holder::Reference(id) = holder else {
                panic!("a built-in writes a placed feature inline");
            };
            assert!(shipped("placed_feature", id.namespace(), id.path()), "{id}");
        }
    }

    #[test]
    fn every_sound_is_a_sound_event_of_the_report() {
        let all: Vec<_> = sound::all().collect();
        assert_eq!(report::missing(&all), Vec::<String>::new());
        assert_eq!(report::repeated(&all), Vec::<String>::new());
    }

    #[test]
    fn every_particle_is_a_particle_type_of_the_report() {
        assert_eq!(report::missing(particle::ALL), Vec::<String>::new());
        assert_eq!(report::repeated(particle::ALL), Vec::<String>::new());
    }
}
