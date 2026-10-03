use super::defaults::*;
use super::mob::*;
use super::*;
use mcrs_minecraft_biome::TemperatureModifier;
use mcrs_minecraft_worldgen_structure::DecorationStep::*;

const DARK_DRY_FOLIAGE_COLOR: i32 = 8082228;
const SWAMP_SKELETON_WEIGHT: i32 = 70;

fn sky_color(temperature: f32) -> i32 {
    let temp = (temperature / 3.0).clamp(-1.0, 1.0);
    mcrs_minecraft_core::mth::hsv_to_rgb(0.62222224 - temp * 0.05, 0.5 + temp * 0.1, 1.0)
}

fn base_biome(temperature: f32, downfall: f32, mobs: Mobs, generation: Generation) -> Biome {
    biome(true, temperature, downfall, mobs, generation)
        .with(SKY_COLOR, HexRgb::of(sky_color(temperature)))
}

fn global_overworld_generation(g: &mut Generation) {
    default_carvers_and_lakes(g);
    default_crystal_formations(g);
    default_monster_room(g);
    default_underground_variety(g);
    default_springs(g);
    surface_freezing(g);
}

pub fn old_growth_taiga(spruce: bool) -> Biome {
    let mut m = Mobs::default();
    farm_animals(&mut m);
    m.spawn(WOLF, 8, 4, 4)
        .spawn(RABBIT, 4, 2, 3)
        .spawn(FOX, 8, 2, 4);
    if spruce {
        common_spawns(&mut m);
    } else {
        cave_spawns(&mut m);
        monsters(&mut m, 100, 25, 0, 100);
    }
    let mut g = Generation::default();
    global_overworld_generation(&mut g);
    mossy_stone_block(&mut g);
    ferns(&mut g);
    default_ores(&mut g);
    default_soft_disks(&mut g);
    g.feature(
        VegetalDecoration,
        if spruce {
            placed::TREES_OLD_GROWTH_SPRUCE_TAIGA
        } else {
            placed::TREES_OLD_GROWTH_PINE_TAIGA
        },
    );
    default_flowers(&mut g);
    giant_taiga_vegetation(&mut g);
    default_mushrooms(&mut g);
    default_extra_vegetation(&mut g, true);
    common_berry_bushes(&mut g);
    base_biome(if spruce { 0.25 } else { 0.3 }, 0.8, m, g)
        .music(rl!("minecraft:music.overworld.old_growth_taiga"))
}

pub fn sparse_jungle() -> Biome {
    let mut m = Mobs::default();
    base_jungle_spawns(&mut m);
    m.spawn(WOLF, 8, 2, 4);
    base_jungle(0.8, false, true, false, m).music(rl!("minecraft:music.overworld.sparse_jungle"))
}

pub fn jungle() -> Biome {
    let mut m = Mobs::default();
    base_jungle_spawns(&mut m);
    m.spawn(PARROT, 40, 1, 2)
        .spawn_as(OCELOT, MobCategory::Monster, 2, 1, 3)
        .spawn(PANDA, 1, 1, 2);
    base_jungle(0.9, false, false, true, m)
        .music(rl!("minecraft:music.overworld.jungle"))
        .with(INCREASED_FIRE_BURNOUT, true)
}

pub fn bamboo_jungle() -> Biome {
    let mut m = Mobs::default();
    base_jungle_spawns(&mut m);
    m.spawn(PARROT, 40, 1, 2).spawn(PANDA, 80, 1, 2).spawn_as(
        OCELOT,
        MobCategory::Monster,
        2,
        1,
        1,
    );
    base_jungle(0.9, true, false, true, m)
        .music(rl!("minecraft:music.overworld.bamboo_jungle"))
        .with(INCREASED_FIRE_BURNOUT, true)
}

fn base_jungle(downfall: f32, bamboo: bool, sparse: bool, core: bool, mobs: Mobs) -> Biome {
    let mut g = Generation::default();
    global_overworld_generation(&mut g);
    default_ores(&mut g);
    default_soft_disks(&mut g);
    if bamboo {
        bamboo_vegetation(&mut g);
    } else {
        if core {
            light_bamboo_vegetation(&mut g);
        }
        if sparse {
            sparse_jungle_trees(&mut g);
        } else {
            jungle_trees(&mut g);
        }
    }
    warm_flowers(&mut g);
    jungle_grass(&mut g);
    default_mushrooms(&mut g);
    default_extra_vegetation(&mut g, true);
    jungle_vines(&mut g);
    if sparse {
        sparse_jungle_melons(&mut g);
    } else {
        jungle_melons(&mut g);
    }
    base_biome(0.95, downfall, mobs, g)
}

