use mcrs_minecraft_core::{ResourceKey, rl};
use mcrs_minecraft_worldgen_feature::proto::StructureProcessorList;

pub use mcrs_minecraft_biome::PlacedFeatureKey as PlacedKey;
pub use mcrs_minecraft_core::StaticResourceLocation as Id;
pub type ProcessorsKey = ResourceKey<StructureProcessorList, &'static str>;

macro_rules! keys {
    (@ $key:ty; $($name:ident = $value:expr),*) => {
        $(pub const $name: $key = $value;)*

        #[cfg(test)]
        pub const ALL: &[$key] = &[$($name),*];
    };
    (Id; $($name:ident = $id:literal),* $(,)?) => {
        keys!(@ Id; $($name = rl!($id)),*);
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

    keys! { Id;
        CAVE = "minecraft:cave",
        CAVE_EXTRA_UNDERGROUND = "minecraft:cave_extra_underground",
        CANYON = "minecraft:canyon",
        NETHER_CAVE = "minecraft:nether_cave",
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
        for id in carver::ALL {
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
}
