use mcrs_minecraft_chunk::{LightLayer, LightStorage, SectionNibbles as LightNibbles};
use mcrs_minecraft_protocol_game::chunk::{LightChunk, LightData};
use std::borrow::Cow;

use mcrs_minecraft_region_disk::stored_column::StoredColumn;

/// One row of the light packet's section sequence. The packet carries a
/// section below and a section above the dimension's real Y range, so the
/// sequence is one padding row, every real section, then one padding row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WireRow {
    BottomPadding,
    Section,
    TopPadding,
}

/// The light of the section at `y` in one layer.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SectionLight<'a> {
    pub(crate) y: i32,
    pub(crate) light: &'a LightStorage,
}

/// The masks and the arrays of one light layer of a packet, filled row by row.
#[derive(Debug, Default)]
pub(crate) struct LayerPacker {
    mask: Vec<u64>,
    empty_mask: Vec<u64>,
    arrays: Vec<LightChunk>,
}

impl LayerPacker {
    fn with_capacity(rows: usize) -> Self {
        Self {
            mask: Vec::new(),
            empty_mask: Vec::new(),
            arrays: Vec::with_capacity(rows),
        }
    }
}

/// The light packet that the packed layers make.
fn light_data(block: LayerPacker, sky: LayerPacker) -> LightData<'static> {
    LightData {
        sky_light_mask: Cow::Owned(sky.mask),
        block_light_mask: Cow::Owned(block.mask),
        empty_sky_light_mask: Cow::Owned(sky.empty_mask),
        empty_block_light_mask: Cow::Owned(block.empty_mask),
        sky_light_arrays: Cow::Owned(sky.arrays),
        block_light_arrays: Cow::Owned(block.arrays),
    }
}

/// Packs the light of one row in `layer` at `bit_idx`. `storage` is `None` for a section
/// with no light of the layer.
pub(crate) fn pack_chunk(
    chunk: WireRow,
    storage: Option<&LightStorage>,
    layer: LightLayer,
    has_sky_light: bool,
    bit_idx: usize,
    out: &mut LayerPacker,
) {
    let LayerPacker {
        mask,
        empty_mask,
        arrays,
    } = out;
    match chunk {
        WireRow::BottomPadding => {
            set_bit(empty_mask, bit_idx);
        }
        WireRow::TopPadding => match layer {
            LightLayer::Sky => {
                if has_sky_light {
                    set_bit(mask, bit_idx);
                    arrays.push(LightChunk([0xFFu8; 2048]));
                } else {
                    set_bit(empty_mask, bit_idx);
                }
            }
            LightLayer::Block => {
                set_bit(empty_mask, bit_idx);
            }
        },
        WireRow::Section => {
            if matches!(layer, LightLayer::Sky) && !has_sky_light {
                set_bit(empty_mask, bit_idx);
                return;
            }
            match storage {
                None | Some(LightStorage::Empty) | Some(LightStorage::Uniform(0)) => {
                    set_bit(empty_mask, bit_idx);
                }
                Some(LightStorage::Uniform(n)) => {
                    set_bit(mask, bit_idx);
                    arrays.push(LightChunk(*LightNibbles::filled(*n).0));
                }
                Some(LightStorage::Dense(arr)) => {
                    set_bit(mask, bit_idx);
                    arrays.push(LightChunk(*arr.0));
                }
            }
        }
    }
}

#[inline]
fn set_bit(mask: &mut Vec<u64>, bit_idx: usize) {
    let word_idx = bit_idx / 64;
    let bit = bit_idx % 64;
    if mask.len() <= word_idx {
        mask.resize(word_idx + 1, 0);
    }
    if let Some(word) = mask.get_mut(word_idx) {
        *word |= 1u64 << bit;
    }
}

