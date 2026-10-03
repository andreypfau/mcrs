use super::defaults::*;
use super::mob::*;
use super::*;
use mcrs_minecraft_biome::TemperatureModifier;
use mcrs_minecraft_worldgen_structure::DecorationStep::*;

const DARK_DRY_FOLIAGE_COLOR: i32 = 8082228;
const COLD_WATER_COLOR: i32 = 4020182;
const FROZEN_WATER_COLOR: i32 = 3750089;
const FOREST_MUSIC: Id = rl!("minecraft:music.overworld.forest");
const SWAMP_SKELETON_WEIGHT: i32 = 70;

fn sky_color(temperature: f32) -> i32 {
    let temp = (temperature / 3.0).clamp(-1.0, 1.0);
    mcrs_minecraft_core::mth::hsv_to_rgb(0.62222224 - temp * 0.05, 0.5 + temp * 0.1, 1.0)
}

fn climate(temperature: f32, downfall: f32) -> Biome {
    Biome::new(true, temperature, downfall).with(SKY_COLOR, HexRgb::of(sky_color(temperature)))
}

fn base_biome(temperature: f32, downfall: f32, mobs: Mobs, generation: Generation) -> Biome {
    climate(temperature, downfall)
        .spawns(mobs.0)
        .generation(generation)
}

fn arid_biome(mobs: Mobs, generation: Generation) -> Biome {
    base_biome(2.0, 0.0, mobs, generation)
        .precipitation(false)
        .with(SNOW_GOLEM_MELTS, true)
}

fn swamp_biome(mobs: Mobs, generation: Generation) -> Biome {
    base_biome(0.8, 0.9, mobs, generation)
        .modified(WATER_FOG_END_DISTANCE, Operation::Multiply, 0.85)
        .music(rl!("minecraft:music.overworld.swamp"))
        .with(INCREASED_FIRE_BURNOUT, true)
        .dry_foliage(DARK_DRY_FOLIAGE_COLOR)
        .grass_modifier(GrassColorModifier::Swamp)
}

fn overworld_generation() -> Generation {
    let mut g = Generation::default();
    overworld_features(&mut g);
    g
}

pub fn old_growth_taiga(spruce: bool) -> Biome {
    let mut m = Mobs::default();
    m.taiga_animals();
    let (trees, temperature) = if spruce {
        m.common_spawns();
        (placed::TREES_OLD_GROWTH_SPRUCE_TAIGA, 0.25)
    } else {
        m.cave_spawns();
        m.monsters(100, 25, 0, 100);
        (placed::TREES_OLD_GROWTH_PINE_TAIGA, 0.3)
    };
    let mut g = overworld_generation();
    mossy_stone_block(&mut g);
    ferns(&mut g);
    g.feature(VegetalDecoration, trees);
    default_flowers(&mut g);
    giant_taiga_vegetation(&mut g);
    mushrooms_and_extra_vegetation(&mut g);
    common_berry_bushes(&mut g);
    base_biome(temperature, 0.8, m, g).music(rl!("minecraft:music.overworld.old_growth_taiga"))
}

pub fn sparse_jungle() -> Biome {
    let mut m = Mobs::default();
    m.base_jungle_spawns().spawn(WOLF, 8, 2, 4);
    base_jungle(0.8, false, true, m).music(rl!("minecraft:music.overworld.sparse_jungle"))
}

pub fn jungle(bamboo: bool) -> Biome {
    let (panda_weight, ocelot_max_count, sound) = if bamboo {
        (80, 1, rl!("minecraft:music.overworld.bamboo_jungle"))
    } else {
        (1, 3, rl!("minecraft:music.overworld.jungle"))
    };
    let mut m = Mobs::default();
    m.base_jungle_spawns()
        .spawn(PARROT, 40, 1, 2)
        .spawn(PANDA, panda_weight, 1, 2)
        .spawn_as(OCELOT, MobCategory::Monster, 2, 1, ocelot_max_count);
    base_jungle(0.9, bamboo, false, m)
        .music(sound)
        .with(INCREASED_FIRE_BURNOUT, true)
}

