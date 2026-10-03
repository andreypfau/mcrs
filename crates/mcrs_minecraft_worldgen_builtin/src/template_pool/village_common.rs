use super::Piece::*;
use super::Pool;
use mcrs_minecraft_core::rl;
use mcrs_minecraft_worldgen_feature::template::Projection::*;

#[rustfmt::skip]
pub const POOLS: &[Pool] = &[
    Pool {
        name: rl!("minecraft:village/common/animals"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (Legacy(rl!("minecraft:village/common/animals/cows_1")), 7),
            (Legacy(rl!("minecraft:village/common/animals/pigs_1")), 7),
            (Legacy(rl!("minecraft:village/common/animals/horses_1")), 1),
            (Legacy(rl!("minecraft:village/common/animals/horses_2")), 1),
            (Legacy(rl!("minecraft:village/common/animals/horses_3")), 1),
            (Legacy(rl!("minecraft:village/common/animals/horses_4")), 1),
            (Legacy(rl!("minecraft:village/common/animals/horses_5")), 1),
            (Legacy(rl!("minecraft:village/common/animals/sheep_1")), 1),
            (Legacy(rl!("minecraft:village/common/animals/sheep_2")), 1),
            (Empty, 5),
        ],
    },
    Pool {
        name: rl!("minecraft:village/common/butcher_animals"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (Legacy(rl!("minecraft:village/common/animals/cows_1")), 3),
            (Legacy(rl!("minecraft:village/common/animals/pigs_1")), 3),
            (Legacy(rl!("minecraft:village/common/animals/sheep_1")), 1),
            (Legacy(rl!("minecraft:village/common/animals/sheep_2")), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:village/common/cats"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (Legacy(rl!("minecraft:village/common/animals/cat_black")), 1),
            (Legacy(rl!("minecraft:village/common/animals/cat_british")), 1),
            (Legacy(rl!("minecraft:village/common/animals/cat_calico")), 1),
            (Legacy(rl!("minecraft:village/common/animals/cat_persian")), 1),
            (Legacy(rl!("minecraft:village/common/animals/cat_ragdoll")), 1),
            (Legacy(rl!("minecraft:village/common/animals/cat_red")), 1),
            (Legacy(rl!("minecraft:village/common/animals/cat_siamese")), 1),
            (Legacy(rl!("minecraft:village/common/animals/cat_tabby")), 1),
            (Legacy(rl!("minecraft:village/common/animals/cat_white")), 1),
            (Legacy(rl!("minecraft:village/common/animals/cat_jellie")), 1),
            (Empty, 3),
        ],
    },
    Pool {
        name: rl!("minecraft:village/common/iron_golem"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (Legacy(rl!("minecraft:village/common/iron_golem")), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:village/common/sheep"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (Legacy(rl!("minecraft:village/common/animals/sheep_1")), 1),
            (Legacy(rl!("minecraft:village/common/animals/sheep_2")), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:village/common/well_bottoms"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (Legacy(rl!("minecraft:village/common/well_bottom")), 1),
        ],
    },
];
