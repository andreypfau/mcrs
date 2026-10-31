use super::Piece::*;
use super::{Piece, Pool, legacy, pool};
use mcrs_minecraft_biome::keys::placed_feature;
use mcrs_minecraft_resource::rl;
use mcrs_minecraft_worldgen_feature::keys::processor_list;

const STREETS: &[(Piece, i32)] = &[
    (Template(legacy("corner_").padded(1, 2, 2)), 3),
    (Template(legacy("straight_").padded(1, 2, 2)), 4),
    (Template(legacy("straight_03")), 3),
    (Template(legacy("crossroad_").padded(1, 3, 2)), 3),
    (Template(legacy("square_").padded(1, 2, 2)), 3),
    (Template(legacy("turn_01")), 3),
];

const DECOR: &[(Piece, i32)] = &[
    (Template(legacy("desert_lamp_1")), 10),
    (Feature(placed_feature::PATCH_CACTUS), 4),
    (Feature(placed_feature::PILE_HAY), 4),
    (Empty, 10),
];

#[rustfmt::skip]
pub const POOLS: &[Pool] = &[
    pool(rl!("minecraft:village/desert/camel")).dir("village/desert/").pieces(&[(Template(legacy("camel_spawn")), 1)]),
    pool(rl!("minecraft:village/desert/decor")).dir("village/desert/").pieces(DECOR),
    pool(rl!("minecraft:village/desert/houses")).fallback(rl!("minecraft:village/desert/terminators")).dir("village/desert/houses/").pieces(&[
        (Template(legacy("desert_small_house_").numbered(1, 5)), 2),
        (Template(legacy("desert_small_house_6")), 1),
        (Template(legacy("desert_small_house_").numbered(7, 8)), 2),
        (Template(legacy("desert_medium_house_").numbered(1, 2)), 2),
        (Template(legacy("desert_butcher_shop_1")), 2),
        (Template(legacy("desert_tool_smith_1")), 2),
        (Template(legacy("desert_fletcher_house_1")), 2),
        (Template(legacy("desert_shepherd_house_1")), 2),
        (Template(legacy("desert_armorer_1")), 1),
        (Template(legacy("desert_fisher_1")), 2),
        (Template(legacy("desert_tannery_1")), 2),
        (Template(legacy("desert_cartographer_house_1")), 2),
        (Template(legacy("desert_library_1")), 2),
        (Template(legacy("desert_mason_1")), 2),
        (Template(legacy("desert_weaponsmith_1")), 2),
        (Template(legacy("desert_temple_").numbered(1, 2)), 2),
        (Template(legacy("desert_large_farm_1").with(processor_list::FARM_DESERT)), 11),
        (Template(legacy("desert_farm_").numbered(1, 2).with(processor_list::FARM_DESERT)), 4),
        (Template(legacy("desert_animal_pen_").numbered(1, 2)), 2),
        (Empty, 5),
    ]),
    pool(rl!("minecraft:village/desert/streets")).fallback(rl!("minecraft:village/desert/terminators")).terrain_matching().dir("village/desert/streets/").pieces(STREETS),
    pool(rl!("minecraft:village/desert/terminators")).terrain_matching().dir("village/desert/terminators/").pieces(&[(Template(legacy("terminator_").padded(1, 2, 2)), 1)]),
    pool(rl!("minecraft:village/desert/town_centers")).dir("village/desert/").pieces(&[
        (Template(legacy("town_centers/desert_meeting_point_").numbered(1, 2)), 98),
        (Template(legacy("town_centers/desert_meeting_point_3")), 49),
        (Template(legacy("zombie/town_centers/desert_meeting_point_").numbered(1, 2).with(processor_list::ZOMBIE_DESERT)), 2),
        (Template(legacy("zombie/town_centers/desert_meeting_point_3").with(processor_list::ZOMBIE_DESERT)), 1),
    ]),
    pool(rl!("minecraft:village/desert/villagers")).dir("village/desert/villagers/").pieces(&[
        (Template(legacy("nitwit")), 1),
        (Template(legacy("baby")), 1),
        (Template(legacy("unemployed")), 10),
    ]),
    pool(rl!("minecraft:village/desert/zombie/decor")).dir("village/desert/").processors(processor_list::ZOMBIE_DESERT).pieces(DECOR),
    pool(rl!("minecraft:village/desert/zombie/houses")).fallback(rl!("minecraft:village/desert/zombie/terminators")).dir("village/desert/").processors(processor_list::ZOMBIE_DESERT).pieces(&[
        (Template(legacy("zombie/houses/desert_small_house_").numbered(1, 5)), 2),
        (Template(legacy("zombie/houses/desert_small_house_6")), 1),
        (Template(legacy("zombie/houses/desert_small_house_").numbered(7, 8)), 2),
        (Template(legacy("zombie/houses/desert_medium_house_").numbered(1, 2)), 2),
        (Template(legacy("houses/desert_butcher_shop_1")), 2),
        (Template(legacy("houses/desert_tool_smith_1")), 2),
        (Template(legacy("houses/desert_fletcher_house_1")), 2),
        (Template(legacy("houses/desert_shepherd_house_1")), 2),
        (Template(legacy("houses/desert_armorer_1")), 1),
        (Template(legacy("houses/desert_fisher_1")), 2),
        (Template(legacy("houses/desert_tannery_1")), 2),
        (Template(legacy("houses/desert_cartographer_house_1")), 2),
        (Template(legacy("houses/desert_library_1")), 2),
        (Template(legacy("houses/desert_mason_1")), 2),
        (Template(legacy("houses/desert_weaponsmith_1")), 2),
        (Template(legacy("houses/desert_temple_").numbered(1, 2)), 2),
        (Template(legacy("houses/desert_large_farm_1")), 7),
        (Template(legacy("houses/desert_farm_").numbered(1, 2)), 4),
        (Template(legacy("houses/desert_animal_pen_").numbered(1, 2)), 2),
        (Empty, 5),
    ]),
    pool(rl!("minecraft:village/desert/zombie/streets")).fallback(rl!("minecraft:village/desert/zombie/terminators")).terrain_matching().dir("village/desert/zombie/streets/").pieces(STREETS),
    pool(rl!("minecraft:village/desert/zombie/terminators")).terrain_matching().dir("village/desert/").pieces(&[
        (Template(legacy("terminators/terminator_01")), 1),
        (Template(legacy("zombie/terminators/terminator_02")), 1),
    ]),
    pool(rl!("minecraft:village/desert/zombie/villagers")).dir("village/desert/zombie/villagers/").pieces(&[
        (Template(legacy("nitwit")), 1),
        (Template(legacy("unemployed")), 10),
    ]),
];