pub fn windswept_hills(more_trees: bool) -> Biome {
    let mut m = Mobs::default();
    farm_animals(&mut m);
    m.spawn(LLAMA, 5, 4, 6);
    common_spawns(&mut m);
    let mut g = Generation::default();
    global_overworld_generation(&mut g);
    default_ores(&mut g);
    default_soft_disks(&mut g);
    if more_trees {
        mountain_forest_trees(&mut g);
    } else {
        mountain_trees(&mut g);
    }
    bushes(&mut g);
    default_flowers(&mut g);
    default_grass(&mut g);
    default_mushrooms(&mut g);
    default_extra_vegetation(&mut g, true);
    extra_emeralds(&mut g);
    infested_stone(&mut g);
    base_biome(0.2, 0.3, m, g)
}

pub fn desert() -> Biome {
    let mut m = Mobs::default();
    desert_spawns(&mut m);
    let mut g = Generation::default();
    fossil_decoration(&mut g);
    global_overworld_generation(&mut g);
    default_ores(&mut g);
    default_soft_disks(&mut g);
    default_flowers(&mut g);
    default_grass(&mut g);
    desert_vegetation(&mut g);
    default_mushrooms(&mut g);
    desert_extra_vegetation(&mut g);
    desert_extra_decoration(&mut g);
    base_biome(2.0, 0.0, m, g)
        .precipitation(false)
        .music(rl!("minecraft:music.overworld.desert"))
        .with(SNOW_GOLEM_MELTS, true)
}

pub fn plains(sunflower: bool, snowy: bool, spikes: bool) -> Biome {
    let mut m = Mobs::default();
    let mut g = Generation::default();
    global_overworld_generation(&mut g);
    if snowy {
        snowy_spawns(&mut m, !spikes);
        if spikes {
            g.features(SurfaceStructures, &[placed::ICE_SPIKE, placed::ICE_PATCH]);
        }
    } else {
        plains_spawns(&mut m);
        plain_grass(&mut g);
        if sunflower {
            g.feature(VegetalDecoration, placed::PATCH_SUNFLOWER);
        } else {
            bushes(&mut g);
        }
    }
    default_ores(&mut g);
    default_soft_disks(&mut g);
    if snowy {
        snowy_trees(&mut g);
        default_flowers(&mut g);
        default_grass(&mut g);
    } else {
        plain_vegetation(&mut g);
    }
    default_mushrooms(&mut g);
    default_extra_vegetation(&mut g, true);
    let mut biome = if snowy {
        base_biome(0.0, 0.5, m, g)
    } else {
        base_biome(0.8, 0.4, m, g)
    };
    if snowy {
        biome = biome.with(CREATURE_WORLD_GEN_SPAWN_PROBABILITY, 0.07f32);
    }
    biome
}

pub fn mushroom_fields() -> Biome {
    let mut m = Mobs::default();
    mooshroom_spawns(&mut m);
    let mut g = Generation::default();
    global_overworld_generation(&mut g);
    default_ores(&mut g);
    default_soft_disks(&mut g);
    mushroom_field_vegetation(&mut g);
    near_water_vegetation(&mut g);
    base_biome(0.9, 1.0, m, g)
        .with(INCREASED_FIRE_BURNOUT, true)
        .with(CAN_PILLAGER_PATROL_SPAWN, false)
}

