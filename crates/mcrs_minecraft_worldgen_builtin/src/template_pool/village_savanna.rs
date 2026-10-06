use super::Piece::*;
use super::{Piece, Pool, legacy, pool};
use mcrs_minecraft_worldgen_feature::keys::placed_feature;
use mcrs_minecraft_worldgen_feature::keys::processor_list;

const STREETS: &[(Piece, i32)] = &[
    (legacy("corner_01"), 2),
    (legacy("corner_03"), 2),
    (legacy("straight_02"), 4),
    (legacy("straight_04"), 7),
    (legacy("straight_05"), 3),
    (legacy("straight_06"), 4),
    (legacy("straight_").padded(8, 11, 2), 4),
    (legacy("crossroad_02"), 1),
    (legacy("crossroad_").padded(3, 7, 2), 2),
    (legacy("split_").padded(1, 2, 2), 2),
    (legacy("turn_01"), 3),
];

const DECOR: &[(Piece, i32)] = &[
    (legacy("savanna_lamp_post_01"), 4),
    (Feature(placed_feature::ACACIA), 4),
    (Feature(placed_feature::PILE_HAY), 4),
    (Feature(placed_feature::PILE_MELON), 1),
    (Empty, 4),
];

#[rustfmt::skip]
pub const POOLS: &[Pool] = &[
    pool("village/savanna/decor").dir("village/savanna/").pieces(DECOR),
    pool("village/savanna/houses").fallback("village/savanna/terminators").dir("village/savanna/houses/").pieces(&[
        (legacy("savanna_small_house_").numbered(1, 8), 2),
        (legacy("savanna_medium_house_").numbered(1, 2), 2),
        (legacy("savanna_butchers_shop_").numbered(1, 2), 2),
        (legacy("savanna_tool_smith_1"), 2),
        (legacy("savanna_fletcher_house_1"), 2),
        (legacy("savanna_shepherd_1"), 7),
        (legacy("savanna_armorer_1"), 1),
        (legacy("savanna_fisher_cottage_1"), 3),
        (legacy("savanna_tannery_1"), 2),
        (legacy("savanna_cartographer_1"), 2),
        (legacy("savanna_library_1"), 2),
        (legacy("savanna_mason_1"), 2),
        (legacy("savanna_weaponsmith_").numbered(1, 2), 2),
        (legacy("savanna_temple_1"), 2),
        (legacy("savanna_temple_2"), 3),
        (legacy("savanna_large_farm_1").with(processor_list::FARM_SAVANNA), 4),
        (legacy("savanna_large_farm_2").with(processor_list::FARM_SAVANNA), 6),
        (legacy("savanna_small_farm").with(processor_list::FARM_SAVANNA), 4),
        (legacy("savanna_animal_pen_").numbered(1, 3), 2),
        (Empty, 5),
    ]),
    pool("village/savanna/streets").fallback("village/savanna/terminators").terrain_matching().dir("village/savanna/streets/").processors(processor_list::STREET_SAVANNA).pieces(STREETS),
    pool("village/savanna/terminators").terrain_matching().dir("village/").processors(processor_list::STREET_SAVANNA).pieces(&[
        (legacy("plains/terminators/terminator_").padded(1, 4, 2), 1),
        (legacy("savanna/terminators/terminator_05"), 1),
    ]),
    pool("village/savanna/town_centers").dir("village/savanna/").pieces(&[
        (legacy("town_centers/savanna_meeting_point_1"), 100),
        (legacy("town_centers/savanna_meeting_point_2"), 50),
        (legacy("town_centers/savanna_meeting_point_").numbered(3, 4), 150),
        (legacy("zombie/town_centers/savanna_meeting_point_1").with(processor_list::ZOMBIE_SAVANNA), 2),
        (legacy("zombie/town_centers/savanna_meeting_point_2").with(processor_list::ZOMBIE_SAVANNA), 1),
        (legacy("zombie/town_centers/savanna_meeting_point_").numbered(3, 4).with(processor_list::ZOMBIE_SAVANNA), 3),
    ]),
    pool("village/savanna/trees").pieces(&[(Feature(placed_feature::ACACIA), 1)]),
    pool("village/savanna/villagers").dir("village/savanna/villagers/").pieces(&[
        (legacy("nitwit"), 1),
        (legacy("baby"), 1),
        (legacy("unemployed"), 10),
    ]),
    pool("village/savanna/zombie/decor").dir("village/savanna/").processors(processor_list::ZOMBIE_SAVANNA).pieces(DECOR),
    pool("village/savanna/zombie/houses").fallback("village/savanna/zombie/terminators").dir("village/savanna/").processors(processor_list::ZOMBIE_SAVANNA).pieces(&[
        (legacy("zombie/houses/savanna_small_house_").numbered(1, 8), 2),
        (legacy("zombie/houses/savanna_medium_house_").numbered(1, 2), 2),
        (legacy("houses/savanna_butchers_shop_").numbered(1, 2), 2),
        (legacy("houses/savanna_tool_smith_1"), 2),
        (legacy("houses/savanna_fletcher_house_1"), 2),
        (legacy("houses/savanna_shepherd_1"), 2),
        (legacy("houses/savanna_armorer_1"), 1),
        (legacy("houses/savanna_fisher_cottage_1"), 2),
        (legacy("houses/savanna_tannery_1"), 2),
        (legacy("houses/savanna_cartographer_1"), 2),
        (legacy("houses/savanna_library_1"), 2),
        (legacy("houses/savanna_mason_1"), 2),
        (legacy("houses/savanna_weaponsmith_").numbered(1, 2), 2),
        (legacy("houses/savanna_temple_1"), 1),
        (legacy("houses/savanna_temple_2"), 3),
        (legacy("houses/savanna_large_farm_1"), 4),
        (legacy("zombie/houses/savanna_large_farm_2"), 4),
        (legacy("houses/savanna_small_farm"), 4),
        (legacy("houses/savanna_animal_pen_1"), 2),
        (legacy("zombie/houses/savanna_animal_pen_").numbered(2, 3), 2),
        (Empty, 5),
    ]),
    pool("village/savanna/zombie/streets").fallback("village/savanna/zombie/terminators").terrain_matching().dir("village/savanna/zombie/streets/").processors(processor_list::STREET_SAVANNA).pieces(STREETS),
    pool("village/savanna/zombie/terminators").terrain_matching().dir("village/").processors(processor_list::STREET_SAVANNA).pieces(&[
        (legacy("plains/terminators/terminator_").padded(1, 4, 2), 1),
        (legacy("savanna/zombie/terminators/terminator_05"), 1),
    ]),
    pool("village/savanna/zombie/villagers").dir("village/savanna/zombie/villagers/").pieces(&[
        (legacy("nitwit"), 1),
        (legacy("unemployed"), 10),
    ]),
];
