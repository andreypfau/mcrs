use super::*;
use PaletteForm::{Direct, Indirect, Single};

#[test]
fn the_width_table_matches_the_strategy_configurations() {
    let blocks = [
        (1, 0, Single),
        (2, 4, Indirect { bits: 4 }),
        (3, 4, Indirect { bits: 4 }),
        (4, 4, Indirect { bits: 4 }),
        (5, 4, Indirect { bits: 4 }),
        (16, 4, Indirect { bits: 4 }),
        (17, 5, Indirect { bits: 5 }),
        (256, 8, Indirect { bits: 8 }),
        (257, 9, Direct { bits: 15 }),
    ];
    for (len, storage, form) in blocks {
        assert_eq!(Blocks::storage_bits(len), storage, "block palette of {len}");
        assert_eq!(Blocks::network_form(len), form, "block palette of {len}");
    }

    let biomes = [
        (1, 0, Single),
        (2, 1, Indirect { bits: 1 }),
        (3, 2, Indirect { bits: 2 }),
        (4, 2, Indirect { bits: 2 }),
        (5, 3, Indirect { bits: 3 }),
        (8, 3, Indirect { bits: 3 }),
        (9, 4, Direct { bits: 7 }),
    ];
    for (len, storage, form) in biomes {
        assert_eq!(Biomes::storage_bits(len), storage, "biome palette of {len}");
        assert_eq!(Biomes::network_form(len), form, "biome palette of {len}");
    }
}

#[test]
fn packing_and_unpacking_are_inverses() {
    for bits in 1..=16u32 {
        let cells: Vec<u16> = (0..Blocks::ENTRY_COUNT)
            .map(|i| (i as u16).wrapping_mul(2654) & ((1u32 << bits) - 1) as u16)
            .collect();
        let packed = pack_from(bits, &cells, |&c| c as u32);
        assert_eq!(packed.len(), packed_len(bits, cells.len()));

        let mut out = vec![0u16; Blocks::ENTRY_COUNT];
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