fn base_jungle(downfall: f32, bamboo: bool, sparse: bool, mobs: Mobs) -> Biome {
    let (trees, melons) = if sparse {
        (placed::TREES_SPARSE_JUNGLE, placed::PATCH_MELON_SPARSE)
    } else {
        (placed::TREES_JUNGLE, placed::PATCH_MELON)
    };
    let mut g = overworld_generation();
    if bamboo {
        bamboo_vegetation(&mut g);
    } else {
        if !sparse {
            light_bamboo_vegetation(&mut g);
        }
        g.feature(VegetalDecoration, trees);
    }
    warm_flowers(&mut g);
    jungle_grass(&mut g);
    mushrooms_and_extra_vegetation(&mut g);
    jungle_vines(&mut g);
    g.feature(VegetalDecoration, melons);
    base_biome(0.95, downfall, mobs, g)
}

pub fn windswept_hills(more_trees: bool) -> Biome {
    let mut m = Mobs::default();
    m.farm_animals().spawn(LLAMA, 5, 4, 6).common_spawns();
    let mut g = overworld_generation();
    if more_trees {
        mountain_forest_trees(&mut g);
    } else {
        mountain_trees(&mut g);
    }
    bushes(&mut g);
    default_vegetation(&mut g);
    mountain_ores(&mut g);
    base_biome(0.2, 0.3, m, g)
}

pub fn desert() -> Biome {
    let mut m = Mobs::default();
    m.desert_spawns();
    let mut g = Generation::default();
    fossil_decoration(&mut g);
    overworld_features(&mut g);
    default_flowers(&mut g);
    default_grass(&mut g);
    desert_vegetation(&mut g);
    default_mushrooms(&mut g);
    desert_extra_vegetation(&mut g);
    desert_extra_decoration(&mut g);
    arid_biome(m, g).music(rl!("minecraft:music.overworld.desert"))
}

pub fn plains(sunflower: bool, snowy: bool, spikes: bool) -> Biome {
    let mut m = Mobs::default();
    let mut g = overworld_generation();
    let biome = if snowy {
        m.snowy_spawns(!spikes);
        if spikes {
            g.features(SurfaceStructures, &[placed::ICE_SPIKE, placed::ICE_PATCH]);
        }
        snowy_trees(&mut g);
        default_flowers(&mut g);
        default_grass(&mut g);
        climate(0.0, 0.5).with(CREATURE_WORLD_GEN_SPAWN_PROBABILITY, 0.07)
    } else {
        m.plains_spawns();
        plain_grass(&mut g);
        if sunflower {
            g.feature(VegetalDecoration, placed::PATCH_SUNFLOWER);
        } else {
            bushes(&mut g);
        }
        plain_vegetation(&mut g);
        climate(0.8, 0.4)
    };
    mushrooms_and_extra_vegetation(&mut g);
    biome.spawns(m.0).generation(g)
}

pub fn mushroom_fields() -> Biome {
    let mut m = Mobs::default();
    m.mooshroom_spawns();
    let mut g = overworld_generation();
    mushroom_field_vegetation(&mut g);
    near_water_vegetation(&mut g);
    base_biome(0.9, 1.0, m, g)
        .with(INCREASED_FIRE_BURNOUT, true)
        .with(CAN_PILLAGER_PATROL_SPAWN, false)
}

pub fn savanna(shattered: bool, plateau: bool) -> Biome {
    let mut g = overworld_generation();
    if shattered {
        shattered_savanna_trees(&mut g);
        default_flowers(&mut g);
        shattered_savanna_grass(&mut g);
    } else {
        savanna_grass(&mut g);
        savanna_trees(&mut g);
        warm_flowers(&mut g);
        savanna_extra_grass(&mut g);
    }
    mushrooms_and_extra_vegetation(&mut g);
    let mut m = Mobs::default();
    m.farm_animals()
        .spawn(HORSE, 1, 2, 6)
        .spawn(DONKEY, 1, 1, 1)
        .spawn(ARMADILLO, 10, 2, 3)
        .common_spawn_with_zombie_horse();
    if plateau {
        m.spawn(LLAMA, 8, 4, 4).spawn(WOLF, 8, 4, 8);
    }
    arid_biome(m, g)
}

pub fn badlands(wooded: bool) -> Biome {
    let mut m = Mobs::default();
    m.farm_animals().common_spawns().spawn(ARMADILLO, 6, 1, 2);
    let mut g = Generation::default();
    global_overworld_generation(&mut g);
    default_ores(&mut g);
    extra_gold(&mut g);
    default_soft_disks(&mut g);
    let creature_spawn_probability = if wooded {
        m.spawn(WOLF, 2, 4, 8);
        badlands_trees(&mut g);
        0.04
    } else {
        0.03
    };
    badland_grass(&mut g);
    default_mushrooms(&mut g);
    badland_extra_vegetation(&mut g);
    arid_biome(m, g)
        .with(
            CREATURE_WORLD_GEN_SPAWN_PROBABILITY,
            creature_spawn_probability,
        )
        .music(rl!("minecraft:music.overworld.badlands"))
        .foliage(10387789)
        .grass(9470285)
}

