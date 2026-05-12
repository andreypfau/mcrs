//! Internal block-light BFS state.
//!
//! `BfsEntry(u64)` is the per-cell queue entry used by `propagate_increase`
//! and `propagate_decrease`. It is intentionally distinct from
//! `components::Wavefront(u32)`, which is the cross-section egress
//! representation pushed onto `BlockEgress` at face boundaries. The two
//! types differ in width and field set: `BfsEntry` carries Y plus a 6-bit
//! direction bitset and a 3-bit flag field, neither of which `Wavefront`
//! needs.
//!
//! Bit layout reference: Canvas `StarLightEngine.java:1050-1064`.

use mcrs_core::voxel_shape::Direction;

pub(crate) const FLAG_HAS_SIDED_TRANSPARENT_BLOCKS: u8 = 1 << 0;
pub(crate) const FLAG_RECHECK_LEVEL: u8 = 1 << 1;
pub(crate) const FLAG_WRITE_LEVEL: u8 = 1 << 2;
pub(crate) const ALL_DIRECTIONS_BITSET: u8 = 0b111111;

pub(crate) struct BfsEntry(pub(crate) u64);

#[inline]
pub(crate) const fn pack_bfs_entry(
    x: u8,
    z: u8,
    y: u8,
    level: u8,
    direction_bitset: u8,
    flags: u8,
) -> u64 {
    debug_assert!(x < 16);
    debug_assert!(z < 16);
    debug_assert!(y < 16);
    debug_assert!(level < 16);
    debug_assert!(direction_bitset < 64);
    debug_assert!(flags < 8);
    (x as u64 & 0xF)
        | ((z as u64 & 0xF) << 4)
        | ((y as u64 & 0xFFF) << 8)
        | ((level as u64 & 0xF) << 20)
        | ((direction_bitset as u64 & 0x3F) << 24)
        | ((flags as u64 & 0x7) << 30)
}

#[inline]
pub(crate) const fn unpack_bfs_entry_x(entry: u64) -> u8 {
    (entry & 0xF) as u8
}

#[inline]
pub(crate) const fn unpack_bfs_entry_z(entry: u64) -> u8 {
    ((entry >> 4) & 0xF) as u8
}

#[inline]
pub(crate) const fn unpack_bfs_entry_y(entry: u64) -> u16 {
    ((entry >> 8) & 0xFFF) as u16
}

#[inline]
pub(crate) const fn unpack_bfs_entry_level(entry: u64) -> u8 {
    ((entry >> 20) & 0xF) as u8
}

#[inline]
pub(crate) const fn unpack_bfs_entry_dir_bitset(entry: u64) -> u8 {
    ((entry >> 24) & 0x3F) as u8
}

#[inline]
pub(crate) const fn unpack_bfs_entry_flags(entry: u64) -> u8 {
    ((entry >> 30) & 0x7) as u8
}

#[inline]
pub(crate) const fn normal_of(d: Direction) -> (i8, i8, i8) {
    match d {
        Direction::Down => (0, -1, 0),
        Direction::Up => (0, 1, 0),
        Direction::North => (0, 0, -1),
        Direction::South => (0, 0, 1),
        Direction::West => (-1, 0, 0),
        Direction::East => (1, 0, 0),
    }
}

const fn build_directions_except_opposite() -> [u8; 6] {
    let all = 0b111111u8;
    let mut t = [0u8; 6];
    let mut i = 0;
    while i < 6 {
        let opp = i ^ 1;
        t[i] = all & !(1u8 << opp);
        i += 1;
    }
    t
}

pub(crate) const DIRECTIONS_EXCEPT_OPPOSITE: [u8; 6] = build_directions_except_opposite();

