use super::Piece::*;
use super::{Pool, pool, single};
use mcrs_minecraft_biome::keys::placed_feature;
use mcrs_minecraft_core::rl;
use mcrs_minecraft_worldgen_feature::keys::processor_list;

#[rustfmt::skip]
pub const POOLS: &[Pool] = &[
    pool(rl!("minecraft:ancient_city/city/entrance")).dir("ancient_city/city/entrance/").processors(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION).pieces(&[
        (Template(single("entrance_connector")), 1),
        (Template(single("entrance_path_").numbered(1, 5)), 1),
    ]),
    pool(rl!("minecraft:ancient_city/city_center")).dir("ancient_city/city_center/").processors(processor_list::ANCIENT_CITY_START_DEGRADATION).pieces(&[(Template(single("city_center_").numbered(1, 3)), 1)]),
    pool(rl!("minecraft:ancient_city/city_center/walls")).dir("ancient_city/city_center/walls/").processors(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION).pieces(&[
        (Template(single("bottom_").numbered(1, 2)), 1),
        (Template(single("bottom_left_corner")), 1),
        (Template(single("bottom_right_corner_").numbered(1, 2)), 1),
        (Template(single("left")), 1),
        (Template(single("right")), 1),
        (Template(single("top")), 1),
        (Template(single("top_right_corner")), 1),
        (Template(single("top_left_corner")), 1),
    ]),
    pool(rl!("minecraft:ancient_city/sculk")).pieces(&[
        (Feature(placed_feature::SCULK_PATCH_ANCIENT_CITY), 6),
        (Empty, 1),
    ]),
    pool(rl!("minecraft:ancient_city/structures")).dir("ancient_city/structures/").pieces(&[
        (Empty, 7),
        (Template(single("barracks").with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION)), 4),
        (Template(single("chamber_").numbered(1, 3).with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION)), 4),
        (Template(single("sauna_1").with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION)), 4),
        (Template(single("small_statue").with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION)), 4),
        (Template(single("large_ruin_1").with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION)), 1),
        (Template(single("tall_ruin_").numbered(1, 2).with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION)), 1),
        (Template(single("tall_ruin_").numbered(3, 4).with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION)), 2),
        (List(&[Template(single("camp_1").with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION)), Template(single("camp_2").with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION)), Template(single("camp_3").with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION))]), 1),
        (Template(single("medium_ruin_").numbered(1, 2).with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION)), 1),
        (Template(single("small_ruin_").numbered(1, 2).with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION)), 1),
        (Template(single("large_pillar_1").with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION)), 1),
        (Template(single("medium_pillar_1").with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION)), 1),
        (List(&[Template(single("ice_box_1"))]), 1),
    ]),
    pool(rl!("minecraft:ancient_city/walls")).dir("ancient_city/walls/").processors(processor_list::ANCIENT_CITY_WALLS_DEGRADATION).pieces(&[
        (Template(single("intact_corner_wall_1")), 1),
        (Template(single("intact_intersection_wall_1")), 1),
        (Template(single("intact_lshape_wall_1")), 1),
        (Template(single("intact_horizontal_wall_").numbered(1, 2)), 1),
        (Template(single("intact_horizontal_wall_stairs_").numbered(1, 3)), 1),
        (Template(single("intact_horizontal_wall_stairs_4")), 4),
        (Template(single("intact_horizontal_wall_passage_1")), 3),
        (Template(single("ruined_corner_wall_").numbered(1, 2)), 1),
        (Template(single("ruined_horizontal_wall_stairs_").numbered(1, 2)), 2),
        (Template(single("ruined_horizontal_wall_stairs_").numbered(3, 4)), 3),
    ]),
    pool(rl!("minecraft:ancient_city/walls/no_corners")).dir("ancient_city/walls/").processors(processor_list::ANCIENT_CITY_WALLS_DEGRADATION).pieces(&[
        (Template(single("intact_horizontal_wall_").numbered(1, 2)), 1),
        (Template(single("intact_horizontal_wall_stairs_").numbered(1, 5)), 1),
        (Template(single("intact_horizontal_wall_bridge")), 1),
    ]),
];
