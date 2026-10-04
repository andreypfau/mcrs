use super::mob::*;
use super::{Generation, Mobs, carver, placed};
use mcrs_minecraft_worldgen_structure::DecorationStep::*;

macro_rules! feature_sets {
    ($($name:ident => $($step:ident [$($key:literal),+])+;)*) => {$(
        pub fn $name(g: &mut Generation) {
            $(g.features($step, &[$(placed!($key)),+]);)+
        }
    )*};
}

#[rustfmt::skip]
feature_sets! {
    default_monster_room => UndergroundStructures ["monster_room", "monster_room_deep"];
    sculk => UndergroundDecoration ["sculk_vein", "sculk_patch_deep_dark"];
    extra_gold => UndergroundOres ["ore_gold_extra"];
    extra_emeralds => UndergroundOres ["ore_emerald"];
    infested_stone => UndergroundDecoration ["ore_infested"];
    default_soft_disks => UndergroundOres ["disk_sand", "disk_clay", "disk_gravel"];
    swamp_clay_disk => UndergroundOres ["disk_clay"];
    mangrove_swamp_disks => UndergroundOres ["disk_grass", "disk_clay"];
    mossy_stone_block => LocalModifications ["forest_rock"];
    ferns => VegetalDecoration ["patch_large_fern"];
    bushes => VegetalDecoration ["patch_bush"];
    rare_berry_bushes => VegetalDecoration ["patch_berry_rare"];
    common_berry_bushes => VegetalDecoration ["patch_berry_common"];
    light_bamboo_vegetation => VegetalDecoration ["bamboo_light"];
    bamboo_vegetation => VegetalDecoration ["bamboo", "bamboo_vegetation"];
    taiga_trees => VegetalDecoration ["trees_taiga"];
    grove_trees => VegetalDecoration ["trees_grove"];
    water_trees => VegetalDecoration ["trees_water"];
    birch_trees => VegetalDecoration ["trees_birch"];
    other_birch_trees => VegetalDecoration ["trees_birch_and_oak_leaf_litter"];
    tall_birch_trees => VegetalDecoration ["birch_tall"];
    birch_forest_flowers => VegetalDecoration ["wildflowers_birch_forest"];
    savanna_trees => VegetalDecoration ["trees_savanna"];
    shattered_savanna_trees => VegetalDecoration ["trees_windswept_savanna"];
    lush_caves_vegetation_features => VegetalDecoration ["lush_caves_ceiling_vegetation", "cave_vines", "lush_caves_clay", "lush_caves_vegetation", "rooted_azalea_tree", "spore_blossom", "classic_vines_cave_feature"];
    lush_caves_special_ores => UndergroundOres ["ore_clay"];
    mountain_trees => VegetalDecoration ["trees_windswept_hills"];
    mountain_forest_trees => VegetalDecoration ["trees_windswept_forest"];
    badlands_trees => VegetalDecoration ["trees_badlands"];
    snowy_trees => VegetalDecoration ["trees_snowy"];
    jungle_grass => VegetalDecoration ["patch_grass_jungle"];
    savanna_grass => VegetalDecoration ["patch_tall_grass"];
    shattered_savanna_grass => VegetalDecoration ["patch_grass_normal"];
    savanna_extra_grass => VegetalDecoration ["patch_grass_savanna"];
    badland_grass => VegetalDecoration ["patch_grass_badlands", "patch_dry_grass_badlands", "patch_dead_bush_badlands"];
    forest_flowers => VegetalDecoration ["forest_flowers"];
    forest_grass => VegetalDecoration ["patch_grass_forest"];
    swamp_vegetation => VegetalDecoration ["trees_swamp", "flower_swamp", "patch_grass_normal", "patch_dead_bush", "patch_waterlily", "brown_mushroom_swamp", "red_mushroom_swamp"];
    dappled_forest_vegetation => VegetalDecoration ["brown_mushroom_dappled_forest", "patch_red_shrub"];
    mangrove_swamp_vegetation => VegetalDecoration ["trees_mangrove", "patch_grass_normal", "patch_dead_bush", "patch_waterlily"];
    mushroom_field_vegetation => VegetalDecoration ["mushroom_island_vegetation", "brown_mushroom_taiga", "red_mushroom_taiga"];
    plain_vegetation => VegetalDecoration ["trees_plains", "flower_plains", "patch_grass_plain"];
    desert_vegetation => VegetalDecoration ["patch_dry_grass_desert", "patch_dead_bush_2"];
    giant_taiga_vegetation => VegetalDecoration ["patch_grass_taiga", "patch_dead_bush", "brown_mushroom_old_growth", "red_mushroom_old_growth"];
    default_flowers => VegetalDecoration ["flower_default"];
    cherry_grove_vegetation => VegetalDecoration ["patch_grass_plain", "flower_cherry", "trees_cherry"];
    meadow_vegetation => VegetalDecoration ["patch_grass_meadow", "flower_meadow", "trees_meadow", "wildflowers_meadow"];
    warm_flowers => VegetalDecoration ["flower_warm"];
    default_grass => VegetalDecoration ["patch_grass_badlands"];
    taiga_grass => VegetalDecoration ["patch_grass_taiga_2", "brown_mushroom_taiga", "red_mushroom_taiga"];
    plain_grass => VegetalDecoration ["patch_tall_grass_2"];
    default_mushrooms => VegetalDecoration ["brown_mushroom_normal", "red_mushroom_normal"];
    near_water_vegetation => VegetalDecoration ["patch_sugar_cane", "patch_firefly_bush_near_water"];
    leaf_litter_patch => VegetalDecoration ["patch_leaf_litter"];
    badland_extra_vegetation => VegetalDecoration ["patch_sugar_cane_badlands", "patch_pumpkin", "patch_cactus_decorated", "patch_firefly_bush_near_water"];
    jungle_vines => VegetalDecoration ["vines"];
    desert_extra_vegetation => VegetalDecoration ["patch_sugar_cane_desert", "patch_pumpkin", "patch_cactus_desert"];
    swamp_extra_vegetation => VegetalDecoration ["patch_sugar_cane_swamp", "patch_pumpkin", "patch_firefly_bush_swamp", "patch_firefly_bush_near_water_swamp"];
    mangrove_swamp_extra_vegetation => VegetalDecoration ["seagrass_swamp", "patch_firefly_bush_near_water"];
    desert_extra_decoration => SurfaceStructures ["desert_well"];
    fossil_decoration => UndergroundStructures ["fossil_upper", "fossil_lower"];
    cold_ocean_extra_vegetation => VegetalDecoration ["kelp_cold"];
    lukewarm_kelp => VegetalDecoration ["kelp_warm"];
    default_springs => FluidSprings ["spring_water", "spring_lava"];
    frozen_springs => FluidSprings ["spring_lava_frozen"];
    icebergs => LocalModifications ["iceberg_packed", "iceberg_blue"];
    blue_ice => SurfaceStructures ["blue_ice"];
    surface_freezing => TopLayerModification ["freeze_top_layer"];
    ancient_debris => UndergroundDecoration ["ore_ancient_debris_large", "ore_debris_small"];
    default_crystal_formations => LocalModifications ["amethyst_geode"];
    pumpkin_patches => VegetalDecoration ["patch_pumpkin"];
    default_underground_variety => UndergroundOres ["ore_dirt", "ore_gravel", "ore_granite_upper", "ore_granite_lower", "ore_diorite_upper", "ore_diorite_lower", "ore_andesite_upper", "ore_andesite_lower", "ore_tuff"] VegetalDecoration ["glow_lichen"];
    dripstone => LocalModifications ["large_dripstone"] UndergroundDecoration ["dripstone_cluster", "pointed_dripstone"];
    sulfur_caves_features => Lakes ["rooted_sulfur_spring", "sulfur_pool"] UndergroundDecoration ["sulfur_spike_cluster", "sulfur_spike"];
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
            placed!("lake_lava_underground"),
            placed!("lake_lava_surface"),
        ],
    );
}