static ARR_00: [Direction; 0] = [];
static ARR_01: [Direction; 1] = [Direction::Down];
static ARR_02: [Direction; 1] = [Direction::Up];
static ARR_03: [Direction; 2] = [Direction::Down, Direction::Up];
static ARR_04: [Direction; 1] = [Direction::North];
static ARR_05: [Direction; 2] = [Direction::Down, Direction::North];
static ARR_06: [Direction; 2] = [Direction::Up, Direction::North];
static ARR_07: [Direction; 3] = [Direction::Down, Direction::Up, Direction::North];
static ARR_08: [Direction; 1] = [Direction::South];
static ARR_09: [Direction; 2] = [Direction::Down, Direction::South];
static ARR_10: [Direction; 2] = [Direction::Up, Direction::South];
static ARR_11: [Direction; 3] = [Direction::Down, Direction::Up, Direction::South];
static ARR_12: [Direction; 2] = [Direction::North, Direction::South];
static ARR_13: [Direction; 3] = [Direction::Down, Direction::North, Direction::South];
static ARR_14: [Direction; 3] = [Direction::Up, Direction::North, Direction::South];
static ARR_15: [Direction; 4] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
];
static ARR_16: [Direction; 1] = [Direction::West];
static ARR_17: [Direction; 2] = [Direction::Down, Direction::West];
static ARR_18: [Direction; 2] = [Direction::Up, Direction::West];
static ARR_19: [Direction; 3] = [Direction::Down, Direction::Up, Direction::West];
static ARR_20: [Direction; 2] = [Direction::North, Direction::West];
static ARR_21: [Direction; 3] = [Direction::Down, Direction::North, Direction::West];
static ARR_22: [Direction; 3] = [Direction::Up, Direction::North, Direction::West];
static ARR_23: [Direction; 4] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::West,
];
static ARR_24: [Direction; 2] = [Direction::South, Direction::West];
static ARR_25: [Direction; 3] = [Direction::Down, Direction::South, Direction::West];
static ARR_26: [Direction; 3] = [Direction::Up, Direction::South, Direction::West];
static ARR_27: [Direction; 4] = [
    Direction::Down,
    Direction::Up,
    Direction::South,
    Direction::West,
];
static ARR_28: [Direction; 3] = [Direction::North, Direction::South, Direction::West];
static ARR_29: [Direction; 4] = [
    Direction::Down,
    Direction::North,
    Direction::South,
    Direction::West,
];
static ARR_30: [Direction; 4] = [
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
];
static ARR_31: [Direction; 5] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
];
static ARR_32: [Direction; 1] = [Direction::East];
static ARR_33: [Direction; 2] = [Direction::Down, Direction::East];
static ARR_34: [Direction; 2] = [Direction::Up, Direction::East];
static ARR_35: [Direction; 3] = [Direction::Down, Direction::Up, Direction::East];
static ARR_36: [Direction; 2] = [Direction::North, Direction::East];
static ARR_37: [Direction; 3] = [Direction::Down, Direction::North, Direction::East];
static ARR_38: [Direction; 3] = [Direction::Up, Direction::North, Direction::East];
static ARR_39: [Direction; 4] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::East,
];
static ARR_40: [Direction; 2] = [Direction::South, Direction::East];
static ARR_41: [Direction; 3] = [Direction::Down, Direction::South, Direction::East];
static ARR_42: [Direction; 3] = [Direction::Up, Direction::South, Direction::East];
static ARR_43: [Direction; 4] = [
    Direction::Down,
    Direction::Up,
    Direction::South,
    Direction::East,
];
static ARR_44: [Direction; 3] = [Direction::North, Direction::South, Direction::East];
static ARR_45: [Direction; 4] = [
    Direction::Down,
    Direction::North,
    Direction::South,
    Direction::East,
];
static ARR_46: [Direction; 4] = [
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::East,
];
static ARR_47: [Direction; 5] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::East,
];
static ARR_48: [Direction; 2] = [Direction::West, Direction::East];
static ARR_49: [Direction; 3] = [Direction::Down, Direction::West, Direction::East];
static ARR_50: [Direction; 3] = [Direction::Up, Direction::West, Direction::East];
static ARR_51: [Direction; 4] = [
    Direction::Down,
    Direction::Up,
    Direction::West,
    Direction::East,
];
static ARR_52: [Direction; 3] = [Direction::North, Direction::West, Direction::East];
static ARR_53: [Direction; 4] = [
    Direction::Down,
    Direction::North,
    Direction::West,
    Direction::East,
];
static ARR_54: [Direction; 4] = [
    Direction::Up,
    Direction::North,
    Direction::West,
    Direction::East,
];
static ARR_55: [Direction; 5] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::West,
    Direction::East,
];
static ARR_56: [Direction; 3] = [Direction::South, Direction::West, Direction::East];
static ARR_57: [Direction; 4] = [
    Direction::Down,
    Direction::South,
    Direction::West,
    Direction::East,
];
static ARR_58: [Direction; 4] = [
    Direction::Up,
    Direction::South,
    Direction::West,
    Direction::East,
];
static ARR_59: [Direction; 5] = [
    Direction::Down,
    Direction::Up,
    Direction::South,
    Direction::West,
    Direction::East,
];
static ARR_60: [Direction; 4] = [
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];
static ARR_61: [Direction; 5] = [
    Direction::Down,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];
