use super::mob::*;
use super::{Generation, Mobs, carver, placed_feature};
use mcrs_minecraft_biome_file::PlacedFeatureKey;
use mcrs_minecraft_worldgen_feature::placement::DecorationStep::*;

macro_rules! feature_sets {
    ($($name:ident => $($step:ident [$($key:ident),+])+;)*) => {$(
        pub fn $name(g: &mut Generation) {
            $(g.features($step, &[$(placed_feature::$key),+]);)+
        }
    )*};
}

#[rustfmt::skip]
feature_sets! {
    default_monster_room => UndergroundStructures [MONSTER_ROOM, MONSTER_ROOM_DEEP];
    sculk => UndergroundDecoration [SCULK_VEIN, SCULK_PATCH_DEEP_DARK];
    extra_gold => UndergroundOres [ORE_GOLD_EXTRA];
    extra_emeralds => UndergroundOres [ORE_EMERALD];
    infested_stone => UndergroundDecoration [ORE_INFESTED];
    default_soft_disks => UndergroundOres [DISK_SAND, DISK_CLAY, DISK_GRAVEL];
    swamp_clay_disk => UndergroundOres [DISK_CLAY];
    mangrove_swamp_disks => UndergroundOres [DISK_GRASS, DISK_CLAY];
    mossy_stone_block => LocalModifications [FOREST_ROCK];
    ferns => VegetalDecoration [PATCH_LARGE_FERN];
    bushes => VegetalDecoration [PATCH_BUSH];
    rare_berry_bushes => VegetalDecoration [PATCH_BERRY_RARE];
    common_berry_bushes => VegetalDecoration [PATCH_BERRY_COMMON];
    light_bamboo_vegetation => VegetalDecoration [BAMBOO_LIGHT];
    bamboo_vegetation => VegetalDecoration [BAMBOO, BAMBOO_VEGETATION];
    taiga_trees => VegetalDecoration [TREES_TAIGA];
    grove_trees => VegetalDecoration [TREES_GROVE];
    water_trees => VegetalDecoration [TREES_WATER];
    birch_trees => VegetalDecoration [TREES_BIRCH];
    other_birch_trees => VegetalDecoration [TREES_BIRCH_AND_OAK_LEAF_LITTER];
    tall_birch_trees => VegetalDecoration [BIRCH_TALL];
    birch_forest_flowers => VegetalDecoration [WILDFLOWERS_BIRCH_FOREST];
    savanna_trees => VegetalDecoration [TREES_SAVANNA];
    shattered_savanna_trees => VegetalDecoration [TREES_WINDSWEPT_SAVANNA];
    lush_caves_vegetation_features => VegetalDecoration [LUSH_CAVES_CEILING_VEGETATION, CAVE_VINES, LUSH_CAVES_CLAY, LUSH_CAVES_VEGETATION, ROOTED_AZALEA_TREE, SPORE_BLOSSOM, CLASSIC_VINES_CAVE_FEATURE];
    lush_caves_special_ores => UndergroundOres [ORE_CLAY];
    mountain_trees => VegetalDecoration [TREES_WINDSWEPT_HILLS];
    mountain_forest_trees => VegetalDecoration [TREES_WINDSWEPT_FOREST];
    badlands_trees => VegetalDecoration [TREES_BADLANDS];
    snowy_trees => VegetalDecoration [TREES_SNOWY];
    jungle_grass => VegetalDecoration [PATCH_GRASS_JUNGLE];
    savanna_grass => VegetalDecoration [PATCH_TALL_GRASS];
    shattered_savanna_grass => VegetalDecoration [PATCH_GRASS_NORMAL];
    savanna_extra_grass => VegetalDecoration [PATCH_GRASS_SAVANNA];
    badland_grass => VegetalDecoration [PATCH_GRASS_BADLANDS, PATCH_DRY_GRASS_BADLANDS, PATCH_DEAD_BUSH_BADLANDS];
    forest_flowers => VegetalDecoration [FOREST_FLOWERS];
    forest_grass => VegetalDecoration [PATCH_GRASS_FOREST];
    swamp_vegetation => VegetalDecoration [TREES_SWAMP, FLOWER_SWAMP, PATCH_GRASS_NORMAL, PATCH_DEAD_BUSH, PATCH_WATERLILY, BROWN_MUSHROOM_SWAMP, RED_MUSHROOM_SWAMP];
    dappled_forest_vegetation => VegetalDecoration [BROWN_MUSHROOM_DAPPLED_FOREST, PATCH_RED_SHRUB];
    mangrove_swamp_vegetation => VegetalDecoration [TREES_MANGROVE, PATCH_GRASS_NORMAL, PATCH_DEAD_BUSH, PATCH_WATERLILY];
    mushroom_field_vegetation => VegetalDecoration [MUSHROOM_ISLAND_VEGETATION, BROWN_MUSHROOM_TAIGA, RED_MUSHROOM_TAIGA];
    plain_vegetation => VegetalDecoration [TREES_PLAINS, FLOWER_PLAINS, PATCH_GRASS_PLAIN];
    desert_vegetation => VegetalDecoration [PATCH_DRY_GRASS_DESERT, PATCH_DEAD_BUSH_2];
    giant_taiga_vegetation => VegetalDecoration [PATCH_GRASS_TAIGA, PATCH_DEAD_BUSH, BROWN_MUSHROOM_OLD_GROWTH, RED_MUSHROOM_OLD_GROWTH];
    default_flowers => VegetalDecoration [FLOWER_DEFAULT];
    cherry_grove_vegetation => VegetalDecoration [PATCH_GRASS_PLAIN, FLOWER_CHERRY, TREES_CHERRY];
    meadow_vegetation => VegetalDecoration [PATCH_GRASS_MEADOW, FLOWER_MEADOW, TREES_MEADOW, WILDFLOWERS_MEADOW];
    warm_flowers => VegetalDecoration [FLOWER_WARM];
    default_grass => VegetalDecoration [PATCH_GRASS_BADLANDS];
    taiga_grass => VegetalDecoration [PATCH_GRASS_TAIGA_2, BROWN_MUSHROOM_TAIGA, RED_MUSHROOM_TAIGA];
    plain_grass => VegetalDecoration [PATCH_TALL_GRASS_2];
    default_mushrooms => VegetalDecoration [BROWN_MUSHROOM_NORMAL, RED_MUSHROOM_NORMAL];
    near_water_vegetation => VegetalDecoration [PATCH_SUGAR_CANE, PATCH_FIREFLY_BUSH_NEAR_WATER];
    leaf_litter_patch => VegetalDecoration [PATCH_LEAF_LITTER];
    badland_extra_vegetation => VegetalDecoration [PATCH_SUGAR_CANE_BADLANDS, PATCH_PUMPKIN, PATCH_CACTUS_DECORATED, PATCH_FIREFLY_BUSH_NEAR_WATER];
    jungle_vines => VegetalDecoration [VINES];
    desert_extra_vegetation => VegetalDecoration [PATCH_SUGAR_CANE_DESERT, PATCH_PUMPKIN, PATCH_CACTUS_DESERT];
    swamp_extra_vegetation => VegetalDecoration [PATCH_SUGAR_CANE_SWAMP, PATCH_PUMPKIN, PATCH_FIREFLY_BUSH_SWAMP, PATCH_FIREFLY_BUSH_NEAR_WATER_SWAMP];
    mangrove_swamp_extra_vegetation => VegetalDecoration [SEAGRASS_SWAMP, PATCH_FIREFLY_BUSH_NEAR_WATER];
    desert_extra_decoration => SurfaceStructures [DESERT_WELL];
    fossil_decoration => UndergroundStructures [FOSSIL_UPPER, FOSSIL_LOWER];
    cold_ocean_extra_vegetation => VegetalDecoration [KELP_COLD];
    lukewarm_kelp => VegetalDecoration [KELP_WARM];
    default_springs => FluidSprings [SPRING_WATER, SPRING_LAVA];
    frozen_springs => FluidSprings [SPRING_LAVA_FROZEN];
    icebergs => LocalModifications [ICEBERG_PACKED, ICEBERG_BLUE];
    blue_ice => SurfaceStructures [BLUE_ICE];
    surface_freezing => TopLayerModification [FREEZE_TOP_LAYER];
    ancient_debris => UndergroundDecoration [ORE_ANCIENT_DEBRIS_LARGE, ORE_DEBRIS_SMALL];
    default_crystal_formations => LocalModifications [AMETHYST_GEODE];
    pumpkin_patches => VegetalDecoration [PATCH_PUMPKIN];
    default_underground_variety => UndergroundOres [ORE_DIRT, ORE_GRAVEL, ORE_GRANITE_UPPER, ORE_GRANITE_LOWER, ORE_DIORITE_UPPER, ORE_DIORITE_LOWER, ORE_ANDESITE_UPPER, ORE_ANDESITE_LOWER, ORE_TUFF] VegetalDecoration [GLOW_LICHEN];
    dripstone => LocalModifications [LARGE_DRIPSTONE] UndergroundDecoration [DRIPSTONE_CLUSTER, POINTED_DRIPSTONE];
    sulfur_caves_features => Lakes [ROOTED_SULFUR_SPRING, SULFUR_POOL] UndergroundDecoration [SULFUR_SPIKE_CLUSTER, SULFUR_SPIKE];
}