pub fn savanna(shattered: bool, plateau: bool) -> Biome {
    let mut g = Generation::default();
    global_overworld_generation(&mut g);
    if !shattered {
        savanna_grass(&mut g);
    }
    default_ores(&mut g);
    default_soft_disks(&mut g);
    if shattered {
        shattered_savanna_trees(&mut g);
        default_flowers(&mut g);
        shattered_savanna_grass(&mut g);
    } else {
        savanna_trees(&mut g);
        warm_flowers(&mut g);
        savanna_extra_grass(&mut g);
    }
    default_mushrooms(&mut g);
    default_extra_vegetation(&mut g, true);
    let mut m = Mobs::default();
    farm_animals(&mut m);
    m.spawn(HORSE, 1, 2, 6)
        .spawn(DONKEY, 1, 1, 1)
        .spawn(ARMADILLO, 10, 2, 3);
    common_spawn_with_zombie_horse(&mut m);
    if plateau {
        m.spawn(LLAMA, 8, 4, 4).spawn(WOLF, 8, 4, 8);
    }
    base_biome(2.0, 0.0, m, g)
        .precipitation(false)
        .with(SNOW_GOLEM_MELTS, true)
}

pub fn badlands(wooded: bool) -> Biome {
    let mut m = Mobs::default();
    farm_animals(&mut m);
    common_spawns(&mut m);
    m.spawn(ARMADILLO, 6, 1, 2);
    if wooded {
        m.spawn(WOLF, 2, 4, 8);
    }
    let mut g = Generation::default();
    global_overworld_generation(&mut g);
    default_ores(&mut g);
    extra_gold(&mut g);
    default_soft_disks(&mut g);
    if wooded {
        badlands_trees(&mut g);
    }
    badland_grass(&mut g);
    default_mushrooms(&mut g);
    badland_extra_vegetation(&mut g);
    base_biome(2.0, 0.0, m, g)
        .precipitation(false)
        .with(
            CREATURE_WORLD_GEN_SPAWN_PROBABILITY,
            if wooded { 0.04f32 } else { 0.03 },
        )
        .music(rl!("minecraft:music.overworld.badlands"))
        .with(SNOW_GOLEM_MELTS, true)
        .effects(
            BiomeEffects::water(NORMAL_WATER_COLOR)
                .foliage(10387789)
                .grass(9470285),
        )
}

fn base_ocean(mobs: Mobs, generation: Generation) -> Biome {
    base_biome(0.5, 0.5, mobs, generation).with(
        BACKGROUND_MUSIC,
        BackgroundMusic::overworld_with_underwater(),
    )
}

fn base_ocean_generation() -> Generation {
    let mut g = Generation::default();
    global_overworld_generation(&mut g);
    default_ores(&mut g);
    default_soft_disks(&mut g);
    water_trees(&mut g);
    default_flowers(&mut g);
    default_grass(&mut g);
    default_mushrooms(&mut g);
    default_extra_vegetation(&mut g, true);
    g
}

pub fn cold_ocean(deep: bool) -> Biome {
    let mut m = Mobs::default();
    ocean_spawns(&mut m, 3, 4, 15);
    m.spawn(SALMON, 15, 1, 5).spawn(NAUTILUS, 2, 1, 1);
    let mut g = base_ocean_generation();
    g.feature(
        VegetalDecoration,
        if deep {
            placed::SEAGRASS_DEEP_COLD
        } else {
            placed::SEAGRASS_COLD
        },
    );
    cold_ocean_extra_vegetation(&mut g);
    base_ocean(m, g).effects(BiomeEffects::water(4020182))
}

pub fn ocean(deep: bool) -> Biome {
    let mut m = Mobs::default();
    ocean_spawns(&mut m, 1, 4, 10);
    m.spawn(DOLPHIN, 1, 1, 2).spawn(NAUTILUS, 5, 1, 1);
    let mut g = base_ocean_generation();
    g.feature(
        VegetalDecoration,
        if deep {
            placed::SEAGRASS_DEEP
        } else {
            placed::SEAGRASS_NORMAL
        },
    );
    cold_ocean_extra_vegetation(&mut g);
    base_ocean(m, g)
}

pub fn lukewarm_ocean(deep: bool) -> Biome {
    let mut m = Mobs::default();
    if deep {
        ocean_spawns(&mut m, 8, 4, 8);
    } else {
        ocean_spawns(&mut m, 10, 2, 15);
    }
    m.spawn(PUFFERFISH, 5, 1, 3)
        .spawn(TROPICAL_FISH, 25, 8, 8)
        .spawn(DOLPHIN, 2, 1, 2)
        .spawn(NAUTILUS, 5, 1, 1);
    let mut g = base_ocean_generation();
    g.feature(
        VegetalDecoration,
        if deep {
            placed::SEAGRASS_DEEP_WARM
        } else {
            placed::SEAGRASS_WARM
        },
    );
    lukewarm_kelp(&mut g);
    base_ocean(m, g)
        .with(WATER_FOG_COLOR, HexRgb::of(-16509389))
        .effects(BiomeEffects::water(4566514))
}

