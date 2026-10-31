use super::Piece::*;
use super::{Piece, Pool, legacy, pool};
use mcrs_minecraft_biome::keys::placed_feature;
use mcrs_minecraft_resource::rl;
use mcrs_minecraft_worldgen_feature::keys::processor_list;

const STREETS: &[(Piece, i32)] = &[
    (Template(legacy("corner_01")), 2),
    (Template(legacy("corner_03")), 2),
    (Template(legacy("straight_02")), 4),
    (Template(legacy("straight_04")), 7),
    (Template(legacy("straight_05")), 3),
    (Template(legacy("straight_06")), 4),
    (Template(legacy("straight_").padded(8, 11, 2)), 4),
    (Template(legacy("crossroad_02")), 1),
    (Template(legacy("crossroad_").padded(3, 7, 2)), 2),
    (Template(legacy("split_").padded(1, 2, 2)), 2),
    (Template(legacy("turn_01")), 3),
];

const DECOR: &[(Piece, i32)] = &[
    (Template(legacy("savanna_lamp_post_01")), 4),
    (Feature(placed_feature::ACACIA), 4),
    (Feature(placed_feature::PILE_HAY), 4),
    (Feature(placed_feature::PILE_MELON), 1),
    (Empty, 4),
];

#[rustfmt::skip]
pub const POOLS: &[Pool] = &[
    pool(rl!("minecraft:village/savanna/decor")).dir("village/savanna/").pieces(DECOR),
    pool(rl!("minecraft:village/savanna/houses")).fallback(rl!("minecraft:village/savanna/terminators")).dir("village/savanna/houses/").pieces(&[
        (Template(legacy("savanna_small_house_").numbered(1, 8)), 2),
        (Template(legacy("savanna_medium_house_").numbered(1, 2)), 2),
        (Template(legacy("savanna_butchers_shop_").numbered(1, 2)), 2),
        (Template(legacy("savanna_tool_smith_1")), 2),
        (Template(legacy("savanna_fletcher_house_1")), 2),
        (Template(legacy("savanna_shepherd_1")), 7),
        (Template(legacy("savanna_armorer_1")), 1),
        (Template(legacy("savanna_fisher_cottage_1")), 3),
        (Template(legacy("savanna_tannery_1")), 2),
        (Template(legacy("savanna_cartographer_1")), 2),
        (Template(legacy("savanna_library_1")), 2),
        (Template(legacy("savanna_mason_1")), 2),
        (Template(legacy("savanna_weaponsmith_").numbered(1, 2)), 2),
        (Template(legacy("savanna_temple_1")), 2),
        (Template(legacy("savanna_temple_2")), 3),
        (Template(legacy("savanna_large_farm_1").with(processor_list::FARM_SAVANNA)), 4),
        (Template(legacy("savanna_large_farm_2").with(processor_list::FARM_SAVANNA)), 6),
        (Template(legacy("savanna_small_farm").with(processor_list::FARM_SAVANNA)), 4),
        (Template(legacy("savanna_animal_pen_").numbered(1, 3)), 2),
        (Empty, 5),
    ]),
    pool(rl!("minecraft:village/savanna/streets")).fallback(rl!("minecraft:village/savanna/terminators")).terrain_matching().dir("village/savanna/streets/").processors(processor_list::STREET_SAVANNA).pieces(STREETS),
    pool(rl!("minecraft:village/savanna/terminators")).terrain_matching().dir("village/").processors(processor_list::STREET_SAVANNA).pieces(&[
        (Template(legacy("plains/terminators/terminator_").padded(1, 4, 2)), 1),
        (Template(legacy("savanna/terminators/terminator_05")), 1),
    ]),
    pool(rl!("minecraft:village/savanna/town_centers")).dir("village/savanna/").pieces(&[
        (Template(legacy("town_centers/savanna_meeting_point_1")), 100),
        (Template(legacy("town_centers/savanna_meeting_point_2")), 50),
        (Template(legacy("town_centers/savanna_meeting_point_").numbered(3, 4)), 150),
        (Template(legacy("zombie/town_centers/savanna_meeting_point_1").with(processor_list::ZOMBIE_SAVANNA)), 2),
        (Template(legacy("zombie/town_centers/savanna_meeting_point_2").with(processor_list::ZOMBIE_SAVANNA)), 1),
        (Template(legacy("zombie/town_centers/savanna_meeting_point_").numbered(3, 4).with(processor_list::ZOMBIE_SAVANNA)), 3),
    ]),
    pool(rl!("minecraft:village/savanna/trees")).pieces(&[(Feature(placed_feature::ACACIA), 1)]),
    pool(rl!("minecraft:village/savanna/villagers")).dir("village/savanna/villagers/").pieces(&[
        (Template(legacy("nitwit")), 1),
        (Template(legacy("baby")), 1),
        (Template(legacy("unemployed")), 10),
    ]),
    pool(rl!("minecraft:village/savanna/zombie/decor")).dir("village/savanna/").processors(processor_list::ZOMBIE_SAVANNA).pieces(DECOR),
    pool(rl!("minecraft:village/savanna/zombie/houses")).fallback(rl!("minecraft:village/savanna/zombie/terminators")).dir("village/savanna/").processors(processor_list::ZOMBIE_SAVANNA).pieces(&[
        (Template(legacy("zombie/houses/savanna_small_house_").numbered(1, 8)), 2),
        (Template(legacy("zombie/houses/savanna_medium_house_").numbered(1, 2)), 2),
        (Template(legacy("houses/savanna_butchers_shop_").numbered(1, 2)), 2),
        (Template(legacy("houses/savanna_tool_smith_1")), 2),
        (Template(legacy("houses/savanna_fletcher_house_1")), 2),
        (Template(legacy("houses/savanna_shepherd_1")), 2),
        (Template(legacy("houses/savanna_armorer_1")), 1),
        (Template(legacy("houses/savanna_fisher_cottage_1")), 2),
        (Template(legacy("houses/savanna_tannery_1")), 2),
        (Template(legacy("houses/savanna_cartographer_1")), 2),
        (Template(legacy("houses/savanna_library_1")), 2),
        (Template(legacy("houses/savanna_mason_1")), 2),
        (Template(legacy("houses/savanna_weaponsmith_").numbered(1, 2)), 2),
        (Template(legacy("houses/savanna_temple_1")), 1),
        (Template(legacy("houses/savanna_temple_2")), 3),
        (Template(legacy("houses/savanna_large_farm_1")), 4),
        (Template(legacy("zombie/houses/savanna_large_farm_2")), 4),
        (Template(legacy("houses/savanna_small_farm")), 4),
        (Template(legacy("houses/savanna_animal_pen_1")), 2),
        (Template(legacy("zombie/houses/savanna_animal_pen_").numbered(2, 3)), 2),
        (Empty, 5),
    ]),
    pool(rl!("minecraft:village/savanna/zombie/streets")).fallback(rl!("minecraft:village/savanna/zombie/terminators")).terrain_matching().dir("village/savanna/zombie/streets/").processors(processor_list::STREET_SAVANNA).pieces(STREETS),
    pool(rl!("minecraft:village/savanna/zombie/terminators")).terrain_matching().dir("village/").processors(processor_list::STREET_SAVANNA).pieces(&[
        (Template(legacy("plains/terminators/terminator_").padded(1, 4, 2)), 1),
        (Template(legacy("savanna/zombie/terminators/terminator_05")), 1),
    ]),
    pool(rl!("minecraft:village/savanna/zombie/villagers")).dir("village/savanna/zombie/villagers/").pieces(&[
        (Template(legacy("nitwit")), 1),
        (Template(legacy("unemployed")), 10),
    ]),
];