macro_rules! set_groups {
    ($($name:ident => [$($part:ident),+];)*) => {$(
        pub fn $name(g: &mut Generation) {
            $($part(g);)+
        }
    )*};
}

pub fn default_carvers(g: &mut Generation) {
    g.carver(carver::CAVE)
        .carver(carver::CAVE_EXTRA_UNDERGROUND)
        .carver(carver::CANYON);
}

pub fn default_carvers_and_lakes(g: &mut Generation) {
    default_carvers(g);
    g.features(
        Lakes,
        &[
            placed_feature::LAKE_LAVA_UNDERGROUND,
            placed_feature::LAKE_LAVA_SURFACE,
        ],
    );
}

pub fn default_ores(g: &mut Generation) {
    ores(g, placed_feature::ORE_COPPER);
}

pub fn default_ores_with_large_copper_blobs(g: &mut Generation) {
    ores(g, placed_feature::ORE_COPPER_LARGE);
}

fn ores(g: &mut Generation, copper: PlacedFeatureKey) {
    g.features(
        UndergroundOres,
        &[
            placed_feature::ORE_COAL_UPPER,
            placed_feature::ORE_COAL_LOWER,
            placed_feature::ORE_IRON_UPPER,
            placed_feature::ORE_IRON_MIDDLE,
            placed_feature::ORE_IRON_SMALL,
            placed_feature::ORE_GOLD,
            placed_feature::ORE_GOLD_LOWER,
            placed_feature::ORE_REDSTONE,
            placed_feature::ORE_REDSTONE_LOWER,
            placed_feature::ORE_DIAMOND,
            placed_feature::ORE_DIAMOND_MEDIUM,
            placed_feature::ORE_DIAMOND_LARGE,
            placed_feature::ORE_DIAMOND_BURIED,
            placed_feature::ORE_LAPIS,
            placed_feature::ORE_LAPIS_BURIED,
            copper,
            placed_feature::UNDERWATER_MAGMA,
        ],
    );
}

