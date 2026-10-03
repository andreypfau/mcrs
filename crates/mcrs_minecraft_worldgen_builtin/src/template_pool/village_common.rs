use super::Piece::*;
use super::{Pool, legacy, pool};

#[rustfmt::skip]
pub const POOLS: &[Pool] = &[
    pool("village/common/animals").dir("village/common/animals/").pieces(&[
        (legacy("cows_1"), 7),
        (legacy("pigs_1"), 7),
        (legacy("horses_").numbered(1, 5), 1),
        (legacy("sheep_").numbered(1, 2), 1),
        (Empty, 5),
    ]),
    pool("village/common/butcher_animals").dir("village/common/animals/").pieces(&[
        (legacy("cows_1"), 3),
        (legacy("pigs_1"), 3),
        (legacy("sheep_").numbered(1, 2), 1),
    ]),
    pool("village/common/cats").dir("village/common/animals/").pieces(&[
        (legacy("cat_black"), 1),
        (legacy("cat_british"), 1),
        (legacy("cat_calico"), 1),
        (legacy("cat_persian"), 1),
        (legacy("cat_ragdoll"), 1),
        (legacy("cat_red"), 1),
        (legacy("cat_siamese"), 1),
        (legacy("cat_tabby"), 1),
        (legacy("cat_white"), 1),
        (legacy("cat_jellie"), 1),
        (Empty, 3),
    ]),
    pool("village/common/iron_golem").dir("village/common/").pieces(&[(legacy("iron_golem"), 1)]),
    pool("village/common/sheep").dir("village/common/animals/").pieces(&[(legacy("sheep_").numbered(1, 2), 1)]),
    pool("village/common/well_bottoms").dir("village/common/").pieces(&[(legacy("well_bottom"), 1)]),
];
