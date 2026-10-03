use super::Piece::*;
use super::Pool;
use crate::keys::processors;
use mcrs_minecraft_core::rl;
use mcrs_minecraft_worldgen_feature::template::Projection::*;

#[rustfmt::skip]
pub const POOLS: &[Pool] = &[
    Pool {
        name: rl!("minecraft:pillager_outpost/base_plates"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (Legacy(rl!("minecraft:pillager_outpost/base_plate")), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:pillager_outpost/feature_plates"),
        fallback: rl!("minecraft:empty"),
        projection: TerrainMatching,
        pieces: &[
            (Legacy(rl!("minecraft:pillager_outpost/feature_plate")), 1),
        ],
    },
    Pool {
        name: rl!("minecraft:pillager_outpost/features"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (Legacy(rl!("minecraft:pillager_outpost/feature_cage1")), 1),
            (Legacy(rl!("minecraft:pillager_outpost/feature_cage2")), 1),
            (Legacy(rl!("minecraft:pillager_outpost/feature_cage_with_allays")), 1),
            (Legacy(rl!("minecraft:pillager_outpost/feature_logs")), 1),
            (Legacy(rl!("minecraft:pillager_outpost/feature_tent1")), 1),
            (Legacy(rl!("minecraft:pillager_outpost/feature_tent2")), 1),
            (Legacy(rl!("minecraft:pillager_outpost/feature_targets")), 1),
            (Empty, 6),
        ],
    },
    Pool {
        name: rl!("minecraft:pillager_outpost/towers"),
        fallback: rl!("minecraft:empty"),
        projection: Rigid,
        pieces: &[
            (List(&[Legacy(rl!("minecraft:pillager_outpost/watchtower")), LegacyWith(rl!("minecraft:pillager_outpost/watchtower_overgrown"), processors::OUTPOST_ROT)]), 1),
        ],
    },
];