fn base_ocean(mobs: Mobs, generation: Generation) -> Biome {
    base_biome(0.5, 0.5, mobs, generation)
        .background_music(BackgroundMusic::overworld_with_underwater())
}

fn base_ocean_generation() -> Generation {
    let mut g = overworld_generation();
    ocean_vegetation(&mut g);
    g
}

pub fn cold_ocean(deep: bool) -> Biome {
    let mut m = Mobs::default();
    m.ocean_spawns(3, 4, 15)
        .spawn(SALMON, 15, 1, 5)
        .spawn(NAUTILUS, 2, 1, 1);
    let mut g = base_ocean_generation();
    let seagrass = if deep {
        placed::SEAGRASS_DEEP_COLD
    } else {
        placed::SEAGRASS_COLD
    };
    g.feature(VegetalDecoration, seagrass);
    cold_ocean_extra_vegetation(&mut g);
    base_ocean(m, g).water(COLD_WATER_COLOR)
}

pub fn ocean(deep: bool) -> Biome {
    let mut m = Mobs::default();
    m.ocean_spawns(1, 4, 10)
        .spawn(DOLPHIN, 1, 1, 2)
        .spawn(NAUTILUS, 5, 1, 1);
    let mut g = base_ocean_generation();
    let seagrass = if deep {
        placed::SEAGRASS_DEEP
    } else {
        placed::SEAGRASS_NORMAL
    };
    g.feature(VegetalDecoration, seagrass);
    cold_ocean_extra_vegetation(&mut g);
    base_ocean(m, g)
}

pub fn lukewarm_ocean(deep: bool) -> Biome {
    let mut m = Mobs::default();
    let seagrass = if deep {
        m.ocean_spawns(8, 4, 8);
        placed::SEAGRASS_DEEP_WARM
    } else {
        m.ocean_spawns(10, 2, 15);
        placed::SEAGRASS_WARM
    };
    m.spawn(PUFFERFISH, 5, 1, 3)
        .spawn(TROPICAL_FISH, 25, 8, 8)
        .spawn(DOLPHIN, 2, 1, 2)
        .spawn(NAUTILUS, 5, 1, 1);
    let mut g = base_ocean_generation();
    g.feature(VegetalDecoration, seagrass);
    lukewarm_kelp(&mut g);
    base_ocean(m, g)
        .with(WATER_FOG_COLOR, HexRgb::of(-16509389))
        .water(4566514)
}

pub fn warm_ocean() -> Biome {
    let mut m = Mobs::default();
    m.spawn(PUFFERFISH, 15, 1, 3)
        .spawn(NAUTILUS, 5, 1, 1)
        .warm_ocean_spawns(10, 4);
    let mut g = base_ocean_generation();
    g.features(
        VegetalDecoration,
        &[
            placed::WARM_OCEAN_VEGETATION,
            placed::SEAGRASS_WARM,
            placed::SEA_PICKLE,
        ],
    );
    base_ocean(m, g)
        .with(WATER_FOG_COLOR, HexRgb::of(-16507085))
        .water(4445678)
}

pub fn frozen_ocean(deep: bool) -> Biome {
    let mut m = Mobs::default();
    m.spawn(SQUID, 1, 1, 4)
        .spawn(SALMON, 15, 1, 5)
        .spawn(POLAR_BEAR, 1, 1, 2)
        .spawn(NAUTILUS, 2, 1, 1)
        .common_spawns()
        .spawn(DROWNED, 5, 1, 1);
    let mut g = Generation::default();
    icebergs(&mut g);
    overworld_features(&mut g);
    blue_ice(&mut g);
    ocean_vegetation(&mut g);
    base_biome(if deep { 0.5 } else { 0.0 }, 0.5, m, g)
        .temperature_modifier(TemperatureModifier::Frozen)
        .water(FROZEN_WATER_COLOR)
}

