use super::Piece::*;
use super::{Pool, pool, single};
use mcrs_minecraft_worldgen_feature::keys::placed_feature;
use mcrs_minecraft_worldgen_feature::keys::processor_list;

#[rustfmt::skip]
pub const POOLS: &[Pool] = &[
    pool("ancient_city/city/entrance").dir("ancient_city/city/entrance/").processors(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION).pieces(&[
        (single("entrance_connector"), 1),
        (single("entrance_path_").numbered(1, 5), 1),
    ]),
    pool("ancient_city/city_center").dir("ancient_city/city_center/").processors(processor_list::ANCIENT_CITY_START_DEGRADATION).pieces(&[(single("city_center_").numbered(1, 3), 1)]),
    pool("ancient_city/city_center/walls").dir("ancient_city/city_center/walls/").processors(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION).pieces(&[
        (single("bottom_").numbered(1, 2), 1),
        (single("bottom_left_corner"), 1),
        (single("bottom_right_corner_").numbered(1, 2), 1),
        (single("left"), 1),
        (single("right"), 1),
        (single("top"), 1),
        (single("top_right_corner"), 1),
        (single("top_left_corner"), 1),
    ]),
    pool("ancient_city/sculk").pieces(&[
        (Feature(placed_feature::SCULK_PATCH_ANCIENT_CITY), 6),
        (Empty, 1),
    ]),
    pool("ancient_city/structures").dir("ancient_city/structures/").pieces(&[
        (Empty, 7),
        (single("barracks").with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION), 4),
        (single("chamber_").numbered(1, 3).with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION), 4),
        (single("sauna_1").with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION), 4),
        (single("small_statue").with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION), 4),
        (single("large_ruin_1").with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION), 1),
        (single("tall_ruin_").numbered(1, 2).with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION), 1),
        (single("tall_ruin_").numbered(3, 4).with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION), 2),
        (List(&[single("camp_1").with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION), single("camp_2").with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION), single("camp_3").with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION)]), 1),
        (single("medium_ruin_").numbered(1, 2).with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION), 1),
        (single("small_ruin_").numbered(1, 2).with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION), 1),
        (single("large_pillar_1").with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION), 1),
        (single("medium_pillar_1").with(processor_list::ANCIENT_CITY_GENERIC_DEGRADATION), 1),
        (List(&[single("ice_box_1")]), 1),
    ]),
    pool("ancient_city/walls").dir("ancient_city/walls/").processors(processor_list::ANCIENT_CITY_WALLS_DEGRADATION).pieces(&[
        (single("intact_corner_wall_1"), 1),
        (single("intact_intersection_wall_1"), 1),
        (single("intact_lshape_wall_1"), 1),
        (single("intact_horizontal_wall_").numbered(1, 2), 1),
        (single("intact_horizontal_wall_stairs_").numbered(1, 3), 1),
        (single("intact_horizontal_wall_stairs_4"), 4),
        (single("intact_horizontal_wall_passage_1"), 3),
        (single("ruined_corner_wall_").numbered(1, 2), 1),
        (single("ruined_horizontal_wall_stairs_").numbered(1, 2), 2),
        (single("ruined_horizontal_wall_stairs_").numbered(3, 4), 3),
    ]),
    pool("ancient_city/walls/no_corners").dir("ancient_city/walls/").processors(processor_list::ANCIENT_CITY_WALLS_DEGRADATION).pieces(&[
        (single("intact_horizontal_wall_").numbered(1, 2), 1),
        (single("intact_horizontal_wall_stairs_").numbered(1, 5), 1),
        (single("intact_horizontal_wall_bridge"), 1),
    ]),
];
