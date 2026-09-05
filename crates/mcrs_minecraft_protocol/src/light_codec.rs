//! Lighting wire codec.
//!
//! Pure transformation core that converts per-chunk `BlockLight` / `SkyLight`
//! state into the protocol's `LightData` payload. Two public entry points:
//!
//! 1. `pack_chunk` — the per-chunk, per-layer wire-mapping decision matrix.
//!    Given a `WireRow` (Loaded / Unloaded / BottomPadding / TopPadding) and
//!    the optional `LightStorage` for the requested `Layer`, it updates the
//!    four wire masks (`*_light_mask` and `empty_*_light_mask`) and may append
//!    a 2048-byte payload to the matching arrays builder.
//!
//! 2. `build_full_light_data` / `build_delta_light_data` — iterate `wire_rows`
//!    for a column entity, dispatch `pack_chunk` per row per layer, and return
//!    a wire-ready `LightData<'static>` with `Cow::Owned` payloads. They differ
//!    only in which rows they pack: a delta leaves the rest out of both masks,
//!    which the client reads as "unchanged".
//!
//! The codec is read-only against ECS state and allocates only the output
//! buffers (worst case 24 chunks × 2 layers × 2048 bytes = 96 KB per column).
//! The `'static` lifetime on the returned `LightData` is required because
//! downstream `Message<T>` types must be `Send + Sync + 'static`.

use crate::chunk::{LightChunk, LightData};
use anyhow::{Context, bail, ensure};
use bevy_ecs::prelude::{Entity, Query, With};
use bevy_ecs::system::SystemParam;
use mcrs_minecraft_light::storage::LightStorage;
use mcrs_minecraft_light::{BlockLight, SkyLight};
use mcrs_voxel_world::world::dimension::{HasSkyLight, InDimension};
use mcrs_voxel_world::world::storage::column::{ChunkLookup, ColumnChunks};
use std::borrow::Cow;

/// One row of the light packet's section sequence. The packet carries a
/// section below and a section above the dimension's real Y range, so the
/// sequence is one padding row, every real section, then one padding row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireRow {
    BottomPadding,
    Loaded(Entity),
    Unloaded,
    TopPadding,
}

/// The padded row sequence for a column, in wire order.
pub fn wire_rows(chunks: &ColumnChunks) -> impl Iterator<Item = WireRow> + '_ {
    std::iter::once(WireRow::BottomPadding)
        .chain(chunks.iter().map(|lookup| match lookup {
            ChunkLookup::Loaded(e) => WireRow::Loaded(e),
            _ => WireRow::Unloaded,
        }))
        .chain(std::iter::once(WireRow::TopPadding))
}

/// Which light layer a `pack_chunk` call is operating on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    Block,
    Sky,
}

/// Wire-mapping decision matrix dispatcher for a single (chunk, layer) pair.
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
pub fn pack_chunk(
    chunk: WireRow,
    storage: Option<&LightStorage>,
    layer: Layer,
    has_sky_light: bool,
    bit_idx: usize,
    mask: &mut Vec<u64>,
    empty_mask: &mut Vec<u64>,
    arrays: &mut Vec<LightChunk>,
) {
    match chunk {
        WireRow::BottomPadding => {
            set_bit(empty_mask, bit_idx);
        }
        WireRow::TopPadding => match layer {
            Layer::Sky => {
                if has_sky_light {
                    set_bit(mask, bit_idx);
                    arrays.push(LightChunk([0xFFu8; 2048]));
                } else {
                    set_bit(empty_mask, bit_idx);
                }
            }
            Layer::Block => {
                set_bit(empty_mask, bit_idx);
            }
        },
        WireRow::Loaded(_) => {
            if matches!(layer, Layer::Sky) && !has_sky_light {
                set_bit(empty_mask, bit_idx);
                return;
            }
            match storage {
                None | Some(LightStorage::Empty) | Some(LightStorage::Uniform(0)) => {
                    set_bit(empty_mask, bit_idx);
                }
                Some(LightStorage::Uniform(n)) => {
                    set_bit(mask, bit_idx);
                    let packed = *n | (*n << 4);
                    arrays.push(LightChunk([packed; 2048]));
                }
                Some(LightStorage::Dense(arr)) => {
                    set_bit(mask, bit_idx);
                    arrays.push(LightChunk(*arr.0));
                }
            }
        }
        WireRow::Unloaded => {
            // Neither mask bit is set — vanilla treats unloaded chunks as
            // "absent from the column" rather than "present but empty". The
            // bit index still advances in the outer iterator so wire ordering
            // stays aligned with the padded row indices.
        }
    }
}

