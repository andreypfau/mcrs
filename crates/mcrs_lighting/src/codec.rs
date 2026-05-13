//! Lighting wire codec.
//!
//! Pure transformation core that converts per-section `BlockLight` / `SkyLight`
//! state into the protocol's `LightData` payload. Two public entry points:
//!
//! 1. `pack_section` — the per-section, per-layer wire-mapping decision matrix.
//!    Given a `SectionLookup` row (Loaded / Unloaded / BottomPadding /
//!    TopPadding / OutOfRange) and the optional `LightStorage` for the
//!    requested `Layer`, it updates the four wire masks (`*_light_mask` and
//!    `empty_*_light_mask`) and may append a 2048-byte payload to the matching
//!    arrays builder.
//!
//! 2. `build_full_light_data` — iterates `SectionIndex::iter_wire()` for a
//!    column entity, dispatches `pack_section` per row per layer, and returns
//!    a wire-ready `LightData<'static>` with `Cow::Owned` payloads.
//!
//! The codec is read-only against ECS state and allocates only the output
//! buffers (worst case 24 sections × 2 layers × 2048 bytes = 96 KB per column).
//! The `'static` lifetime on the returned `LightData` is required because
//! downstream `Message<T>` types must be `Send + Sync + 'static`.

use bevy_ecs::prelude::{Entity, Query, With};
use bevy_ecs::system::SystemParam;
use mcrs_engine::world::column::{SectionIndex, SectionLookup};
use mcrs_engine::world::dimension::{HasSkyLight, InDimension};
use mcrs_protocol::chunk::LightData;
use std::borrow::Cow;

use crate::components::{BlockLight, SkyLight};
use crate::storage::LightStorage;

/// Which light layer a `pack_section` call is operating on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    Block,
    Sky,
}