static ARR_62: [Direction; 5] = [
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];
static ARR_63: [Direction; 6] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

pub(crate) const DIRECTIONS_FROM_BITSET: [&'static [Direction]; 64] = [
    &ARR_00, &ARR_01, &ARR_02, &ARR_03, &ARR_04, &ARR_05, &ARR_06, &ARR_07, &ARR_08, &ARR_09,
    &ARR_10, &ARR_11, &ARR_12, &ARR_13, &ARR_14, &ARR_15, &ARR_16, &ARR_17, &ARR_18, &ARR_19,
    &ARR_20, &ARR_21, &ARR_22, &ARR_23, &ARR_24, &ARR_25, &ARR_26, &ARR_27, &ARR_28, &ARR_29,
    &ARR_30, &ARR_31, &ARR_32, &ARR_33, &ARR_34, &ARR_35, &ARR_36, &ARR_37, &ARR_38, &ARR_39,
    &ARR_40, &ARR_41, &ARR_42, &ARR_43, &ARR_44, &ARR_45, &ARR_46, &ARR_47, &ARR_48, &ARR_49,
    &ARR_50, &ARR_51, &ARR_52, &ARR_53, &ARR_54, &ARR_55, &ARR_56, &ARR_57, &ARR_58, &ARR_59,
    &ARR_60, &ARR_61, &ARR_62, &ARR_63,
];

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_DIRECTIONS: [Direction; 6] = [
        Direction::Down,
        Direction::Up,
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ];

    #[test]
    fn bfs_entry_pack_unpack_round_trip() {
        for &x in &[0u8, 7, 15] {
            for &z in &[0u8, 7, 15] {
                for &y in &[0u8, 7, 15] {
                    for &level in &[0u8, 7, 15] {
                        for &dir_bitset in &[0u8, 0b000001, 0b111110, 0b111111] {
                            for &flags in &[0u8, 1, 2, 4, 7] {
                                let packed = pack_bfs_entry(x, z, y, level, dir_bitset, flags);
                                assert_eq!(unpack_bfs_entry_x(packed), x, "x mismatch");
                                assert_eq!(unpack_bfs_entry_z(packed), z, "z mismatch");
                                assert_eq!(unpack_bfs_entry_y(packed) as u8, y, "y mismatch");
                                assert_eq!(
                                    unpack_bfs_entry_level(packed),
                                    level,
                                    "level mismatch"
                                );
                                assert_eq!(
                                    unpack_bfs_entry_dir_bitset(packed),
                                    dir_bitset,
                                    "dir_bitset mismatch"
                                );
                                assert_eq!(
                                    unpack_bfs_entry_flags(packed),
                                    flags,
                                    "flags mismatch"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn bfs_entry_reserved_bits_are_zero() {
        let packed = pack_bfs_entry(15, 15, 15, 15, 0b111111, 0b111);
        assert_eq!(packed >> 33, 0, "reserved bits 33..=63 must be zero");
    }

    #[test]
    fn bfs_directions_from_bitset_complete() {
        let slice = DIRECTIONS_FROM_BITSET[0b111111];
        assert_eq!(slice.len(), 6);
        for d in ALL_DIRECTIONS {
            assert!(
                slice.iter().filter(|&&x| x == d).count() == 1,
                "{:?} missing or duplicated",
                d
            );
        }
    }

    #[test]
    fn bfs_directions_from_bitset_empty() {
        let slice = DIRECTIONS_FROM_BITSET[0];
        assert!(slice.is_empty());
    }

    #[test]
    fn bfs_directions_from_bitset_single() {
        for d in ALL_DIRECTIONS {
            let bit = 1u8 << d.index();
            let slice = DIRECTIONS_FROM_BITSET[bit as usize];
            assert_eq!(slice.len(), 1, "len mismatch for {:?}", d);
            assert_eq!(slice[0], d);
        }
    }

    #[test]
    fn bfs_directions_except_opposite_clears_back_bit() {
        for d in ALL_DIRECTIONS {
            let bitset = DIRECTIONS_EXCEPT_OPPOSITE[d.index()];
            let opp_bit = 1u8 << d.opposite().index();
            assert_eq!(
                bitset & opp_bit,
                0,
                "opposite bit not cleared for {:?}",
                d
            );
            assert_eq!(
                bitset.count_ones(),
                5,
                "expected exactly five bits set for {:?}",
                d
            );
        }
    }
}
