use super::{Pool, pool, single};
use mcrs_minecraft_keys::processor_list;

#[rustfmt::skip]
pub const POOLS: &[Pool] = &[
    pool("trail_ruins/buildings").dir("trail_ruins/buildings/").processors(processor_list::TRAIL_RUINS_HOUSES_ARCHAEOLOGY).pieces(&[
        (single("group_hall_").numbered(1, 5), 1),
        (single("large_room_").numbered(1, 5), 1),
        (single("one_room_").numbered(1, 5), 1),
    ]),
    pool("trail_ruins/buildings/grouped").dir("trail_ruins/buildings/").processors(processor_list::TRAIL_RUINS_HOUSES_ARCHAEOLOGY).pieces(&[
        (single("group_full_").numbered(1, 5), 1),
        (single("group_lower_").numbered(1, 5), 1),
        (single("group_upper_").numbered(1, 5), 1),
        (single("group_room_").numbered(1, 5), 1),
    ]),
    pool("trail_ruins/decor").dir("trail_ruins/decor/").processors(processor_list::TRAIL_RUINS_HOUSES_ARCHAEOLOGY).pieces(&[(single("decor_").numbered(1, 7), 1)]),
    pool("trail_ruins/roads").dir("trail_ruins/roads/").processors(processor_list::TRAIL_RUINS_ROADS_ARCHAEOLOGY).pieces(&[
        (single("long_road_end"), 1),
        (single("road_end_1"), 1),
        (single("road_section_").numbered(1, 4), 1),
        (single("road_spacer_1"), 1),
    ]),
    pool("trail_ruins/tower").dir("trail_ruins/tower/").processors(processor_list::TRAIL_RUINS_HOUSES_ARCHAEOLOGY).pieces(&[(single("tower_").numbered(1, 5), 1)]),
    pool("trail_ruins/tower/additions").dir("trail_ruins/tower/").processors(processor_list::TRAIL_RUINS_HOUSES_ARCHAEOLOGY).pieces(&[
        (single("hall_").numbered(1, 5), 1),
        (single("large_hall_").numbered(1, 5), 1),
        (single("one_room_").numbered(1, 5), 1),
        (single("platform_").numbered(1, 5), 1),
        (single("stable_").numbered(1, 5), 1),
    ]),
    pool("trail_ruins/tower/tower_top").dir("trail_ruins/tower/").processors(processor_list::TRAIL_RUINS_TOWER_TOP_ARCHAEOLOGY).pieces(&[(single("tower_top_").numbered(1, 5), 1)]),
];
