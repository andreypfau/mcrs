use super::*;

struct Section16;

impl SectionKind for Section16 {
    const AXIS_BITS: u32 = 4;
    const MIN_INDIRECT_BITS: u32 = 4;
}

#[test]
fn packing_and_unpacking_are_inverses() {
    for bits in 1..=16u32 {
        let cells: Vec<u16> = (0..Section16::ENTRY_COUNT)
            .map(|i| (i as u16).wrapping_mul(2654) & ((1u32 << bits) - 1) as u16)
            .collect();
        let packed = pack_from(bits, &cells, |&c| c as u32);
        assert_eq!(packed.len(), packed_len(bits, cells.len()));

        let mut out = vec![0u16; Section16::ENTRY_COUNT];
        unpack_into(bits, &packed, &mut out).unwrap();
        assert_eq!(out, cells, "{bits} bits");
    }
}

#[test]
fn a_data_array_of_the_wrong_length_is_an_error() {
    let mut out = [0u16; 64];
    assert_eq!(
        unpack_into(4, &[0i64; 3], &mut out),
        Err(DataLength {
            found: 3,
            expected: 4
        })
    );
}

#[test]
fn remapping_translates_every_cell_through_the_table() {
    let cells: Vec<u16> = (0..64u16).map(|i| i % 4).collect();
    let packed = pack_from(2, &cells, |&c| c as u32);
    let table = [70u16, 71, 72, 73];
    let mut out = [0u16; 64];
    remap_into(2, &packed, &table, &mut out).unwrap();
    assert!(
        out.iter()
            .zip(&cells)
            .all(|(&o, &c)| o == table[c as usize])
    );
}

#[test]
fn reading_one_cell_agrees_with_unpacking_all_of_them() {
    for bits in 1..=16u32 {
        let cells: Vec<u16> = (0..Section16::ENTRY_COUNT)
            .map(|i| (i as u16).wrapping_mul(7919) & ((1u32 << bits) - 1) as u16)
            .collect();
        let packed = pack_from(bits, &cells, |&c| c as u32);
        for index in [0, 1, 63, 64, 1000, Section16::ENTRY_COUNT - 1] {
            assert_eq!(entry_at(bits, &packed, index), cells[index], "{bits} bits");
        }
    }
}

#[test]
fn the_word_wise_bound_test_matches_a_cell_by_cell_scan() {
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for bits in 1..=16u32 {
        let width = 1u32 << bits;
        for entry_count in [1usize, 2, 63, 64, 65, 100, 4096] {
            let cells: Vec<u16> = (0..entry_count)
                .map(|_| (next() % width as u64) as u16)
                .collect();
            let packed = pack_from(bits, &cells, |&c| c as u32);
            for limit in [1usize, 2, 3, (width / 2) as usize, width as usize] {
                let naive = cells.iter().any(|&c| c as usize >= limit);
                assert_eq!(
                    any_entry_past(bits, &packed, entry_count, limit),
                    naive,
                    "{bits} bits, {entry_count} entries, limit {limit}"
                );
                assert_eq!(
                    first_entry_past(bits, &packed, entry_count, limit).is_some(),
                    naive,
                    "{bits} bits, {entry_count} entries, limit {limit}"
                );
            }
        }
    }
}

/// The tail of the last word holds no cell, so a stray value there is not one.
#[test]
fn padding_past_the_last_cell_is_not_an_entry() {
    let entry_count = 10;
    let mut packed = pack_from(3, &vec![1u16; entry_count], |&c| c as u32);
    let last = packed.len() - 1;
    packed[last] |= 0b111 << (3 * (entry_count % entries_per_long(3)));
    assert!(!any_entry_past(3, &packed, entry_count, 2));
    assert_eq!(first_entry_past(3, &packed, entry_count, 2), None);
}

#[test]
fn first_entry_past_finds_the_earliest_offender() {
    let mut cells = vec![0u16; 64];
    cells[9] = 5;
    cells[40] = 6;
    let packed = pack_from(4, &cells, |&c| c as u32);
    assert_eq!(first_entry_past(4, &packed, 64, 2), Some(5));
    assert_eq!(first_entry_past(4, &packed, 64, 7), None);
}

/// A random array cannot tell a check of every lane from a check of half of
/// them: with one offender per lane position, it can.
#[test]
fn a_single_out_of_range_entry_is_found_wherever_it_sits() {
    for bits in 1..=16u32 {
        let limit = 1usize << (bits - 1);
        let per_long = entries_per_long(bits);
        let entry_count = 3 * per_long + 5;
        for position in 0..entry_count {
            let mut cells = vec![0u16; entry_count];
            cells[position] = limit as u16;
            let packed = pack_from(bits, &cells, |&c| c as u32);
            assert!(
                any_entry_past(bits, &packed, entry_count, limit),
                "{bits} bits, cell {position}"
            );
            assert_eq!(
                first_entry_past(bits, &packed, entry_count, limit),
                Some(limit as u16),
                "{bits} bits, cell {position}"
            );
            assert!(
                !any_entry_past(bits, &packed, entry_count, limit + 1),
                "{bits} bits, cell {position}"
            );
        }
    }
}

/// The O(1) split of a homogeneous cube must produce exactly what a full
/// `from_cube` scan of the same cells would: same palette order, same counts.
#[test]
fn splitting_a_homogeneous_cube_matches_a_full_scan() {
    type C = PalettedContainer<u16, 16>;
    const V: usize = C::VOLUME;

    let reference = |cells: &[u16]| {
        let c = C::from_cells(cells);
        match c {
            PalettedContainer::Homogeneous(v) => (vec![v], vec![V as u16]),
            PalettedContainer::Heterogeneous(d) => (d.palette.clone(), d.counts.clone()),
        }
    };
    let actual = |c: &C| match c {
        PalettedContainer::Homogeneous(v) => (vec![*v], vec![V as u16]),
        PalettedContainer::Heterogeneous(d) => (d.palette.clone(), d.counts.clone()),
    };

    for &(x, y, z) in &[
        (0usize, 0usize, 0usize),
        (1, 0, 0),
        (0, 1, 0),
        (5, 9, 13),
        (15, 15, 15),
    ] {
        let mut cells = vec![7u16; V];
        cells[y * 256 + z * 16 + x] = 42;
        let mut c = C::Homogeneous(7);
        c.set(x, y, z, 42);
        assert_eq!(actual(&c), reference(&cells), "set at {x},{y},{z}");
    }

    for &(x0, x1, y0, y1, z0, z1) in &[
        (0usize, 4usize, 0usize, 4usize, 0usize, 4usize),
        (2, 16, 3, 9, 0, 16),
        (0, 16, 0, 16, 0, 16),
    ] {
        let mut cells = vec![7u16; V];
        for y in y0..y1 {
            for z in z0..z1 {
                for x in x0..x1 {
                    cells[y * 256 + z * 16 + x] = 42;
                }
            }
        }
        let mut c = C::Homogeneous(7);
        c.fill_box(x0, x1, y0, y1, z0, z1, 42);
        assert_eq!(actual(&c), reference(&cells), "fill_box {x0}..{x1}");
    }
}
