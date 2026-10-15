use mcrs_minecraft_volume::{Access, Denied, Unavailable};

pub const WEAKEST_FIRST: [Access; 5] = [
    Access::Outside,
    Access::Unloaded,
    Access::Foreign,
    Access::Read,
    Access::Write,
];

#[test]
fn meet_keeps_the_weaker_right() {
    for (i, a) in WEAKEST_FIRST.iter().enumerate() {
        for (j, b) in WEAKEST_FIRST.iter().enumerate() {
            assert_eq!(a.meet(*b), WEAKEST_FIRST[i.min(j)], "{a:?} meet {b:?}");
        }
    }
}

#[test]
fn per_call_types_stay_small() {
    use mcrs_minecraft_volume::{BlockStateId, Cursor, SparseVolume};
    use std::mem::size_of;
    assert!(size_of::<Result<BlockStateId, Unavailable>>() <= 4);
    assert!(size_of::<Result<BlockStateId, Denied>>() <= 4);
    assert!(size_of::<Cursor<'_, SparseVolume>>() <= 48);
}