pub fn warm_ocean() -> Biome {
    let mut m = Mobs::default();
    m.spawn(PUFFERFISH, 15, 1, 3).spawn(NAUTILUS, 5, 1, 1);
    warm_ocean_spawns(&mut m, 10, 4);
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
        .effects(BiomeEffects::water(4445678))
}

pub fn frozen_ocean(deep: bool) -> Biome {
    let mut m = Mobs::default();
    m.spawn(SQUID, 1, 1, 4)
        .spawn(SALMON, 15, 1, 5)
        .spawn(POLAR_BEAR, 1, 1, 2)
        .spawn(NAUTILUS, 2, 1, 1);
    common_spawns(&mut m);
    m.spawn(DROWNED, 5, 1, 1);
    let mut g = Generation::default();
    icebergs(&mut g);
    global_overworld_generation(&mut g);
    blue_ice(&mut g);
    default_ores(&mut g);
    default_soft_disks(&mut g);
    water_trees(&mut g);
    default_flowers(&mut g);
    default_grass(&mut g);
    default_mushrooms(&mut g);
    default_extra_vegetation(&mut g, true);
    base_biome(if deep { 0.5 } else { 0.0 }, 0.5, m, g)
        .temperature_modifier(TemperatureModifier::Frozen)
        .effects(BiomeEffects::water(3750089))
}

pub fn forest(birch: bool, tall: bool, flower: bool) -> Biome {
    let mut g = Generation::default();
    global_overworld_generation(&mut g);
    if flower {
        g.feature(VegetalDecoration, placed::FLOWER_FOREST_FLOWERS);
    } else {
        forest_flowers(&mut g);
    }
    default_ores(&mut g);
    default_soft_disks(&mut g);
    if flower {
        g.features(
            VegetalDecoration,
            &[placed::TREES_FLOWER_FOREST, placed::FLOWER_FLOWER_FOREST],
        );
        default_grass(&mut g);
    } else {
        if birch {
            birch_forest_flowers(&mut g);
            if tall {
                tall_birch_trees(&mut g);
            } else {
                birch_trees(&mut g);
            }
        } else {
            other_birch_trees(&mut g);
        }
        bushes(&mut g);
        default_flowers(&mut g);
        forest_grass(&mut g);
    }
    default_mushrooms(&mut g);
    default_extra_vegetation(&mut g, true);
    let mut m = Mobs::default();
    farm_animals(&mut m);
    common_spawns(&mut m);
    if flower {
        m.spawn(RABBIT, 4, 2, 3);
    } else if !birch {
        m.spawn(WOLF, 5, 4, 4);
    }
    if birch {
        base_biome(0.6, 0.6, m, g)
    } else {
        base_biome(0.7, 0.8, m, g)
    }
    .music(if flower {
        rl!("minecraft:music.overworld.flower_forest")
    } else {
        rl!("minecraft:music.overworld.forest")
    })
}

pub fn taiga(snowy: bool) -> Biome {
    let mut m = Mobs::default();
    farm_animals(&mut m);
    m.spawn(WOLF, 8, 4, 4)
        .spawn(RABBIT, 4, 2, 3)
        .spawn(FOX, 8, 2, 4);
    common_spawns(&mut m);
    let mut g = Generation::default();
    global_overworld_generation(&mut g);
    ferns(&mut g);
    default_ores(&mut g);
    default_soft_disks(&mut g);
    taiga_trees(&mut g);
    default_flowers(&mut g);
    taiga_grass(&mut g);
    default_extra_vegetation(&mut g, true);
    if snowy {
        rare_berry_bushes(&mut g);
    } else {
        common_berry_bushes(&mut g);
    }
    if snowy {
        base_biome(-0.5, 0.4, m, g)
    } else {
        base_biome(0.25, 0.8, m, g)
    }
    .effects(BiomeEffects::water(if snowy {
        4020182
    } else {
        NORMAL_WATER_COLOR
    }))
}