/// What one wire row says about a layer. A row absent from both masks is not
/// "dark": vanilla means "unchanged", and a delta update leans on that.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RowLight {
    Unchanged,
    Empty,
    Filled(LightChunk),
}

pub struct ColumnLight {
    pub sky: Vec<RowLight>,
    pub block: Vec<RowLight>,
}

/// The inverse of `pack_chunk` over a whole column: `rows` is the padded row
/// count, one more than the dimension's section count at each end.
pub fn unpack_light_data(data: &LightData<'_>, rows: usize) -> anyhow::Result<ColumnLight> {
    Ok(ColumnLight {
        sky: unpack_layer(
            &data.sky_light_mask,
            &data.empty_sky_light_mask,
            &data.sky_light_arrays,
            rows,
        )
        .context("sky light")?,
        block: unpack_layer(
            &data.block_light_mask,
            &data.empty_block_light_mask,
            &data.block_light_arrays,
            rows,
        )
        .context("block light")?,
    })
}

fn unpack_layer(
    mask: &[u64],
    empty_mask: &[u64],
    arrays: &[LightChunk],
    rows: usize,
) -> anyhow::Result<Vec<RowLight>> {
    ensure!(
        !any_bit_from(mask, rows) && !any_bit_from(empty_mask, rows),
        "a mask bit is set past the column's {rows} rows"
    );

    let mut taken = 0usize;
    let layer = (0..rows)
        .map(|bit_idx| {
            Ok(
                match (bit_is_set(mask, bit_idx), bit_is_set(empty_mask, bit_idx)) {
                    (true, true) => bail!("row {bit_idx} is both populated and empty"),
                    (true, false) => {
                        let chunk = *arrays
                            .get(taken)
                            .with_context(|| format!("row {bit_idx} has no payload"))?;
                        taken += 1;
                        RowLight::Filled(chunk)
                    }
                    (false, true) => RowLight::Empty,
                    (false, false) => RowLight::Unchanged,
                },
            )
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    ensure!(
        taken == arrays.len(),
        "{} payloads left over after {taken} populated rows",
        arrays.len() - taken
    );
    Ok(layer)
}

fn bit_is_set(mask: &[u64], bit_idx: usize) -> bool {
    mask.get(bit_idx / 64)
        .is_some_and(|word| word >> (bit_idx % 64) & 1 == 1)
}

fn any_bit_from(mask: &[u64], bit_idx: usize) -> bool {
    let word_idx = bit_idx / 64;
    mask.get(word_idx)
        .is_some_and(|word| word >> (bit_idx % 64) != 0)
        || mask.len() > word_idx + 1 && mask[word_idx + 1..].iter().any(|word| *word != 0)
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
    pub chunk_indexes: Query<'w, 's, &'static ColumnChunks>,
    pub block_lights: Query<'w, 's, &'static BlockLight>,
    pub sky_lights: Query<'w, 's, &'static SkyLight>,
    pub in_dimensions: Query<'w, 's, &'static InDimension>,
    pub has_sky_lights: Query<'w, 's, (), With<HasSkyLight>>,
}

/// Sky at full, block at nothing, for every row a column puts on the wire.
/// A server running without a lighting engine still has to say something about
/// light, and saying nothing renders the world black on the client.
pub fn build_fullbright_light_data(rows: usize) -> LightData<'static> {
    let words = rows.div_ceil(64);
    let mut all_rows = vec![0u64; words];
    for row in 0..rows {
        all_rows[row / 64] |= 1 << (row % 64);
    }
    LightData {
        sky_light_mask: Cow::Owned(all_rows.clone()),
        block_light_mask: Cow::Owned(vec![0; words]),
        empty_sky_light_mask: Cow::Owned(vec![0; words]),
        empty_block_light_mask: Cow::Owned(all_rows),
        sky_light_arrays: Cow::Owned(vec![LightChunk([0xff; 2048]); rows]),
        block_light_arrays: Cow::Owned(Vec::new()),
    }
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
    build_light_data(column_entity, params, |_, _| true)
}

/// Build a `LightData` describing only the sections named in the changed
/// slices. Every other row is left out of both masks, which the client reads as
/// [`RowLight::Unchanged`] and leaves as it is — including the two padding rows,
/// which a delta never synthesizes.
///
/// Changed sections arrive as entities because that is what a change-detection
/// query hands the caller, and a delta touches a handful of sections, so a
/// linear scan is cheaper than making the caller build a set.
pub fn build_delta_light_data(
    column_entity: Entity,
    changed_block: &[Entity],
    changed_sky: &[Entity],
    params: &LightCodecParams,
) -> LightData<'static> {
    build_light_data(column_entity, params, |row, layer| match (row, layer) {
        (WireRow::Loaded(e), Layer::Block) => changed_block.contains(&e),
        (WireRow::Loaded(e), Layer::Sky) => changed_sky.contains(&e),
        _ => false,
    })
}

