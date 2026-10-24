use super::Piece::*;
use super::{Piece, Pool, legacy, pool};
use mcrs_minecraft_biome::keys::placed_feature;
use mcrs_minecraft_core::rl;
use mcrs_minecraft_worldgen_feature::keys::processor_list;

const STREETS: &[(Piece, i32)] = &[
    (Template(legacy("corner_").padded(1, 3, 2)), 2),
    (Template(legacy("square_01")), 2),
    (Template(legacy("straight_").padded(1, 3, 2)), 4),
    (Template(legacy("straight_04")), 7),
    (Template(legacy("straight_06")), 4),
    (Template(legacy("straight_08")), 4),
    (Template(legacy("crossroad_02")), 1),
    (Template(legacy("crossroad_").padded(3, 6, 2)), 2),
    (Template(legacy("turn_01")), 3),
];

#[rustfmt::skip]
pub const POOLS: &[Pool] = &[
    pool(rl!("minecraft:village/snowy/decor")).dir("village/snowy/").pieces(&[
        (Template(legacy("snowy_lamp_post_").padded(1, 2, 2)), 4),
        (Template(legacy("snowy_lamp_post_03")), 1),
        (Feature(placed_feature::SPRUCE), 4),
        (Feature(placed_feature::PILE_SNOW), 4),
        (Feature(placed_feature::PILE_ICE), 1),
        (Empty, 9),
    ]),
    pool(rl!("minecraft:village/snowy/houses")).fallback(rl!("minecraft:village/snowy/terminators")).dir("village/snowy/houses/").pieces(&[
        (Template(legacy("snowy_small_house_").numbered(1, 3)), 2),
        (Template(legacy("snowy_small_house_4")), 3),
        (Template(legacy("snowy_small_house_").numbered(5, 8)), 2),
        (Template(legacy("snowy_medium_house_").numbered(1, 3)), 2),
        (Template(legacy("snowy_butchers_shop_").numbered(1, 2)), 2),
        (Template(legacy("snowy_tool_smith_1")), 2),
        (Template(legacy("snowy_fletcher_house_1")), 2),
        (Template(legacy("snowy_shepherds_house_1")), 3),
        (Template(legacy("snowy_armorer_house_").numbered(1, 2)), 1),
        (Template(legacy("snowy_fisher_cottage")), 2),
        (Template(legacy("snowy_tannery_1")), 2),
        (Template(legacy("snowy_cartographer_house_1")), 2),
        (Template(legacy("snowy_library_1")), 2),
        (Template(legacy("snowy_masons_house_").numbered(1, 2)), 2),
        (Template(legacy("snowy_weapon_smith_1")), 2),
        (Template(legacy("snowy_temple_1")), 2),
        (Template(legacy("snowy_farm_").numbered(1, 2).with(processor_list::FARM_SNOWY)), 3),
        (Template(legacy("snowy_animal_pen_").numbered(1, 2)), 2),
        (Empty, 6),
    ]),
    pool(rl!("minecraft:village/snowy/streets")).fallback(rl!("minecraft:village/snowy/terminators")).terrain_matching().dir("village/snowy/streets/").processors(processor_list::STREET_SNOWY_OR_TAIGA).pieces(STREETS),
    pool(rl!("minecraft:village/snowy/terminators")).terrain_matching().dir("village/plains/terminators/").processors(processor_list::STREET_SNOWY_OR_TAIGA).pieces(&[(Template(legacy("terminator_").padded(1, 4, 2)), 1)]),
    pool(rl!("minecraft:village/snowy/town_centers")).dir("village/snowy/").pieces(&[
        (Template(legacy("town_centers/snowy_meeting_point_1")), 100),
        (Template(legacy("town_centers/snowy_meeting_point_2")), 50),
        (Template(legacy("town_centers/snowy_meeting_point_3")), 150),
        (Template(legacy("zombie/town_centers/snowy_meeting_point_1")), 2),
        (Template(legacy("zombie/town_centers/snowy_meeting_point_2")), 1),
        (Template(legacy("zombie/town_centers/snowy_meeting_point_3")), 3),
    ]),
    pool(rl!("minecraft:village/snowy/trees")).pieces(&[(Feature(placed_feature::SPRUCE), 1)]),
    pool(rl!("minecraft:village/snowy/villagers")).dir("village/snowy/villagers/").pieces(&[
        (Template(legacy("nitwit")), 1),
        (Template(legacy("baby")), 1),
        (Template(legacy("unemployed")), 10),
    ]),
    pool(rl!("minecraft:village/snowy/zombie/decor")).dir("village/snowy/").processors(processor_list::ZOMBIE_SNOWY).pieces(&[
        (Template(legacy("snowy_lamp_post_").padded(1, 3, 2)), 1),
        (Feature(placed_feature::SPRUCE), 4),
        (Feature(placed_feature::PILE_SNOW), 4),
        (Feature(placed_feature::PILE_ICE), 4),
        (Empty, 7),
    ]),
    pool(rl!("minecraft:village/snowy/zombie/houses")).fallback(rl!("minecraft:village/snowy/terminators")).dir("village/snowy/").processors(processor_list::ZOMBIE_SNOWY).pieces(&[
        (Template(legacy("zombie/houses/snowy_small_house_").numbered(1, 8)), 2),
        (Template(legacy("zombie/houses/snowy_medium_house_").numbered(1, 2)), 2),
        (Template(legacy("zombie/houses/snowy_medium_house_3")), 1),
        (Template(legacy("houses/snowy_butchers_shop_").numbered(1, 2)), 2),
        (Template(legacy("houses/snowy_tool_smith_1")), 2),
        (Template(legacy("houses/snowy_fletcher_house_1")), 2),
        (Template(legacy("houses/snowy_shepherds_house_1")), 2),
        (Template(legacy("houses/snowy_armorer_house_").numbered(1, 2)), 1),
        (Template(legacy("houses/snowy_fisher_cottage")), 2),
        (Template(legacy("houses/snowy_tannery_1")), 2),
        (Template(legacy("houses/snowy_cartographer_house_1")), 2),
        (Template(legacy("houses/snowy_library_1")), 2),
        (Template(legacy("houses/snowy_masons_house_").numbered(1, 2)), 2),
        (Template(legacy("houses/snowy_weapon_smith_1")), 2),
        (Template(legacy("houses/snowy_temple_1")), 2),
        (Template(legacy("houses/snowy_farm_").numbered(1, 2)), 3),
        (Template(legacy("houses/snowy_animal_pen_").numbered(1, 2)), 2),
        (Empty, 6),
    ]),
    pool(rl!("minecraft:village/snowy/zombie/streets")).fallback(rl!("minecraft:village/snowy/terminators")).terrain_matching().dir("village/snowy/zombie/streets/").processors(processor_list::STREET_SNOWY_OR_TAIGA).pieces(STREETS),
    pool(rl!("minecraft:village/snowy/zombie/villagers")).dir("village/snowy/zombie/villagers/").pieces(&[
        (Template(legacy("nitwit")), 1),
        (Template(legacy("unemployed")), 10),
    ]),
];