pub fn default_ores(g: &mut Generation) {
    ores(g, placed!("ore_copper"));
}

pub fn default_ores_with_large_copper_blobs(g: &mut Generation) {
    ores(g, placed!("ore_copper_large"));
}

fn ores(g: &mut Generation, copper: crate::keys::PlacedKey) {
    g.features(
        UndergroundOres,
        &[
            placed!("ore_coal_upper"),
            placed!("ore_coal_lower"),
            placed!("ore_iron_upper"),
            placed!("ore_iron_middle"),
            placed!("ore_iron_small"),
            placed!("ore_gold"),
            placed!("ore_gold_lower"),
            placed!("ore_redstone"),
            placed!("ore_redstone_lower"),
            placed!("ore_diamond"),
            placed!("ore_diamond_medium"),
            placed!("ore_diamond_large"),
            placed!("ore_diamond_buried"),
            placed!("ore_lapis"),
            placed!("ore_lapis_buried"),
            copper,
            placed!("underwater_magma"),
        ],
    );
}

pub fn nether_default_ores(g: &mut Generation) {
    g.features(
        UndergroundDecoration,
        &[
            placed!("ore_gravel_nether"),
            placed!("ore_blackstone"),
            placed!("ore_gold_nether"),
            placed!("ore_quartz_nether"),
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