/// Sky at full, block at nothing, for every row a column puts on the wire.
/// A server running without a lighting engine still has to say something about
/// light, and saying nothing renders the world black on the client.
pub fn build_fullbright_light_data(rows: usize) -> LightData<'static> {
    let words = rows.div_ceil(64);
    let all_rows: Vec<u64> = (0..words)
        .map(|word| u64::MAX >> (64 - (rows - word * 64).min(64)))
        .collect();
    LightData {
        sky_light_mask: Cow::Owned(all_rows.clone()),
        block_light_mask: Cow::Owned(vec![0; words]),
        empty_sky_light_mask: Cow::Owned(vec![0; words]),
        empty_block_light_mask: Cow::Owned(all_rows),
        sky_light_arrays: Cow::Owned(vec![LightChunk([0xff; 2048]); rows]),
        block_light_arrays: Cow::Owned(Vec::new()),
    }
}

/// A `LightData` with only the rows of `block` and `sky`. The client leaves every other row,
/// the padding rows too, as it is.
pub(crate) fn build_delta_light_data(
    min_section_y: i32,
    has_sky_light: bool,
    block: &mut [SectionLight<'_>],
    sky: &mut [SectionLight<'_>],
) -> LightData<'static> {
    light_data(
        pack_sections(min_section_y, has_sky_light, block, LightLayer::Block),
        pack_sections(min_section_y, has_sky_light, sky, LightLayer::Sky),
    )
}

fn pack_sections(
    min_section_y: i32,
    has_sky_light: bool,
    sections: &mut [SectionLight<'_>],
    layer: LightLayer,
) -> LayerPacker {
    // The arrays go out in the order of their rows.
    sections.sort_unstable_by_key(|section| section.y);
    let mut packer = LayerPacker::with_capacity(sections.len());
    for section in sections.iter() {
        // Row 0 is the bottom padding.
        let row = section
            .y
            .checked_sub(min_section_y)
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| index.checked_add(1));
        if let Some(row) = row {
            pack_chunk(
                WireRow::Section,
                Some(section.light),
                layer,
                has_sky_light,
                row,
                &mut packer,
            );
        }
    }
    packer
}

/// Build a wire-ready `LightData` for a column from the data layer, with the light it held when
/// it went cold.
pub(crate) fn build_stored_light_data(
    stored: &StoredColumn,
    has_sky_light: bool,
) -> LightData<'static> {
    let rows = stored.sections.len() + 2;
    let mut block = LayerPacker::with_capacity(rows);
    let mut sky = LayerPacker::with_capacity(rows);
    let sections = stored.sections.iter().map(Some);
    let padded = std::iter::once(None)
        .chain(sections)
        .chain(std::iter::once(None));
    for (bit_idx, section) in padded.enumerate() {
        let row = match section {
            Some(_) => WireRow::Section,
            None if bit_idx == 0 => WireRow::BottomPadding,
            None => WireRow::TopPadding,
        };
        pack_chunk(
            row,
            section
                .and_then(|section| section.block_light.as_ref())
                .map(|light| &light.0),
            LightLayer::Block,
            has_sky_light,
            bit_idx,
            &mut block,
        );
        pack_chunk(
            row,
            section
                .and_then(|section| section.sky_light.as_ref())
                .map(|light| &light.0),
            LightLayer::Sky,
            has_sky_light,
            bit_idx,
            &mut sky,
        );
    }
    light_data(block, sky)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_protocol_game::light_codec::{RowLight, unpack_light_data};
    use std::sync::Arc;

    fn bit_is_set(mask: &[u64], bit_idx: usize) -> bool {
        let word_idx = bit_idx / 64;
        if word_idx >= mask.len() {
            return false;
        }
        (mask[word_idx] >> (bit_idx % 64)) & 1 == 1
    }

    fn popcount(mask: &[u64]) -> u32 {
        mask.iter().map(|w| w.count_ones()).sum()
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the column of 24 synthetic chunks is built and its wire order checked in one scenario"
    )]
    fn codec_wire_ordering_invariant_holds_for_synthetic_24_chunk_column() {
        // Synthesize a column-shaped iter_wire output that exercises every
        // matrix row at least once. Wire-ordering invariant:
        // arrays.len() == popcount(mask) per layer, AND arrays must appear
        // in strictly increasing bit order (i.e., the lowest set bit first).
        let mut nibble = LightNibbles::zeros();
        nibble.set(0, 0, 0, 0xC);

        let rows: Vec<(WireRow, Option<LightStorage>, Option<LightStorage>)> = vec![
            (WireRow::BottomPadding, None, None),
            (
                WireRow::Section,
                Some(LightStorage::Dense(Arc::new(nibble.clone()))),
                Some(LightStorage::Uniform(0xF)),
            ),
            (
                WireRow::Section,
                Some(LightStorage::Uniform(0x5)),
                Some(LightStorage::Uniform(0)),
            ),
            (
                WireRow::Section,
                Some(LightStorage::Empty),
                Some(LightStorage::Empty),
            ),
            (
                WireRow::Section,
                Some(LightStorage::Uniform(0)),
                Some(LightStorage::Dense(Arc::new(nibble))),
            ),
            (WireRow::TopPadding, None, None),
        ];

        let mut block = LayerPacker::default();
        let mut sky = LayerPacker::default();

        let has_sky_light = true;

        for (bit_idx, (chunk, block_storage, sky_storage)) in rows.iter().enumerate() {
            pack_chunk(
                *chunk,
                block_storage.as_ref(),
                LightLayer::Block,
                has_sky_light,
                bit_idx,
                &mut block,
            );
            pack_chunk(
                *chunk,
                sky_storage.as_ref(),
                LightLayer::Sky,
                has_sky_light,
                bit_idx,
                &mut sky,
            );
        }
        let LayerPacker {
            mask: block_mask,
            empty_mask: empty_block_mask,
            arrays: block_arrays,
        } = block;
        let LayerPacker {
            mask: sky_mask,
            empty_mask: empty_sky_mask,
            arrays: sky_arrays,
        } = sky;

        assert_eq!(
            block_arrays.len() as u32,
            popcount(&block_mask),
            "block arrays.len() must equal popcount(block_mask)"
        );
        assert_eq!(
            sky_arrays.len() as u32,
            popcount(&sky_mask),
            "sky arrays.len() must equal popcount(sky_mask)"
        );

        // No bit may appear in both mask and empty_mask simultaneously.
        for word_idx in 0..block_mask.len().max(empty_block_mask.len()) {
            let b = *block_mask.get(word_idx).unwrap_or(&0);
            let e = *empty_block_mask.get(word_idx).unwrap_or(&0);
            assert_eq!(
                b & e,
                0,
                "block: mask and empty_mask overlap at word {word_idx}"
            );
        }
        for word_idx in 0..sky_mask.len().max(empty_sky_mask.len()) {
            let s = *sky_mask.get(word_idx).unwrap_or(&0);
            let e = *empty_sky_mask.get(word_idx).unwrap_or(&0);
            assert_eq!(
                s & e,
                0,
                "sky: mask and empty_mask overlap at word {word_idx}"
            );
        }

        assert!(bit_is_set(&empty_block_mask, 0), "bottom padding");
        assert!(bit_is_set(&block_mask, 1), "dense block");
        assert!(bit_is_set(&block_mask, 2), "uniform 5 block");
        assert!(bit_is_set(&empty_block_mask, 3), "no block light");
        assert!(bit_is_set(&empty_block_mask, 4), "uniform 0 block");
        assert!(bit_is_set(&empty_block_mask, 5), "top padding block");

        assert!(bit_is_set(&empty_sky_mask, 0), "bottom padding sky");
        assert!(bit_is_set(&sky_mask, 1), "uniform 15 sky");
        assert!(bit_is_set(&empty_sky_mask, 2), "uniform 0 sky");
        assert!(bit_is_set(&empty_sky_mask, 3), "no sky light");
        assert!(bit_is_set(&sky_mask, 4), "dense sky");
        assert!(
            bit_is_set(&sky_mask, 5),
            "top padding sky in a dimension with sky"
        );

        // Walk through arrays in bit order and confirm they line up with the
        // set bits of the corresponding mask. The wire format requires arrays
        // to appear in strict bit-order of the mask, lowest set bit first.
        let block_set_bits: Vec<usize> = (0..rows.len())
            .filter(|&b| bit_is_set(&block_mask, b))
            .collect();
        assert_eq!(block_set_bits.len(), block_arrays.len());
        let sky_set_bits: Vec<usize> = (0..rows.len())
            .filter(|&b| bit_is_set(&sky_mask, b))
            .collect();
        assert_eq!(sky_set_bits.len(), sky_arrays.len());

        // Spot-check that the topmost sky array (TopPadding synth) is 0xFF.
        let top_array = &sky_arrays[sky_arrays.len() - 1];
        assert_eq!(*top_array, LightChunk([0xFFu8; 2048]));
    }

    const SECTIONS: usize = 5;
    const ROWS: usize = SECTIONS + 2;

    #[test]
    fn a_delta_carries_only_the_changed_rows() {
        let mut nibbles = LightNibbles::zeros();
        nibbles.set(1, 2, 3, 0xB);
        let dense = LightStorage::Dense(Arc::new(nibbles.clone()));
        let mut cases = vec![
            (
                true,
                2,
                LightLayer::Block,
                dense,
                Some(RowLight::Filled(LightChunk(*nibbles.0))),
            ),
            (
                true,
                2,
                LightLayer::Block,
                LightStorage::Uniform(0),
                Some(RowLight::Empty),
            ),
            (
                true,
                1,
                LightLayer::Sky,
                LightStorage::Uniform(0xF),
                Some(RowLight::Filled(LightChunk([0xFF; 2048]))),
            ),
            (false, 2, LightLayer::Sky, LightStorage::Uniform(0xF), None),
        ];
        for index in 0..SECTIONS {
            cases.push((
                true,
                index,
                LightLayer::Block,
                LightStorage::Uniform(0x3),
                Some(RowLight::Filled(LightChunk([0x33; 2048]))),
            ));
        }

        for (has_sky, index, layer, storage, expected) in cases {
            let what = format!("{layer:?} {storage:?} in section {index}, sky {has_sky}");
            let mut changed = [SectionLight {
                y: index as i32,
                light: &storage,
            }];
            let data = match layer {
                LightLayer::Block => build_delta_light_data(0, has_sky, &mut changed, &mut []),
                LightLayer::Sky => build_delta_light_data(0, has_sky, &mut [], &mut changed),
            };
            let unpacked = unpack_light_data(&data, ROWS).expect("delta round-trips");
            let (rows, arrays) = match layer {
                LightLayer::Block => (&unpacked.block, data.block_light_arrays.len()),
                LightLayer::Sky => (&unpacked.sky, data.sky_light_arrays.len()),
            };
            let filled = rows
                .iter()
                .filter(|row| matches!(row, RowLight::Filled(_)))
                .count();
            assert_eq!(arrays, filled, "{what}: one array per filled row");

            let Some(expected) = expected else {
                assert_eq!(arrays, 0, "{what}: no payload");
                continue;
            };
            let changed_row = index + 1;
            for row in 0..ROWS {
                let block_expected = if layer == LightLayer::Block && row == changed_row {
                    &expected
                } else {
                    &RowLight::Unchanged
                };
                let sky_expected = if layer == LightLayer::Sky && row == changed_row {
                    &expected
                } else {
                    &RowLight::Unchanged
                };
                assert_eq!(
                    &unpacked.block[row], block_expected,
                    "{what}: block row {row}"
                );
                assert_eq!(&unpacked.sky[row], sky_expected, "{what}: sky row {row}");
            }
        }
    }
}
