use super::Piece::*;
use super::Pool;
use crate::keys::processors;
use mcrs_minecraft_core::rl;
use mcrs_minecraft_worldgen_feature::template::Projection::*;

#[rustfmt::skip]
pub const POOLS: &[Pool] = &[
    Pool {
        name: rl!("minecraft:bastion/blocks/gold"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (Single(rl!("minecraft:bastion/blocks/air")), 3),
            (Single(rl!("minecraft:bastion/blocks/gold")), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/bridge/bridge_pieces"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/bridge/bridge_pieces/bridge"), processors::BRIDGE), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/bridge/connectors"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/bridge/connectors/back_bridge_top"), processors::BASTION_GENERIC_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/bridge/connectors/back_bridge_bottom"), processors::BASTION_GENERIC_DEGRADATION), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/bridge/legs"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/bridge/legs/leg_0"), processors::BASTION_GENERIC_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/bridge/legs/leg_1"), processors::BASTION_GENERIC_DEGRADATION), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/bridge/rampart_plates"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/bridge/rampart_plates/plate_0"), processors::RAMPART_DEGRADATION), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/bridge/ramparts"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/bridge/ramparts/rampart_0"), processors::RAMPART_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/bridge/ramparts/rampart_1"), processors::RAMPART_DEGRADATION), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/bridge/starting_pieces"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/bridge/starting_pieces/entrance"), processors::ENTRANCE_REPLACEMENT), 1),
            (SingleWith(rl!("minecraft:bastion/bridge/starting_pieces/entrance_face"), processors::BASTION_GENERIC_DEGRADATION), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/bridge/walls"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/bridge/walls/wall_base_0"), processors::RAMPART_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/bridge/walls/wall_base_1"), processors::RAMPART_DEGRADATION), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/hoglin_stable/connectors"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/connectors/end_post_connector"), processors::STABLE_DEGRADATION), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/hoglin_stable/large_stables/inner"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/large_stables/inner_0"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/large_stables/inner_1"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/large_stables/inner_2"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/large_stables/inner_3"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/large_stables/inner_4"), processors::STABLE_DEGRADATION), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/hoglin_stable/large_stables/outer"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/large_stables/outer_0"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/large_stables/outer_1"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/large_stables/outer_2"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/large_stables/outer_3"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/large_stables/outer_4"), processors::STABLE_DEGRADATION), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/hoglin_stable/mirrored_starting_pieces"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/starting_pieces/stairs_0_mirrored"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/starting_pieces/stairs_1_mirrored"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/starting_pieces/stairs_2_mirrored"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/starting_pieces/stairs_3_mirrored"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/starting_pieces/stairs_4_mirrored"), processors::STABLE_DEGRADATION), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/hoglin_stable/posts"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/posts/stair_post"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/posts/end_post"), processors::STABLE_DEGRADATION), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/hoglin_stable/rampart_plates"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/rampart_plates/rampart_plate_1"), processors::STABLE_DEGRADATION), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/hoglin_stable/ramparts"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/ramparts/ramparts_1"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/ramparts/ramparts_2"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/ramparts/ramparts_3"), processors::STABLE_DEGRADATION), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/hoglin_stable/small_stables/inner"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/small_stables/inner_0"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/small_stables/inner_1"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/small_stables/inner_2"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/small_stables/inner_3"), processors::STABLE_DEGRADATION), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/hoglin_stable/small_stables/outer"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/small_stables/outer_0"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/small_stables/outer_1"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/small_stables/outer_2"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/small_stables/outer_3"), processors::STABLE_DEGRADATION), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/hoglin_stable/stairs"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/stairs/stairs_1_0"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/stairs/stairs_1_1"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/stairs/stairs_1_2"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/stairs/stairs_1_3"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/stairs/stairs_1_4"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/stairs/stairs_2_0"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/stairs/stairs_2_1"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/stairs/stairs_2_2"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/stairs/stairs_2_3"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/stairs/stairs_2_4"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/stairs/stairs_3_0"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/stairs/stairs_3_1"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/stairs/stairs_3_2"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/stairs/stairs_3_3"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/stairs/stairs_3_4"), processors::STABLE_DEGRADATION), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/hoglin_stable/starting_pieces"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/starting_pieces/starting_stairs_0"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/starting_pieces/starting_stairs_1"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/starting_pieces/starting_stairs_2"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/starting_pieces/starting_stairs_3"), processors::STABLE_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/starting_pieces/starting_stairs_4"), processors::STABLE_DEGRADATION), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/hoglin_stable/wall_bases"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/walls/wall_base"), processors::STABLE_DEGRADATION), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/hoglin_stable/walls"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/walls/side_wall_0"), processors::SIDE_WALL_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/walls/side_wall_1"), processors::SIDE_WALL_DEGRADATION), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/mobs/hoglin"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (Single(rl!("minecraft:bastion/mobs/hoglin")), 2),
            (Single(rl!("minecraft:bastion/mobs/empty")), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/mobs/piglin"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (Single(rl!("minecraft:bastion/mobs/melee_piglin")), 1),
            (Single(rl!("minecraft:bastion/mobs/sword_piglin")), 4),
            (Single(rl!("minecraft:bastion/mobs/crossbow_piglin")), 4),
            (Single(rl!("minecraft:bastion/mobs/empty")), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/mobs/piglin_melee"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (Single(rl!("minecraft:bastion/mobs/melee_piglin_always")), 1),
            (Single(rl!("minecraft:bastion/mobs/melee_piglin")), 5),
            (Single(rl!("minecraft:bastion/mobs/sword_piglin")), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/starts"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/units/air_base"), processors::BASTION_GENERIC_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/hoglin_stable/air_base"), processors::BASTION_GENERIC_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/big_air_full"), processors::BASTION_GENERIC_DEGRADATION), 1),
            (SingleWith(rl!("minecraft:bastion/bridge/starting_pieces/entrance_base"), processors::BASTION_GENERIC_DEGRADATION), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/treasure/bases"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/treasure/bases/lava_basin"), processors::TREASURE_ROOMS), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/treasure/bases/centers"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/treasure/bases/centers/center_0"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/bases/centers/center_1"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/bases/centers/center_2"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/bases/centers/center_3"), processors::TREASURE_ROOMS), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/treasure/brains"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/treasure/brains/center_brain"), processors::TREASURE_ROOMS), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/treasure/connectors"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/treasure/connectors/center_to_wall_middle"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/connectors/center_to_wall_top"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/connectors/center_to_wall_top_entrance"), processors::TREASURE_ROOMS), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/treasure/corners/bottom"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/treasure/corners/bottom/corner_0"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/corners/bottom/corner_1"), processors::TREASURE_ROOMS), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/treasure/corners/edges"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/treasure/corners/edges/bottom"), processors::HIGH_WALL), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/corners/edges/middle"), processors::HIGH_WALL), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/corners/edges/top"), processors::HIGH_WALL), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/treasure/corners/middle"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/treasure/corners/middle/corner_0"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/corners/middle/corner_1"), processors::TREASURE_ROOMS), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/treasure/corners/top"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/treasure/corners/top/corner_0"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/corners/top/corner_1"), processors::TREASURE_ROOMS), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/treasure/entrances"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/treasure/entrances/entrance_0"), processors::TREASURE_ROOMS), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/treasure/extensions/houses"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/treasure/extensions/house_0"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/extensions/house_1"), processors::TREASURE_ROOMS), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/treasure/extensions/large_pool"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/treasure/extensions/empty"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/extensions/empty"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/extensions/fire_room"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/extensions/large_bridge_0"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/extensions/large_bridge_1"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/extensions/large_bridge_2"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/extensions/large_bridge_3"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/extensions/roofed_bridge"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/extensions/empty"), processors::TREASURE_ROOMS), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/treasure/extensions/small_pool"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/treasure/extensions/empty"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/extensions/fire_room"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/extensions/empty"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/extensions/small_bridge_0"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/extensions/small_bridge_1"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/extensions/small_bridge_2"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/extensions/small_bridge_3"), processors::TREASURE_ROOMS), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/treasure/ramparts"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/treasure/ramparts/mid_wall_main"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/ramparts/mid_wall_side"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/ramparts/bottom_wall_0"), processors::BOTTOM_RAMPART), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/ramparts/top_wall"), processors::HIGH_RAMPART), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/ramparts/lava_basin_side"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/ramparts/lava_basin_main"), processors::TREASURE_ROOMS), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/treasure/roofs"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/treasure/roofs/wall_roof"), processors::ROOF), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/roofs/corner_roof"), processors::ROOF), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/roofs/center_roof"), processors::ROOF), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/treasure/stairs"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/treasure/stairs/lower_stairs"), processors::TREASURE_ROOMS), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/treasure/walls"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/treasure/walls/lava_wall"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/walls/entrance_wall"), processors::HIGH_WALL), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/treasure/walls/bottom"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/treasure/walls/bottom/wall_0"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/walls/bottom/wall_1"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/walls/bottom/wall_2"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/walls/bottom/wall_3"), processors::TREASURE_ROOMS), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/treasure/walls/mid"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/treasure/walls/mid/wall_0"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/walls/mid/wall_1"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/walls/mid/wall_2"), processors::TREASURE_ROOMS), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/treasure/walls/outer"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/treasure/walls/outer/top_corner"), processors::HIGH_WALL), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/walls/outer/mid_corner"), processors::HIGH_WALL), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/walls/outer/bottom_corner"), processors::HIGH_WALL), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/walls/outer/outer_wall"), processors::HIGH_WALL), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/walls/outer/medium_outer_wall"), processors::HIGH_WALL), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/walls/outer/tall_outer_wall"), processors::HIGH_WALL), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/treasure/walls/top"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/treasure/walls/top/main_entrance"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/walls/top/wall_0"), processors::TREASURE_ROOMS), 1),
            (SingleWith(rl!("minecraft:bastion/treasure/walls/top/wall_1"), processors::TREASURE_ROOMS), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/units/center_pieces"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/units/center_pieces/center_0"), processors::HOUSING), 1),
            (SingleWith(rl!("minecraft:bastion/units/center_pieces/center_1"), processors::HOUSING), 1),
            (SingleWith(rl!("minecraft:bastion/units/center_pieces/center_2"), processors::HOUSING), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/units/edge_wall_units"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/units/wall_units/edge_0_large"), processors::HOUSING), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/units/edges"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/units/edges/edge_0"), processors::HOUSING), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/units/fillers/stage_0"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/units/fillers/stage_0"), processors::HOUSING), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/units/large_ramparts"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/units/ramparts/ramparts_0"), processors::HOUSING), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/units/pathways"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/units/pathways/pathway_0"), processors::HOUSING), 1),
            (SingleWith(rl!("minecraft:bastion/units/pathways/pathway_wall_0"), processors::HOUSING), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/units/rampart_plates"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/units/rampart_plates/plate_0"), processors::HOUSING), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/units/ramparts"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/units/ramparts/ramparts_0"), processors::HOUSING), 1),
            (SingleWith(rl!("minecraft:bastion/units/ramparts/ramparts_1"), processors::HOUSING), 1),
            (SingleWith(rl!("minecraft:bastion/units/ramparts/ramparts_2"), processors::HOUSING), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/units/stages/rot/stage_1"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/units/stages/rot/stage_1_0"), processors::HOUSING), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/units/stages/stage_0"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/units/stages/stage_0_0"), processors::HOUSING), 1),
            (SingleWith(rl!("minecraft:bastion/units/stages/stage_0_1"), processors::HOUSING), 1),
            (SingleWith(rl!("minecraft:bastion/units/stages/stage_0_2"), processors::HOUSING), 1),
            (SingleWith(rl!("minecraft:bastion/units/stages/stage_0_3"), processors::HOUSING), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/units/stages/stage_1"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/units/stages/stage_1_0"), processors::HOUSING), 1),
            (SingleWith(rl!("minecraft:bastion/units/stages/stage_1_1"), processors::HOUSING), 1),
            (SingleWith(rl!("minecraft:bastion/units/stages/stage_1_2"), processors::HOUSING), 1),
            (SingleWith(rl!("minecraft:bastion/units/stages/stage_1_3"), processors::HOUSING), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/units/stages/stage_2"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/units/stages/stage_2_0"), processors::HOUSING), 1),
            (SingleWith(rl!("minecraft:bastion/units/stages/stage_2_1"), processors::HOUSING), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/units/stages/stage_3"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/units/stages/stage_3_0"), processors::HOUSING), 1),
            (SingleWith(rl!("minecraft:bastion/units/stages/stage_3_1"), processors::HOUSING), 1),
            (SingleWith(rl!("minecraft:bastion/units/stages/stage_3_2"), processors::HOUSING), 1),
            (SingleWith(rl!("minecraft:bastion/units/stages/stage_3_3"), processors::HOUSING), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/units/wall_units"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/units/wall_units/unit_0"), processors::HOUSING), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:bastion/units/walls/wall_bases"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (SingleWith(rl!("minecraft:bastion/units/walls/wall_base"), processors::HOUSING), 1),
            (SingleWith(rl!("minecraft:bastion/units/walls/connected_wall"), processors::HOUSING), 1),
        ],
    },
];