fn build_light_data(
    column_entity: Entity,
    params: &LightCodecParams,
    pack_row: impl Fn(WireRow, Layer) -> bool,
) -> LightData<'static> {
    let Ok(chunk_index) = params.chunk_indexes.get(column_entity) else {
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
    let mut sky_arrays: Vec<LightChunk> = Vec::new();
    let mut block_arrays: Vec<LightChunk> = Vec::new();

    for (bit_idx, lookup) in wire_rows(chunk_index).enumerate() {
        let chunk_entity = match lookup {
            WireRow::Loaded(e) => Some(e),
            _ => None,
        };
        let block_storage = chunk_entity
            .and_then(|e| params.block_lights.get(e).ok())
            .map(|bl| &bl.0);
        let sky_storage = chunk_entity
            .and_then(|e| params.sky_lights.get(e).ok())
            .map(|sl| &sl.0);

        if pack_row(lookup, Layer::Block) {
            pack_chunk(
                lookup,
                block_storage,
                Layer::Block,
                has_sky_light,
                bit_idx,
                &mut block_mask,
                &mut empty_block_mask,
                &mut block_arrays,
            );
        }
        if pack_row(lookup, Layer::Sky) {
            pack_chunk(
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
    use bevy_ecs::entity::Entity;
    use bevy_ecs::prelude::{In, World};
    use bevy_ecs::system::RunSystemOnce;
    use mcrs_minecraft_light::nibble::LightNibbles;

    fn fake_entity(index: u32) -> Entity {
        Entity::from_raw_u32(index + 1).expect("valid entity index")
    }

    fn fresh_buffers() -> (Vec<u64>, Vec<u64>, Vec<LightChunk>) {
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
    fn wire_rows_length_equals_real_plus_two() {
        let si = ColumnChunks::new(-4, 24);
        assert_eq!(wire_rows(&si).count(), 26);
    }

    #[test]
    fn wire_rows_first_is_bottom_padding_last_is_top_padding() {
        let si = ColumnChunks::new(-4, 24);
        assert_eq!(wire_rows(&si).next().unwrap(), WireRow::BottomPadding);
        assert_eq!(wire_rows(&si).last().unwrap(), WireRow::TopPadding);
    }

    #[test]
    fn wire_rows_pads_loaded_and_unloaded() {
        let mut si = ColumnChunks::new(0, 3);
        let e = fake_entity(11);
        si.set_loaded(1, e);
        let collected: Vec<_> = wire_rows(&si).collect();
        assert_eq!(
            collected,
            vec![
                WireRow::BottomPadding,
                WireRow::Unloaded,
                WireRow::Loaded(e),
                WireRow::Unloaded,
                WireRow::TopPadding,
            ]
        );
    }

    #[test]
    fn pack_chunk_bottom_padding_sets_both_empty_masks() {
        // Block layer.
        let (mut mask, mut empty_mask, mut arrays) = fresh_buffers();
        pack_chunk(
            WireRow::BottomPadding,
            None,
            Layer::Block,
            true,
            0,
            &mut mask,
            &mut empty_mask,
            &mut arrays,
        );
        assert!(!bit_is_set(&mask, 0), "block mask bit must NOT be set");
        assert!(
            bit_is_set(&empty_mask, 0),
            "empty block mask bit must be set"
        );
        assert!(arrays.is_empty(), "no block array appended");

        // Sky layer (independent of has_sky_light per the matrix).
        for sky in [false, true] {
            let (mut mask, mut empty_mask, mut arrays) = fresh_buffers();
            pack_chunk(
                WireRow::BottomPadding,
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
    fn pack_chunk_loaded_mixed_block_sets_block_mask_and_appends_array() {
        let mut nibble = LightNibbles::zeros();
        nibble.set(3, 7, 11, 0xA);
        let storage = LightStorage::Dense(Box::new(nibble.clone()));

        let (mut mask, mut empty_mask, mut arrays) = fresh_buffers();
        pack_chunk(
            WireRow::Loaded(fake_entity(1)),
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
        assert_eq!(
            arrays[0],
            LightChunk(*nibble.0),
            "appended bytes must equal Mixed payload"
        );
    }

    #[test]
    fn pack_chunk_loaded_uniform_zero_block_sets_empty_block_mask() {
        let storage = LightStorage::Uniform(0);
        let (mut mask, mut empty_mask, mut arrays) = fresh_buffers();
        pack_chunk(
            WireRow::Loaded(fake_entity(2)),
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
    fn pack_chunk_loaded_uniform_nonzero_block_sets_block_mask_and_synthesizes_payload() {
        let storage = LightStorage::Uniform(0x7);
        let (mut mask, mut empty_mask, mut arrays) = fresh_buffers();
        pack_chunk(
            WireRow::Loaded(fake_entity(3)),
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
        assert_eq!(arrays[0], LightChunk(expected));
        assert!(mask.len() >= 2, "mask must grow to cover bit 65");
    }

    #[test]
    fn pack_chunk_loaded_null_block_sets_empty_block_mask() {
        let storage = LightStorage::Empty;
        let (mut mask, mut empty_mask, mut arrays) = fresh_buffers();
        pack_chunk(
            WireRow::Loaded(fake_entity(4)),
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
    fn pack_chunk_loaded_skyless_dim_sets_empty_sky_mask() {
        let storage = LightStorage::Uniform(0xF);
        let (mut mask, mut empty_mask, mut arrays) = fresh_buffers();
        pack_chunk(
            WireRow::Loaded(fake_entity(5)),
            Some(&storage),
            Layer::Sky,
            false, // skyless dimension
            8,
            &mut mask,
            &mut empty_mask,
            &mut arrays,
        );
        assert!(
            !bit_is_set(&mask, 8),
            "sky mask must NOT be set in skyless dim"
        );
        assert!(bit_is_set(&empty_mask, 8), "empty sky mask must be set");
        assert!(arrays.is_empty(), "no sky payload in skyless dim");

        // Same row with storage = None (component absent on the chunk)
        // must reach the same result.
        let (mut mask, mut empty_mask, mut arrays) = fresh_buffers();
        pack_chunk(
            WireRow::Loaded(fake_entity(5)),
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
    fn pack_chunk_unloaded_sets_no_mask_bit() {
        for layer in [Layer::Block, Layer::Sky] {
            for has_sky in [false, true] {
                let (mut mask, mut empty_mask, mut arrays) = fresh_buffers();
                pack_chunk(
                    WireRow::Unloaded,
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
    fn pack_chunk_top_padding_sky_having_sets_sky_mask_and_appends_0xff() {
        let (mut mask, mut empty_mask, mut arrays) = fresh_buffers();
        pack_chunk(
            WireRow::TopPadding,
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
        assert_eq!(arrays[0], LightChunk([0xFFu8; 2048]));

        // Block layer at TopPadding in a sky-having dim still goes to the
        // empty mask — only the sky layer synthesizes the 0xFF payload.
        let (mut mask, mut empty_mask, mut arrays) = fresh_buffers();
        pack_chunk(
            WireRow::TopPadding,
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
    fn pack_chunk_top_padding_skyless_sets_both_empty_masks() {
        // Sky layer.
        let (mut mask, mut empty_mask, mut arrays) = fresh_buffers();
        pack_chunk(
            WireRow::TopPadding,
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
        pack_chunk(
            WireRow::TopPadding,
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
    fn codec_wire_ordering_invariant_holds_for_synthetic_24_chunk_column() {
        // Synthesize a column-shaped iter_wire output that exercises every
        // matrix row at least once. Wire-ordering invariant:
        // arrays.len() == popcount(mask) per layer, AND arrays must appear
        // in strictly increasing bit order (i.e., the lowest set bit first).
        let mut nibble = LightNibbles::zeros();
        nibble.set(0, 0, 0, 0xC);

        let rows: Vec<(WireRow, Option<LightStorage>, Option<LightStorage>)> = vec![
            (WireRow::BottomPadding, None, None),
            (WireRow::Unloaded, None, None),
            (
                WireRow::Loaded(fake_entity(1)),
                Some(LightStorage::Dense(Box::new(nibble.clone()))),
                Some(LightStorage::Uniform(0xF)),
            ),
            (
                WireRow::Loaded(fake_entity(2)),
                Some(LightStorage::Uniform(0x5)),
                Some(LightStorage::Uniform(0)),
            ),
            (
                WireRow::Loaded(fake_entity(3)),
                Some(LightStorage::Empty),
                Some(LightStorage::Empty),
            ),
            (
                WireRow::Loaded(fake_entity(4)),
                Some(LightStorage::Uniform(0)),
                Some(LightStorage::Dense(Box::new(nibble))),
            ),
            (WireRow::Unloaded, None, None),
            (WireRow::TopPadding, None, None),
        ];

        let mut block_mask: Vec<u64> = Vec::new();
        let mut sky_mask: Vec<u64> = Vec::new();
        let mut empty_block_mask: Vec<u64> = Vec::new();
        let mut empty_sky_mask: Vec<u64> = Vec::new();
        let mut block_arrays: Vec<LightChunk> = Vec::new();
        let mut sky_arrays: Vec<LightChunk> = Vec::new();

        let has_sky_light = true;

        for (bit_idx, (chunk, block_storage, sky_storage)) in rows.iter().enumerate() {
            pack_chunk(
                *chunk,
                block_storage.as_ref(),
                Layer::Block,
                has_sky_light,
                bit_idx,
                &mut block_mask,
                &mut empty_block_mask,
                &mut block_arrays,
            );
            pack_chunk(
                *chunk,
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
            assert_eq!(
                s & e,
                0,
                "sky: mask and empty_mask overlap at word {word_idx}"
            );
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
        assert_eq!(*top_array, LightChunk([0xFFu8; 2048]));
    }

    const SECTIONS: usize = 5;
    const ROWS: usize = SECTIONS + 2;

    fn column_world(has_sky_light: bool) -> (World, Entity, Vec<Entity>) {
        let mut world = World::new();
        let dimension = if has_sky_light {
            world.spawn(HasSkyLight).id()
        } else {
            world.spawn_empty().id()
        };
        let mut chunks = ColumnChunks::new(0, SECTIONS);
        let sections: Vec<Entity> = (0..SECTIONS)
            .map(|y| {
                let section = world
                    .spawn((
                        BlockLight(LightStorage::Empty),
                        SkyLight(LightStorage::Empty),
                    ))
                    .id();
                chunks.set_loaded(y as i32, section);
                section
            })
            .collect();
        let column = world.spawn((chunks, InDimension(dimension))).id();
        (world, column, sections)
    }

    fn delta(
        world: &mut World,
        column: Entity,
        changed_block: Vec<Entity>,
        changed_sky: Vec<Entity>,
    ) -> LightData<'static> {
        world
            .run_system_once_with(
                |input: In<(Entity, Vec<Entity>, Vec<Entity>)>, params: LightCodecParams| {
                    let (column, block, sky) = input.0;
                    build_delta_light_data(column, &block, &sky, &params)
                },
                (column, changed_block, changed_sky),
            )
            .expect("delta system runs")
    }

    #[test]
    fn delta_leaves_every_untouched_row_unchanged() {
        let (mut world, column, sections) = column_world(true);
        let mut nibbles = LightNibbles::zeros();
        nibbles.set(1, 2, 3, 0xB);
        world
            .entity_mut(sections[2])
            .insert(BlockLight(LightStorage::Dense(Box::new(nibbles.clone()))));

        let data = delta(&mut world, column, vec![sections[2]], Vec::new());
        let unpacked = unpack_light_data(&data, ROWS).expect("delta round-trips");

        assert_eq!(unpacked.block[3], RowLight::Filled(LightChunk(*nibbles.0)));
        for row in 0..ROWS {
            if row != 3 {
                assert_eq!(unpacked.block[row], RowLight::Unchanged, "block row {row}");
            }
            assert_eq!(unpacked.sky[row], RowLight::Unchanged, "sky row {row}");
        }
    }

    #[test]
    fn delta_of_a_dark_section_is_empty_not_unchanged() {
        let (mut world, column, sections) = column_world(true);
        world
            .entity_mut(sections[2])
            .insert(BlockLight(LightStorage::Uniform(0)));

        let data = delta(&mut world, column, vec![sections[2]], Vec::new());
        let unpacked = unpack_light_data(&data, ROWS).expect("delta round-trips");

        assert_eq!(unpacked.block[3], RowLight::Empty);
        assert!(data.block_light_arrays.is_empty());
    }

    #[test]
    fn delta_carries_sky_without_block() {
        let (mut world, column, sections) = column_world(true);
        world
            .entity_mut(sections[1])
            .insert(SkyLight(LightStorage::Uniform(0xF)));

        let data = delta(&mut world, column, Vec::new(), vec![sections[1]]);
        let unpacked = unpack_light_data(&data, ROWS).expect("delta round-trips");

        assert_eq!(
            unpacked.sky[2],
            RowLight::Filled(LightChunk([0xFFu8; 2048]))
        );
        for row in 0..ROWS {
            assert_eq!(unpacked.block[row], RowLight::Unchanged, "block row {row}");
            if row != 2 {
                assert_eq!(unpacked.sky[row], RowLight::Unchanged, "sky row {row}");
            }
        }
    }

    #[test]
    fn delta_row_index_is_the_section_index_plus_the_bottom_padding() {
        for index in 0..SECTIONS {
            let (mut world, column, sections) = column_world(true);
            world
                .entity_mut(sections[index])
                .insert(BlockLight(LightStorage::Uniform(0x3)));

            let data = delta(&mut world, column, vec![sections[index]], Vec::new());
            let unpacked = unpack_light_data(&data, ROWS).expect("delta round-trips");

            let filled: Vec<usize> = (0..ROWS)
                .filter(|&row| unpacked.block[row] != RowLight::Unchanged)
                .collect();
            assert_eq!(filled, vec![index + 1], "section {index}");
        }
    }

    #[test]
    fn delta_in_a_skyless_dimension_sends_no_sky_payload() {
        let (mut world, column, sections) = column_world(false);
        world
            .entity_mut(sections[2])
            .insert(SkyLight(LightStorage::Uniform(0xF)));

        let data = delta(&mut world, column, Vec::new(), vec![sections[2]]);
        assert!(data.sky_light_arrays.is_empty());
    }
}