pub fn forest(birch: bool, tall: bool, flower: bool) -> Biome {
    let mut m = Mobs::default();
    m.farm_animals().common_spawns();
    let mut g = overworld_generation();
    let music = if flower {
        g.features(
            VegetalDecoration,
            &[
                placed::FLOWER_FOREST_FLOWERS,
                placed::TREES_FLOWER_FOREST,
                placed::FLOWER_FLOWER_FOREST,
            ],
        );
        default_grass(&mut g);
        m.spawn(RABBIT, 4, 2, 3);
        rl!("minecraft:music.overworld.flower_forest")
    } else {
        forest_flowers(&mut g);
        if !birch {
            other_birch_trees(&mut g);
            m.spawn(WOLF, 5, 4, 4);
        } else {
            birch_forest_flowers(&mut g);
            if tall {
                tall_birch_trees(&mut g);
            } else {
                birch_trees(&mut g);
            }
        }
        bushes(&mut g);
        default_flowers(&mut g);
        forest_grass(&mut g);
        FOREST_MUSIC
    };
    mushrooms_and_extra_vegetation(&mut g);
    let (temperature, downfall) = if birch { (0.6, 0.6) } else { (0.7, 0.8) };
    base_biome(temperature, downfall, m, g).music(music)
}

pub fn taiga(snowy: bool) -> Biome {
    let mut m = Mobs::default();
    m.taiga_animals().common_spawns();
    let mut g = overworld_generation();
    ferns(&mut g);
    taiga_trees(&mut g);
    default_flowers(&mut g);
    taiga_grass(&mut g);
    default_extra_vegetation(&mut g);
    let (temperature, downfall, water) = if snowy {
        rare_berry_bushes(&mut g);
        (-0.5, 0.4, COLD_WATER_COLOR)
    } else {
        common_berry_bushes(&mut g);
        (0.25, 0.8, Biome::NORMAL_WATER_COLOR)
    };
    base_biome(temperature, downfall, m, g).water(water)
}

pub fn dark_forest(pale_garden: bool) -> Biome {
    let mut m = Mobs::default();
    m.common_spawns();
    let mut g = overworld_generation();
    let biome = climate(0.7, 0.8);
    let biome = if pale_garden {
        g.features(
            VegetalDecoration,
            &[
                placed::PALE_GARDEN_VEGETATION,
                placed::PALE_MOSS_PATCH,
                placed::PALE_GARDEN_FLOWERS,
                placed::FLOWER_PALE_GARDEN,
            ],
        );
        forest_grass(&mut g);
        biome
            .with(SKY_COLOR, HexRgb::of(-4605511))
            .with(FOG_COLOR, HexRgb::of(-8292496))
            .with(WATER_FOG_COLOR, HexRgb::of(-11179648))
            .background_music(BackgroundMusic::default())
            .with(MUSIC_VOLUME, 0.0)
            .water(7768221)
            .foliage(8883574)
            .grass(7832178)
            .dry_foliage(10528412)
    } else {
        m.farm_animals();
        g.feature(VegetalDecoration, placed::DARK_FOREST_VEGETATION);
        forest_flowers(&mut g);
        default_flowers(&mut g);
        forest_grass(&mut g);
        default_mushrooms(&mut g);
        leaf_litter_patch(&mut g);
        biome
            .music(FOREST_MUSIC)
            .dry_foliage(DARK_DRY_FOLIAGE_COLOR)
            .grass_modifier(GrassColorModifier::DarkForest)
    };
    default_extra_vegetation(&mut g);
    biome.spawns(m.0).generation(g)
}

pub fn swamp() -> Biome {
    let mut m = Mobs::default();
    m.farm_animals().swamp_spawns(SWAMP_SKELETON_WEIGHT);
    let mut g = Generation::default();
    fossil_decoration(&mut g);
    global_overworld_generation(&mut g);
    default_ores(&mut g);
    swamp_clay_disk(&mut g);
    swamp_vegetation(&mut g);
    default_mushrooms(&mut g);
    swamp_extra_vegetation(&mut g);
    g.feature(VegetalDecoration, placed::SEAGRASS_SWAMP);
    swamp_biome(m, g)
        .with(WATER_FOG_COLOR, HexRgb::of(-14474473))
        .water(6388580)
        .foliage(6975545)
}