/// Wire-mapping decision matrix dispatcher for a single (section, layer) pair.
///
/// The bit `bit_idx` is interpreted relative to the mask `Vec<u64>` words:
/// the bit is at position `bit_idx % 64` within word `bit_idx / 64`. Masks are
/// grown on demand so the highest touched word is always present.
///
/// `storage` is `None` only when the parent dimension has no sky-light and the
/// layer is `Sky`; in every other Loaded case the caller supplies the
/// component's `LightStorage`. The `has_sky_light` flag is consulted only for
/// `TopPadding` (sky synthesis is gated on the dimension having a sky) and for
/// the Loaded+sky-missing-in-skyless-dim row.
#[allow(clippy::too_many_arguments)]
pub fn pack_section(
    section: SectionLookup,
    storage: Option<&LightStorage>,
    layer: Layer,
    has_sky_light: bool,
    bit_idx: usize,
    mask: &mut Vec<u64>,
    empty_mask: &mut Vec<u64>,
    arrays: &mut Vec<[u8; 2048]>,
) {
    match section {
        SectionLookup::BottomPadding => {
            set_bit(empty_mask, bit_idx);
        }
        SectionLookup::TopPadding => match layer {
            Layer::Sky => {
                if has_sky_light {
                    set_bit(mask, bit_idx);
                    arrays.push([0xFFu8; 2048]);
                } else {
                    set_bit(empty_mask, bit_idx);
                }
            }
            Layer::Block => {
                set_bit(empty_mask, bit_idx);
            }
        },
        SectionLookup::Loaded(_) => {
            if matches!(layer, Layer::Sky) && !has_sky_light {
                set_bit(empty_mask, bit_idx);
                return;
            }
            match storage {
                None | Some(LightStorage::Null) | Some(LightStorage::Uniform(0)) => {
                    set_bit(empty_mask, bit_idx);
                }
                Some(LightStorage::Uniform(n)) => {
                    set_bit(mask, bit_idx);
                    let packed = *n | (*n << 4);
                    arrays.push([packed; 2048]);
                }
                Some(LightStorage::Mixed(arr)) => {
                    set_bit(mask, bit_idx);
                    arrays.push(*arr.0);
                }
            }
        }
        SectionLookup::Unloaded => {
            // Neither mask bit is set — vanilla treats unloaded sections as
            // "absent from the column" rather than "present but empty". The
            // bit index still advances in the outer iterator so wire ordering
            // stays aligned with `SectionIndex::iter_wire()` indices.
        }
        SectionLookup::OutOfRange => {
            debug_assert!(
                false,
                "SectionIndex::iter_wire never yields OutOfRange; codec invariant violated"
            );
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
    mask[word_idx] |= 1u64 << bit;
}

#[derive(SystemParam)]
pub struct LightCodecParams<'w, 's> {
    pub section_indexes: Query<'w, 's, &'static SectionIndex>,
    pub block_lights: Query<'w, 's, &'static BlockLight>,
    pub sky_lights: Query<'w, 's, &'static SkyLight>,
    pub in_dimensions: Query<'w, 's, &'static InDimension>,
    pub has_sky_lights: Query<'w, 's, (), With<HasSkyLight>>,
}

/// Build a wire-ready `LightData` for the given column entity.
///
/// Returns `LightData::default()` if the column or its parent dimension is
/// missing the required components — callers should treat that as an
/// "ignore this column for now" signal rather than an error, since the
/// reconcile lifecycle may not yet have attached state.
pub fn build_full_light_data(
    column_entity: Entity,
    params: &LightCodecParams,
) -> LightData<'static> {
    let Ok(section_index) = params.section_indexes.get(column_entity) else {
        return LightData::default();
    };
    let Ok(in_dim) = params.in_dimensions.get(column_entity) else {
        return LightData::default();
    };
    let has_sky_light = params.has_sky_lights.get(in_dim.0).is_ok();

    let mut sky_mask: Vec<u64> = Vec::new();
    let mut block_mask: Vec<u64> = Vec::new();
    let mut empty_sky_mask: Vec<u64> = Vec::new();
    let mut empty_block_mask: Vec<u64> = Vec::new();
    let mut sky_arrays: Vec<[u8; 2048]> = Vec::new();
    let mut block_arrays: Vec<[u8; 2048]> = Vec::new();

    for (bit_idx, lookup) in section_index.iter_wire().enumerate() {
        let section_entity = match lookup {
            SectionLookup::Loaded(e) => Some(e),
            _ => None,
        };
        let block_storage = section_entity
            .and_then(|e| params.block_lights.get(e).ok())
            .map(|bl| &bl.0);
        let sky_storage = section_entity
            .and_then(|e| params.sky_lights.get(e).ok())
            .map(|sl| &sl.0);

        pack_section(
            lookup,
            block_storage,
            Layer::Block,
            has_sky_light,
            bit_idx,
            &mut block_mask,
            &mut empty_block_mask,
            &mut block_arrays,
        );
        pack_section(
            lookup,
            sky_storage,
            Layer::Sky,
            has_sky_light,
            bit_idx,
            &mut sky_mask,
            &mut empty_sky_mask,
            &mut sky_arrays,
        );
    }

    LightData {
        sky_light_mask: Cow::Owned(sky_mask),
        block_light_mask: Cow::Owned(block_mask),
        empty_sky_light_mask: Cow::Owned(empty_sky_mask),
        empty_block_light_mask: Cow::Owned(empty_block_mask),
        sky_light_arrays: Cow::Owned(sky_arrays),
        block_light_arrays: Cow::Owned(block_arrays),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nibble::NibbleArray;
    use bevy_ecs::entity::Entity;

    fn fake_entity(index: u32) -> Entity {
        Entity::from_raw_u32(index + 1).expect("valid entity index")
    }

    fn fresh_buffers() -> (Vec<u64>, Vec<u64>, Vec<[u8; 2048]>) {
        (Vec::new(), Vec::new(), Vec::new())
    }

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
    fn pack_section_bottom_padding_sets_both_empty_masks() {
        // Block layer.
        let (mut mask, mut empty_mask, mut arrays) = fresh_buffers();
        pack_section(
            SectionLookup::BottomPadding,
            None,
            Layer::Block,
            true,
            0,
            &mut mask,
            &mut empty_mask,
            &mut arrays,
        );
        assert!(!bit_is_set(&mask, 0), "block mask bit must NOT be set");
        assert!(bit_is_set(&empty_mask, 0), "empty block mask bit must be set");
        assert!(arrays.is_empty(), "no block array appended");

        // Sky layer (independent of has_sky_light per the matrix).
        for sky in [false, true] {
            let (mut mask, mut empty_mask, mut arrays) = fresh_buffers();
            pack_section(
                SectionLookup::BottomPadding,
                None,
                Layer::Sky,
                sky,
                0,
                &mut mask,
                &mut empty_mask,
                &mut arrays,
            );
            assert!(!bit_is_set(&mask, 0));
            assert!(bit_is_set(&empty_mask, 0));
            assert!(arrays.is_empty());
        }
    }

    #[test]
    fn pack_section_loaded_mixed_block_sets_block_mask_and_appends_array() {
        let mut nibble = NibbleArray::zeros();
        nibble.set(3, 7, 11, 0xA);
        let storage = LightStorage::Mixed(Box::new(nibble.clone()));

        let (mut mask, mut empty_mask, mut arrays) = fresh_buffers();
        pack_section(
            SectionLookup::Loaded(fake_entity(1)),
            Some(&storage),
            Layer::Block,
            false, // has_sky_light irrelevant for block layer
            3,
            &mut mask,
            &mut empty_mask,
            &mut arrays,
        );

        assert!(bit_is_set(&mask, 3));
        assert!(!bit_is_set(&empty_mask, 3));
        assert_eq!(arrays.len(), 1);
        assert_eq!(arrays[0], *nibble.0, "appended bytes must equal Mixed payload");
    }

    #[test]
    fn pack_section_loaded_uniform_zero_block_sets_empty_block_mask() {
        let storage = LightStorage::Uniform(0);
        let (mut mask, mut empty_mask, mut arrays) = fresh_buffers();
        pack_section(
            SectionLookup::Loaded(fake_entity(2)),
            Some(&storage),
            Layer::Block,
            true,
            5,
            &mut mask,
            &mut empty_mask,
            &mut arrays,
        );
        assert!(!bit_is_set(&mask, 5));
        assert!(bit_is_set(&empty_mask, 5));
        assert!(arrays.is_empty());
    }

    #[test]
    fn pack_section_loaded_uniform_nonzero_block_sets_block_mask_and_synthesizes_payload() {
        let storage = LightStorage::Uniform(0x7);
        let (mut mask, mut empty_mask, mut arrays) = fresh_buffers();
        pack_section(
            SectionLookup::Loaded(fake_entity(3)),
            Some(&storage),
            Layer::Block,
            true,
            65, // exercises the second mask word
            &mut mask,
            &mut empty_mask,
            &mut arrays,
        );
        assert!(bit_is_set(&mask, 65));
        assert!(!bit_is_set(&empty_mask, 65));
        assert_eq!(arrays.len(), 1);
        let expected = [0x77u8; 2048];
        assert_eq!(arrays[0], expected);
        assert!(mask.len() >= 2, "mask must grow to cover bit 65");
    }

    #[test]
    fn pack_section_loaded_null_block_sets_empty_block_mask() {
        let storage = LightStorage::Null;
        let (mut mask, mut empty_mask, mut arrays) = fresh_buffers();
        pack_section(
            SectionLookup::Loaded(fake_entity(4)),
            Some(&storage),
            Layer::Block,
            true,
            12,
            &mut mask,
            &mut empty_mask,
            &mut arrays,
        );
        assert!(!bit_is_set(&mask, 12));
        assert!(bit_is_set(&empty_mask, 12));
        assert!(arrays.is_empty());
    }

    #[test]
    fn pack_section_loaded_skyless_dim_sets_empty_sky_mask() {
        let storage = LightStorage::Uniform(0xF);
        let (mut mask, mut empty_mask, mut arrays) = fresh_buffers();
        pack_section(
            SectionLookup::Loaded(fake_entity(5)),
            Some(&storage),
            Layer::Sky,
            false, // skyless dimension
            8,
            &mut mask,
            &mut empty_mask,
            &mut arrays,
        );
        assert!(!bit_is_set(&mask, 8), "sky mask must NOT be set in skyless dim");
        assert!(bit_is_set(&empty_mask, 8), "empty sky mask must be set");
        assert!(arrays.is_empty(), "no sky payload in skyless dim");

        // Same row with storage = None (component absent on the section)
        // must reach the same result.
        let (mut mask, mut empty_mask, mut arrays) = fresh_buffers();
        pack_section(
            SectionLookup::Loaded(fake_entity(5)),
            None,
            Layer::Sky,
            false,
            8,
            &mut mask,
            &mut empty_mask,
            &mut arrays,
        );
        assert!(!bit_is_set(&mask, 8));
        assert!(bit_is_set(&empty_mask, 8));
        assert!(arrays.is_empty());
    }

    #[test]
    fn pack_section_unloaded_sets_no_mask_bit() {
        for layer in [Layer::Block, Layer::Sky] {
            for has_sky in [false, true] {
                let (mut mask, mut empty_mask, mut arrays) = fresh_buffers();
                pack_section(
                    SectionLookup::Unloaded,
                    None,
                    layer,
                    has_sky,
                    4,
                    &mut mask,
                    &mut empty_mask,
                    &mut arrays,
                );
                assert!(!bit_is_set(&mask, 4), "{layer:?}/{has_sky}: mask bit set");
                assert!(
                    !bit_is_set(&empty_mask, 4),
                    "{layer:?}/{has_sky}: empty mask bit set"
                );
                assert!(arrays.is_empty());
            }
        }
    }

    #[test]
    fn pack_section_top_padding_sky_having_sets_sky_mask_and_appends_0xff() {
        let (mut mask, mut empty_mask, mut arrays) = fresh_buffers();
        pack_section(
            SectionLookup::TopPadding,
            None,
            Layer::Sky,
            true,
            25,
            &mut mask,
            &mut empty_mask,
            &mut arrays,
        );
        assert!(bit_is_set(&mask, 25), "sky mask bit must be set");
        assert!(!bit_is_set(&empty_mask, 25));
        assert_eq!(arrays.len(), 1);
        assert_eq!(arrays[0], [0xFFu8; 2048]);

        // Block layer at TopPadding in a sky-having dim still goes to the
        // empty mask — only the sky layer synthesizes the 0xFF payload.
        let (mut mask, mut empty_mask, mut arrays) = fresh_buffers();
        pack_section(
            SectionLookup::TopPadding,
            None,
            Layer::Block,
            true,
            25,
            &mut mask,
            &mut empty_mask,
            &mut arrays,
        );
        assert!(!bit_is_set(&mask, 25));
        assert!(bit_is_set(&empty_mask, 25));
        assert!(arrays.is_empty());
    }

    #[test]
    fn pack_section_top_padding_skyless_sets_both_empty_masks() {
        // Sky layer.
        let (mut mask, mut empty_mask, mut arrays) = fresh_buffers();
        pack_section(
            SectionLookup::TopPadding,
            None,
            Layer::Sky,
            false,
            17,
            &mut mask,
            &mut empty_mask,
            &mut arrays,
        );
        assert!(!bit_is_set(&mask, 17));
        assert!(bit_is_set(&empty_mask, 17));
        assert!(arrays.is_empty());

        // Block layer.
        let (mut mask, mut empty_mask, mut arrays) = fresh_buffers();
        pack_section(
            SectionLookup::TopPadding,
            None,
            Layer::Block,
            false,
            17,
            &mut mask,
            &mut empty_mask,
            &mut arrays,
        );
        assert!(!bit_is_set(&mask, 17));
        assert!(bit_is_set(&empty_mask, 17));
        assert!(arrays.is_empty());
    }

    #[test]
    fn codec_wire_ordering_invariant_holds_for_synthetic_24_section_column() {
        // Synthesize a column-shaped iter_wire output that exercises every
        // matrix row at least once. Wire-ordering invariant:
        // arrays.len() == popcount(mask) per layer, AND arrays must appear
        // in strictly increasing bit order (i.e., the lowest set bit first).
        let mut nibble = NibbleArray::zeros();
        nibble.set(0, 0, 0, 0xC);

        let rows: Vec<(SectionLookup, Option<LightStorage>, Option<LightStorage>)> = vec![
            (SectionLookup::BottomPadding, None, None),
            (SectionLookup::Unloaded, None, None),
            (
                SectionLookup::Loaded(fake_entity(1)),
                Some(LightStorage::Mixed(Box::new(nibble.clone()))),
                Some(LightStorage::Uniform(0xF)),
            ),
            (
                SectionLookup::Loaded(fake_entity(2)),
                Some(LightStorage::Uniform(0x5)),
                Some(LightStorage::Uniform(0)),
            ),
            (
                SectionLookup::Loaded(fake_entity(3)),
                Some(LightStorage::Null),
                Some(LightStorage::Null),
            ),
            (
                SectionLookup::Loaded(fake_entity(4)),
                Some(LightStorage::Uniform(0)),
                Some(LightStorage::Mixed(Box::new(nibble))),
            ),
            (SectionLookup::Unloaded, None, None),
            (SectionLookup::TopPadding, None, None),
        ];

        let mut block_mask: Vec<u64> = Vec::new();
        let mut sky_mask: Vec<u64> = Vec::new();
        let mut empty_block_mask: Vec<u64> = Vec::new();
        let mut empty_sky_mask: Vec<u64> = Vec::new();
        let mut block_arrays: Vec<[u8; 2048]> = Vec::new();
        let mut sky_arrays: Vec<[u8; 2048]> = Vec::new();

        let has_sky_light = true;

        for (bit_idx, (section, block_storage, sky_storage)) in rows.iter().enumerate() {
            pack_section(
                *section,
                block_storage.as_ref(),
                Layer::Block,
                has_sky_light,
                bit_idx,
                &mut block_mask,
                &mut empty_block_mask,
                &mut block_arrays,
            );
            pack_section(
                *section,
                sky_storage.as_ref(),
                Layer::Sky,
                has_sky_light,
                bit_idx,
                &mut sky_mask,
                &mut empty_sky_mask,
                &mut sky_arrays,
            );
        }

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
            assert_eq!(s & e, 0, "sky: mask and empty_mask overlap at word {word_idx}");
        }

        // Verify the per-row expectations on the block layer.
        // Row 0 BottomPadding: block empty.
        assert!(bit_is_set(&empty_block_mask, 0));
        // Row 1 Unloaded: neither.
        assert!(!bit_is_set(&block_mask, 1));
        assert!(!bit_is_set(&empty_block_mask, 1));
        // Row 2 Mixed block: block mask set, array appended.
        assert!(bit_is_set(&block_mask, 2));
        // Row 3 Uniform(5) block: block mask set, synthesized payload.
        assert!(bit_is_set(&block_mask, 3));
        // Row 4 Null block: empty.
        assert!(bit_is_set(&empty_block_mask, 4));
        // Row 5 Uniform(0) block: empty.
        assert!(bit_is_set(&empty_block_mask, 5));
        // Row 7 TopPadding block: empty.
        assert!(bit_is_set(&empty_block_mask, 7));

        // Sky layer per-row checks.
        // Row 0 BottomPadding: sky empty.
        assert!(bit_is_set(&empty_sky_mask, 0));
        // Row 2 Uniform(0xF) sky: mask set, synthesized 0xFF payload.
        assert!(bit_is_set(&sky_mask, 2));
        // Row 3 Uniform(0) sky: empty.
        assert!(bit_is_set(&empty_sky_mask, 3));
        // Row 4 Null sky: empty.
        assert!(bit_is_set(&empty_sky_mask, 4));
        // Row 5 Mixed sky: mask set.
        assert!(bit_is_set(&sky_mask, 5));
        // Row 7 TopPadding sky in sky-having dim: mask set, 0xFF payload.
        assert!(bit_is_set(&sky_mask, 7));

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
        assert_eq!(*top_array, [0xFFu8; 2048]);
    }
}
