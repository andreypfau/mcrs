use crate::chunk::{LightChunk, LightData};
use anyhow::{Context, bail, ensure};

/// What one wire row says about a layer. A row absent from both masks is not
/// "dark": vanilla means "unchanged", and a delta update leans on that.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(clippy::large_enum_variant)]
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
