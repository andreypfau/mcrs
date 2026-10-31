use super::Piece::*;
use super::{Pool, legacy, pool};
use mcrs_minecraft_resource::rl;
use mcrs_minecraft_worldgen_feature::keys::processor_list;

#[rustfmt::skip]
pub const POOLS: &[Pool] = &[
    pool(rl!("minecraft:pillager_outpost/base_plates")).dir("pillager_outpost/").pieces(&[(Template(legacy("base_plate")), 1)]),
    pool(rl!("minecraft:pillager_outpost/feature_plates")).terrain_matching().dir("pillager_outpost/").pieces(&[(Template(legacy("feature_plate")), 1)]),
    pool(rl!("minecraft:pillager_outpost/features")).dir("pillager_outpost/").pieces(&[
        (Template(legacy("feature_cage").numbered(1, 2)), 1),
        (Template(legacy("feature_cage_with_allays")), 1),
        (Template(legacy("feature_logs")), 1),
        (Template(legacy("feature_tent").numbered(1, 2)), 1),
        (Template(legacy("feature_targets")), 1),
        (Empty, 6),
    ]),
    pool(rl!("minecraft:pillager_outpost/towers")).dir("pillager_outpost/").pieces(&[(List(&[Template(legacy("watchtower")), Template(legacy("watchtower_overgrown").with(processor_list::OUTPOST_ROT))]), 1)]),
];