pub fn mangrove_swamp() -> Biome {
    let mut m = Mobs::default();
    m.swamp_spawns(SWAMP_SKELETON_WEIGHT)
        .spawn(TROPICAL_FISH, 25, 8, 8);
    let mut g = Generation::default();
    fossil_decoration(&mut g);
    global_overworld_generation(&mut g);
    default_ores(&mut g);
    mangrove_swamp_disks(&mut g);
    mangrove_swamp_vegetation(&mut g);
    mangrove_swamp_extra_vegetation(&mut g);
    swamp_biome(m, g)
        .with(FOG_COLOR, HexRgb::of(-4138753))
        .with(WATER_FOG_COLOR, HexRgb::of(-11699616))
        .water(3832426)
        .foliage(9285927)
}

pub fn river(frozen: bool) -> Biome {
    let mut m = Mobs::default();
    m.spawn(SQUID, 2, 1, 4)
        .spawn(SALMON, 5, 1, 5)
        .common_spawns();
    let mut g = overworld_generation();
    water_trees(&mut g);
    bushes(&mut g);
    default_vegetation(&mut g);
    let (temperature, water) = if frozen {
        m.spawn(DROWNED, 1, 1, 1);
        (0.0, FROZEN_WATER_COLOR)
    } else {
        m.spawn(DROWNED, 100, 1, 1);
        g.feature(VegetalDecoration, placed::SEAGRASS_RIVER);
        (0.5, Biome::NORMAL_WATER_COLOR)
    };
    base_biome(temperature, 0.5, m, g)
        .background_music(BackgroundMusic::overworld_with_underwater())
        .water(water)
}

pub fn beach(snowy: bool, stony: bool) -> Biome {
    let mut m = Mobs::default();
    let (temperature, downfall, water) = match (snowy, stony) {
        (true, _) => (0.05, 0.3, COLD_WATER_COLOR),
        (false, true) => (0.2, 0.3, Biome::NORMAL_WATER_COLOR),
        (false, false) => {
            m.spawn(TURTLE, 5, 2, 5);
            (0.8, 0.4, Biome::NORMAL_WATER_COLOR)
        }
    };
    m.common_spawns();
    let mut g = overworld_generation();
    default_vegetation(&mut g);
    base_biome(temperature, downfall, m, g).water(water)
}

pub fn the_void() -> Biome {
    let mut g = Generation::default();
    g.feature(TopLayerModification, placed::VOID_START_PLATFORM);
    base_biome(0.5, 0.5, Mobs::none(), g).precipitation(false)
}

pub fn meadow_or_cherry_grove(cherry_grove: bool) -> Biome {
    let mut m = Mobs::default();
    let mut g = overworld_generation();
    plain_grass(&mut g);
    let biome = climate(0.5, 0.8);
    let biome = if cherry_grove {
        m.spawn(PIG, 1, 1, 2);
        cherry_grove_vegetation(&mut g);
        biome
            .with(WATER_FOG_COLOR, HexRgb::of(-10635281))
            .music(rl!("minecraft:music.overworld.cherry_grove"))
            .water(6141935)
            .foliage(11983713)
            .grass(11983713)
    } else {
        m.spawn(DONKEY, 1, 1, 2);
        meadow_vegetation(&mut g);
        biome
            .music(rl!("minecraft:music.overworld.meadow"))
            .water(937679)
    };
    m.spawn(RABBIT, 2, 2, 6)
        .spawn(SHEEP, 2, 2, 4)
        .common_spawns();
    mountain_ores(&mut g);
    biome.spawns(m.0).generation(g)
}

pub fn dappled_forest() -> Biome {
    let mut g = overworld_generation();
    g.feature(VegetalDecoration, placed::TREES_DAPPLED_FOREST);
    dappled_forest_vegetation(&mut g);
    forest_grass(&mut g);
    let mut m = Mobs::default();
    m.farm_animals()
        .common_spawns()
        .spawn(RABBIT, 4, 2, 4)
        .spawn(FOX, 4, 2, 4);
    base_biome(0.6, 0.6, m, g)
        .music(FOREST_MUSIC)
        .with(SKY_COLOR, HexRgb::of(8168447))
        .with(FOG_COLOR, HexRgb::of(13424866))
        .with(WATER_FOG_COLOR, HexRgb::of(3625300))
        .water(3625300)
        .foliage(15109680)
        .grass(14641191)
        .dry_foliage(9189892)
}

