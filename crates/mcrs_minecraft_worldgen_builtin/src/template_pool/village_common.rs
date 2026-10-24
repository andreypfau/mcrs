use super::Piece::*;
use super::{Pool, legacy, pool};
use mcrs_minecraft_core::rl;

#[rustfmt::skip]
pub const POOLS: &[Pool] = &[
    pool(rl!("minecraft:village/common/animals")).dir("village/common/animals/").pieces(&[
        (Template(legacy("cows_1")), 7),
        (Template(legacy("pigs_1")), 7),
        (Template(legacy("horses_").numbered(1, 5)), 1),
        (Template(legacy("sheep_").numbered(1, 2)), 1),
        (Empty, 5),
    ]),
    pool(rl!("minecraft:village/common/butcher_animals")).dir("village/common/animals/").pieces(&[
        (Template(legacy("cows_1")), 3),
        (Template(legacy("pigs_1")), 3),
        (Template(legacy("sheep_").numbered(1, 2)), 1),
    ]),
    pool(rl!("minecraft:village/common/cats")).dir("village/common/animals/").pieces(&[
        (Template(legacy("cat_black")), 1),
        (Template(legacy("cat_british")), 1),
        (Template(legacy("cat_calico")), 1),
        (Template(legacy("cat_persian")), 1),
        (Template(legacy("cat_ragdoll")), 1),
        (Template(legacy("cat_red")), 1),
        (Template(legacy("cat_siamese")), 1),
        (Template(legacy("cat_tabby")), 1),
        (Template(legacy("cat_white")), 1),
        (Template(legacy("cat_jellie")), 1),
        (Empty, 3),
    ]),
    pool(rl!("minecraft:village/common/iron_golem")).dir("village/common/").pieces(&[(Template(legacy("iron_golem")), 1)]),
    pool(rl!("minecraft:village/common/sheep")).dir("village/common/animals/").pieces(&[(Template(legacy("sheep_").numbered(1, 2)), 1)]),
    pool(rl!("minecraft:village/common/well_bottoms")).dir("village/common/").pieces(&[(Template(legacy("well_bottom")), 1)]),
];