pub fn dark_forest(pale_garden: bool) -> Biome {
    let mut m = Mobs::default();
    if !pale_garden {
        farm_animals(&mut m);
    }
    common_spawns(&mut m);
    let mut g = Generation::default();
    global_overworld_generation(&mut g);
    g.feature(
        VegetalDecoration,
        if pale_garden {
            placed::PALE_GARDEN_VEGETATION
        } else {
            placed::DARK_FOREST_VEGETATION
        },
    );
    if pale_garden {
        g.features(
            VegetalDecoration,
            &[placed::PALE_MOSS_PATCH, placed::PALE_GARDEN_FLOWERS],
        );
    } else {
        forest_flowers(&mut g);
    }
    default_ores(&mut g);
    default_soft_disks(&mut g);
    if pale_garden {
        g.feature(VegetalDecoration, placed::FLOWER_PALE_GARDEN);
    } else {
        default_flowers(&mut g);
    }
    forest_grass(&mut g);
    if !pale_garden {
        default_mushrooms(&mut g);
        leaf_litter_patch(&mut g);
    }
    default_extra_vegetation(&mut g, true);
    let mut biome = base_biome(0.7, 0.8, m, g);
    if pale_garden {
        biome = biome.with(SKY_COLOR, HexRgb::of(-4605511));
        biome = biome.with(FOG_COLOR, HexRgb::of(-8292496));
        biome = biome.with(WATER_FOG_COLOR, HexRgb::of(-11179648));
        biome = biome.with(BACKGROUND_MUSIC, BackgroundMusic::default());
        biome = biome.with(MUSIC_VOLUME, 0.0f32);
        biome = biome.effects(
            BiomeEffects::water(7768221)
                .foliage(8883574)
                .grass(7832178)
                .dry_foliage(10528412),
        );
    } else {
        biome = biome.music(rl!("minecraft:music.overworld.forest"));
        biome = biome.effects(
            BiomeEffects::water(NORMAL_WATER_COLOR)
                .dry_foliage(DARK_DRY_FOLIAGE_COLOR)
                .grass_modifier(GrassColorModifier::DarkForest),
        );
    }
    biome
}

pub fn swamp() -> Biome {
    let mut m = Mobs::default();
    farm_animals(&mut m);
    swamp_spawns(&mut m, SWAMP_SKELETON_WEIGHT);
    let mut g = Generation::default();
    fossil_decoration(&mut g);
    global_overworld_generation(&mut g);
    default_ores(&mut g);
    swamp_clay_disk(&mut g);
    swamp_vegetation(&mut g);
    default_mushrooms(&mut g);
    swamp_extra_vegetation(&mut g);
    g.feature(VegetalDecoration, placed::SEAGRASS_SWAMP);
    base_biome(0.8, 0.9, m, g)
        .with(WATER_FOG_COLOR, HexRgb::of(-14474473))
        .modified(WATER_FOG_END_DISTANCE, Operation::Multiply, 0.85f32)
        .music(rl!("minecraft:music.overworld.swamp"))
        .with(INCREASED_FIRE_BURNOUT, true)
        .effects(
            BiomeEffects::water(6388580)
                .foliage(6975545)
                .dry_foliage(DARK_DRY_FOLIAGE_COLOR)
                .grass_modifier(GrassColorModifier::Swamp),
        )
}

pub fn mangrove_swamp() -> Biome {
    let mut m = Mobs::default();
    swamp_spawns(&mut m, SWAMP_SKELETON_WEIGHT);
    m.spawn(TROPICAL_FISH, 25, 8, 8);
    let mut g = Generation::default();
    fossil_decoration(&mut g);
    global_overworld_generation(&mut g);
    default_ores(&mut g);
    mangrove_swamp_disks(&mut g);
    mangrove_swamp_vegetation(&mut g);
    mangrove_swamp_extra_vegetation(&mut g);
    base_biome(0.8, 0.9, m, g)
        .with(FOG_COLOR, HexRgb::of(-4138753))
        .with(WATER_FOG_COLOR, HexRgb::of(-11699616))
        .modified(WATER_FOG_END_DISTANCE, Operation::Multiply, 0.85f32)
        .music(rl!("minecraft:music.overworld.swamp"))
        .with(INCREASED_FIRE_BURNOUT, true)
        .effects(
            BiomeEffects::water(3832426)
                .foliage(9285927)
                .dry_foliage(DARK_DRY_FOLIAGE_COLOR)
                .grass_modifier(GrassColorModifier::Swamp),
        )
}