pub fn peaks(sound: Id) -> Biome {
    let mut m = Mobs::default();
    m.spawn(GOAT, 5, 1, 3).common_spawns();
    let mut g = overworld_generation();
    frozen_springs(&mut g);
    mountain_ores(&mut g);
    base_biome(-0.7, 0.9, m, g)
        .with(INCREASED_FIRE_BURNOUT, true)
        .music(sound)
}

pub fn stony_peaks() -> Biome {
    let mut m = Mobs::default();
    m.common_spawns();
    let mut g = overworld_generation();
    mountain_ores(&mut g);
    base_biome(1.0, 0.3, m, g).music(rl!("minecraft:music.overworld.stony_peaks"))
}

pub fn snowy_slopes() -> Biome {
    let mut m = Mobs::default();
    m.spawn(RABBIT, 4, 2, 3)
        .spawn(GOAT, 5, 1, 3)
        .common_spawns();
    let mut g = overworld_generation();
    frozen_springs(&mut g);
    pumpkin_patches(&mut g);
    mountain_ores(&mut g);
    base_biome(-0.3, 0.9, m, g)
        .music(rl!("minecraft:music.overworld.snowy_slopes"))
        .with(INCREASED_FIRE_BURNOUT, true)
}

pub fn grove() -> Biome {
    let mut m = Mobs::default();
    m.spawn(WOLF, 1, 1, 1)
        .spawn(RABBIT, 8, 2, 3)
        .spawn(FOX, 4, 2, 4)
        .common_spawns();
    let mut g = overworld_generation();
    frozen_springs(&mut g);
    grove_trees(&mut g);
    pumpkin_patches(&mut g);
    mountain_ores(&mut g);
    base_biome(-0.2, 0.8, m, g).music(rl!("minecraft:music.overworld.grove"))
}

pub fn sulfur_caves() -> Biome {
    let mut m = Mobs::default();
    m.spawn(BAT, 10, 8, 8)
        .spawn(SULFUR_CUBE, 100, 2, 4)
        .spawn(CREEPER, 50, 2, 2)
        .spawn(SKELETON, 50, 2, 2)
        .spawn(SLIME, 25, 1, 1)
        .spawn(CAVE_SPIDER, 20, 1, 1)
        .spawn(ZOMBIE, 50, 2, 2)
        .spawn(ENDERMAN, 10, 1, 1)
        .spawn(WITCH, 1, 1, 1)
        .spawn(ZOMBIE_VILLAGER, 5, 1, 1);
    let mut g = overworld_generation();
    plain_grass(&mut g);
    sulfur_caves_features(&mut g);
    base_biome(0.8, 0.4, m, g)
        .with(FOG_COLOR, HexRgb::of(-7555023))
        .music(rl!("minecraft:music.overworld.sulfur_caves"))
        .with(WATER_FOG_COLOR, HexRgb::of(-15248324))
        .water(-13320311)
        .grass(11249231)
}

pub fn lush_caves() -> Biome {
    let mut m = Mobs::default();
    m.spawn(AXOLOTL, 10, 4, 6)
        .spawn(TROPICAL_FISH, 25, 8, 8)
        .common_spawns();
    let mut g = Generation::default();
    global_overworld_generation(&mut g);
    plain_grass(&mut g);
    default_ores(&mut g);
    lush_caves_special_ores(&mut g);
    default_soft_disks(&mut g);
    lush_caves_vegetation_features(&mut g);
    base_biome(0.5, 0.5, m, g).music(rl!("minecraft:music.overworld.lush_caves"))
}

pub fn dripstone_caves() -> Biome {
    let mut m = Mobs::default();
    m.dripstone_caves_spawns();
    let mut g = Generation::default();
    global_overworld_generation(&mut g);
    default_ores_with_large_copper_blobs(&mut g);
    default_soft_disks(&mut g);
    cave_plains_vegetation(&mut g);
    dripstone(&mut g);
    base_biome(0.8, 0.4, m, g).music(rl!("minecraft:music.overworld.dripstone_caves"))
}

pub fn deep_dark() -> Biome {
    let mut g = Generation::default();
    default_carvers(&mut g);
    default_crystal_formations(&mut g);
    default_monster_room(&mut g);
    default_underground_variety(&mut g);
    surface_freezing(&mut g);
    default_ores(&mut g);
    default_soft_disks(&mut g);
    cave_plains_vegetation(&mut g);
    sculk(&mut g);
    base_biome(0.8, 0.4, Mobs::none(), g).music(rl!("minecraft:music.overworld.deep_dark"))
}