pub fn nether_default_ores(g: &mut Generation) {
    g.features(
        UndergroundDecoration,
        &[
            placed_feature::ORE_GRAVEL_NETHER,
            placed_feature::ORE_BLACKSTONE,
            placed_feature::ORE_GOLD_NETHER,
            placed_feature::ORE_QUARTZ_NETHER,
        ],
    );
    ancient_debris(g);
}

impl Mobs {
    pub fn farm_animals(&mut self) -> &mut Self {
        self.spawn(SHEEP, 12, 4, 4)
            .spawn(PIG, 10, 4, 4)
            .spawn(CHICKEN, 10, 4, 4)
            .spawn(COW, 8, 4, 4)
    }

    pub fn taiga_animals(&mut self) -> &mut Self {
        self.farm_animals()
            .spawn(WOLF, 8, 4, 4)
            .spawn(RABBIT, 4, 2, 3)
            .spawn(FOX, 8, 2, 4)
    }

    pub fn cave_spawns(&mut self) -> &mut Self {
        self.spawn(BAT, 10, 8, 8).spawn(GLOW_SQUID, 10, 4, 6)
    }

    pub fn common_spawns(&mut self) -> &mut Self {
        self.common_spawns_with_skeletons(100)
    }

    pub fn common_spawns_with_skeletons(&mut self, skeleton_weight: i32) -> &mut Self {
        self.cave_spawns().monsters(95, 5, 0, skeleton_weight)
    }

    pub fn common_spawn_with_zombie_horse(&mut self) -> &mut Self {
        self.cave_spawns().monsters(90, 5, 5, 100)
    }