pub fn river(frozen: bool) -> Biome {
    let mut m = Mobs::default();
    m.spawn(SQUID, 2, 1, 4).spawn(SALMON, 5, 1, 5);
    common_spawns(&mut m);
    m.spawn(DROWNED, if frozen { 1 } else { 100 }, 1, 1);
    let mut g = Generation::default();
    global_overworld_generation(&mut g);
    default_ores(&mut g);
    default_soft_disks(&mut g);
    water_trees(&mut g);
    bushes(&mut g);
    default_flowers(&mut g);
    default_grass(&mut g);
    default_mushrooms(&mut g);
    default_extra_vegetation(&mut g, true);
    if !frozen {
        g.feature(VegetalDecoration, placed::SEAGRASS_RIVER);
    }
    base_biome(if frozen { 0.0 } else { 0.5 }, 0.5, m, g)
        .with(
            BACKGROUND_MUSIC,
            BackgroundMusic::overworld_with_underwater(),
        )
        .effects(BiomeEffects::water(if frozen {
            3750089
        } else {
            NORMAL_WATER_COLOR
        }))
}

pub fn beach(snowy: bool, stony: bool) -> Biome {
    let mut m = Mobs::default();
    let sandy = !stony && !snowy;
    if sandy {
        m.spawn(TURTLE, 5, 2, 5);
    }
    common_spawns(&mut m);
    let mut g = Generation::default();
    global_overworld_generation(&mut g);
    default_ores(&mut g);
    default_soft_disks(&mut g);
    default_flowers(&mut g);
    default_grass(&mut g);
    default_mushrooms(&mut g);
    default_extra_vegetation(&mut g, true);
    let temperature = if snowy {
        0.05
    } else if stony {
        0.2
    } else {
        0.8
    };
    base_biome(temperature, if sandy { 0.4 } else { 0.3 }, m, g).effects(BiomeEffects::water(
        if snowy { 4020182 } else { NORMAL_WATER_COLOR },
    ))
}

pub fn the_void() -> Biome {
    let mut g = Generation::default();
    g.feature(TopLayerModification, placed::VOID_START_PLATFORM);
    base_biome(0.5, 0.5, Mobs::none(), g).precipitation(false)
}

pub fn meadow_or_cherry_grove(cherry_grove: bool) -> Biome {
    let mut g = Generation::default();
    let mut m = Mobs::default();
    m.spawn(if cherry_grove { PIG } else { DONKEY }, 1, 1, 2)
        .spawn(RABBIT, 2, 2, 6)
        .spawn(SHEEP, 2, 2, 4);
    common_spawns(&mut m);
    global_overworld_generation(&mut g);
    plain_grass(&mut g);
    default_ores(&mut g);
    default_soft_disks(&mut g);
    if cherry_grove {
        cherry_grove_vegetation(&mut g);
    } else {
        meadow_vegetation(&mut g);
    }
    extra_emeralds(&mut g);
    infested_stone(&mut g);
    let mut biome = base_biome(0.5, 0.8, m, g);
    if cherry_grove {
        biome = biome.with(WATER_FOG_COLOR, HexRgb::of(-10635281));
        biome = biome.music(rl!("minecraft:music.overworld.cherry_grove"));
        biome = biome.effects(
            BiomeEffects::water(6141935)
                .foliage(11983713)
                .grass(11983713),
        );
    } else {
        biome = biome.music(rl!("minecraft:music.overworld.meadow"));
        biome = biome.effects(BiomeEffects::water(937679));
    }
    biome
}

pub fn dappled_forest() -> Biome {
    let mut g = Generation::default();
    global_overworld_generation(&mut g);
    default_ores(&mut g);
    default_soft_disks(&mut g);
    g.feature(VegetalDecoration, placed::TREES_DAPPLED_FOREST);
    dappled_forest_vegetation(&mut g);
    forest_grass(&mut g);
    let mut m = Mobs::default();
    farm_animals(&mut m);
    common_spawns(&mut m);
    m.spawn(RABBIT, 4, 2, 4).spawn(FOX, 4, 2, 4);
    base_biome(0.6, 0.6, m, g)
        .music(rl!("minecraft:music.overworld.forest"))
        .with(SKY_COLOR, HexRgb::of(8168447))
        .with(FOG_COLOR, HexRgb::of(13424866))
        .with(WATER_FOG_COLOR, HexRgb::of(3625300))
        .effects(
            BiomeEffects::water(3625300)
                .foliage(15109680)
                .grass(14641191)
                .dry_foliage(9189892),
        )
}

pub fn peaks(sound: Id) -> Biome {
    let mut g = Generation::default();
    let mut m = Mobs::default();
    m.spawn(GOAT, 5, 1, 3);
    common_spawns(&mut m);
    global_overworld_generation(&mut g);
    frozen_springs(&mut g);
    default_ores(&mut g);
    default_soft_disks(&mut g);
    extra_emeralds(&mut g);
    infested_stone(&mut g);
    base_biome(-0.7, 0.9, m, g)
        .with(INCREASED_FIRE_BURNOUT, true)
        .music(sound)
}

pub fn stony_peaks() -> Biome {
    let mut g = Generation::default();
    let mut m = Mobs::default();
    common_spawns(&mut m);
    global_overworld_generation(&mut g);
    default_ores(&mut g);
    default_soft_disks(&mut g);
    extra_emeralds(&mut g);
    infested_stone(&mut g);
    base_biome(1.0, 0.3, m, g).music(rl!("minecraft:music.overworld.stony_peaks"))
}

pub fn snowy_slopes() -> Biome {
    let mut g = Generation::default();
    let mut m = Mobs::default();
    m.spawn(RABBIT, 4, 2, 3).spawn(GOAT, 5, 1, 3);
    common_spawns(&mut m);
    global_overworld_generation(&mut g);
    frozen_springs(&mut g);
    default_ores(&mut g);
    default_soft_disks(&mut g);
    default_extra_vegetation(&mut g, false);
    extra_emeralds(&mut g);
    infested_stone(&mut g);
    base_biome(-0.3, 0.9, m, g)
        .music(rl!("minecraft:music.overworld.snowy_slopes"))
        .with(INCREASED_FIRE_BURNOUT, true)
}

pub fn grove() -> Biome {
    let mut g = Generation::default();
    let mut m = Mobs::default();
    m.spawn(WOLF, 1, 1, 1)
        .spawn(RABBIT, 8, 2, 3)
        .spawn(FOX, 4, 2, 4);
    common_spawns(&mut m);
    global_overworld_generation(&mut g);
    frozen_springs(&mut g);
    default_ores(&mut g);
    default_soft_disks(&mut g);
    grove_trees(&mut g);
    default_extra_vegetation(&mut g, false);
    extra_emeralds(&mut g);
    infested_stone(&mut g);
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
    let mut g = Generation::default();
    global_overworld_generation(&mut g);
    plain_grass(&mut g);
    default_ores(&mut g);
    default_soft_disks(&mut g);
    sulfur_caves_features(&mut g);
    base_biome(0.8, 0.4, m, g)
        .with(FOG_COLOR, HexRgb::of(-7555023))
        .music(rl!("minecraft:music.overworld.sulfur_caves"))
        .with(WATER_FOG_COLOR, HexRgb::of(-15248324))
        .effects(BiomeEffects::water(-13320311).grass(11249231))
}

pub fn lush_caves() -> Biome {
    let mut m = Mobs::default();
    m.spawn(AXOLOTL, 10, 4, 6).spawn(TROPICAL_FISH, 25, 8, 8);
    common_spawns(&mut m);
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
    dripstone_caves_spawns(&mut m);
    let mut g = Generation::default();
    global_overworld_generation(&mut g);
    plain_grass(&mut g);
    default_ores_with_large_copper_blobs(&mut g);
    default_soft_disks(&mut g);
    plain_vegetation(&mut g);
    default_mushrooms(&mut g);
    default_extra_vegetation(&mut g, false);
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
    plain_grass(&mut g);
    default_ores(&mut g);
    default_soft_disks(&mut g);
    plain_vegetation(&mut g);
    default_mushrooms(&mut g);
    default_extra_vegetation(&mut g, false);
    sculk(&mut g);
    base_biome(0.8, 0.4, Mobs::none(), g).music(rl!("minecraft:music.overworld.deep_dark"))
}