    pub fn swamp_spawns(&mut self, swamp_skeleton_weight: i32) -> &mut Self {
        self.common_spawns_with_skeletons(swamp_skeleton_weight)
            .spawn(SLIME, 1, 1, 1)
            .spawn(BOGGED, 30, 4, 4)
            .spawn(FROG, 10, 2, 5)
    }

    pub fn ocean_spawns(
        &mut self,
        squid_weight: i32,
        squid_max_count: i32,
        cod_weight: i32,
    ) -> &mut Self {
        self.spawn(SQUID, squid_weight, 1, squid_max_count)
            .spawn(COD, cod_weight, 3, 6)
            .common_spawns()
            .spawn(DROWNED, 5, 1, 1)
    }

    pub fn warm_ocean_spawns(&mut self, squid_weight: i32, squid_min_count: i32) -> &mut Self {
        self.spawn(SQUID, squid_weight, squid_min_count, 4)
            .spawn(TROPICAL_FISH, 25, 8, 8)
            .spawn(DOLPHIN, 2, 1, 2)
            .spawn(DROWNED, 5, 1, 1)
            .common_spawns()
    }

    pub fn plains_spawns(&mut self) -> &mut Self {
        self.farm_animals()
            .spawn(HORSE, 5, 2, 6)
            .spawn(DONKEY, 1, 1, 3)
            .common_spawn_with_zombie_horse()
    }

    pub fn snowy_spawns(&mut self, zombie_horse: bool) -> &mut Self {
        let (zombie_weight, zombie_horse_weight) = if zombie_horse { (90, 5) } else { (95, 0) };
        self.spawn(RABBIT, 10, 2, 3)
            .spawn(POLAR_BEAR, 1, 1, 2)
            .cave_spawns()
            .monsters(zombie_weight, 5, zombie_horse_weight, 20)
            .spawn(STRAY, 80, 4, 4)
    }

    pub fn desert_spawns(&mut self) -> &mut Self {
        self.spawn(RABBIT, 12, 2, 3)
            .spawn(CAMEL, 1, 1, 1)
            .cave_spawns()
            .monsters(19, 1, 0, 50)
            .spawn(HUSK, 80, 4, 4)
            .spawn(PARCHED, 50, 4, 4)
    }

    pub fn dripstone_caves_spawns(&mut self) -> &mut Self {
        self.common_spawns().spawn(DROWNED, 95, 4, 4)
    }

    pub fn monsters(
        &mut self,
        zombie_weight: i32,
        zombie_villager_weight: i32,
        zombie_horse_weight: i32,
        skeleton_weight: i32,
    ) -> &mut Self {
        self.spawn(SPIDER, 100, 4, 4)
            .spawn(ZOMBIE, zombie_weight, 4, 4)
            .spawn(ZOMBIE_VILLAGER, zombie_villager_weight, 1, 1);
        if zombie_horse_weight > 0 {
            self.spawn(ZOMBIE_HORSE, zombie_horse_weight, 1, 1);
        }
        self.spawn(SKELETON, skeleton_weight, 4, 4)
            .spawn(CREEPER, 100, 4, 4)
            .spawn(SLIME, 100, 4, 4)
            .spawn(ENDERMAN, 10, 1, 4)
            .spawn(WITCH, 5, 1, 1)
    }

    pub fn mooshroom_spawns(&mut self) -> &mut Self {
        self.spawn(MOOSHROOM, 8, 4, 8).cave_spawns()
    }

    pub fn base_jungle_spawns(&mut self) -> &mut Self {
        self.farm_animals().spawn(CHICKEN, 10, 4, 4).common_spawns()
    }

    pub fn end_spawns(&mut self) -> &mut Self {
        self.spawn(ENDERMAN, 10, 4, 4)
    }
}

#[rustfmt::skip]
set_groups! {
    mountain_ores => [extra_emeralds, infested_stone];
    default_extra_vegetation => [pumpkin_patches, near_water_vegetation];
    mushrooms_and_extra_vegetation => [default_mushrooms, default_extra_vegetation];
    default_vegetation => [default_flowers, default_grass, mushrooms_and_extra_vegetation];
    ocean_vegetation => [water_trees, default_vegetation];
    global_overworld_generation => [default_carvers_and_lakes, default_crystal_formations, default_monster_room, default_underground_variety, default_springs, surface_freezing];
    overworld_features => [global_overworld_generation, default_ores, default_soft_disks];
    cave_plains_vegetation => [plain_grass, plain_vegetation, default_mushrooms, pumpkin_patches];
}
