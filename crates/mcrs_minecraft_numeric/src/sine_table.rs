//! The two sine tables: the current table and the earlier one that differs from it at one entry.

use std::sync::LazyLock;

/// The number of entries in each sine table.
pub const SINE_TABLE_LEN: usize = 65536;

/// The number of table entries for each radian, `65536 / 2π`.
pub(crate) const ENTRIES_PER_RADIAN: f64 = 10430.378350470453;

const QUARTER_TURN: usize = 16384;
const HALF_TURN: usize = 32768;
const THREE_QUARTER_TURN: usize = 49152;

// The tables are built on the heap: a reader can be deep in a tick, and 256 KiB of stack
// temporaries per table overflow a test thread in an unoptimised build.
static EARLIER: LazyLock<Box<[f32; SINE_TABLE_LEN]>> = LazyLock::new(|| {
    let mut entries: Vec<f32> = Vec::with_capacity(SINE_TABLE_LEN);
    for index in 0..SINE_TABLE_LEN {
        entries.push((index as f64 / ENTRIES_PER_RADIAN).sin() as f32);
    }
    boxed(entries)
});

static CURRENT: LazyLock<Box<[f32; SINE_TABLE_LEN]>> = LazyLock::new(|| {
    let mut entries: Vec<f32> = Vec::with_capacity(SINE_TABLE_LEN);
    for (index, &entry) in EARLIER.iter().enumerate() {
        entries.push(match index {
            0 | HALF_TURN => 0.0,
            QUARTER_TURN => 1.0,
            THREE_QUARTER_TURN => -1.0,
            _ => entry,
        });
    }
    boxed(entries)
});

#[expect(
    clippy::expect_used,
    reason = "each caller pushes exactly SINE_TABLE_LEN entries, so a failure is a programming error"
)]
fn boxed(entries: Vec<f32>) -> Box<[f32; SINE_TABLE_LEN]> {
    entries
        .into_boxed_slice()
        .try_into()
        .expect("a sine table has SINE_TABLE_LEN entries")
}

/// The current sine table: entry `k` is the sine of `k / ENTRIES_PER_RADIAN`, with the four
/// quarter turns set to exact values.
#[must_use]
pub fn sine_table() -> &'static [f32; SINE_TABLE_LEN] {
    &CURRENT
}

/// The sine table of earlier releases, without the exact quarter turns. It differs from
/// [`sine_table`] only at the half turn.
#[must_use]
pub fn earlier_sine_table() -> &'static [f32; SINE_TABLE_LEN] {
    &EARLIER
}

#[inline]
pub(crate) fn current_entry(index: u16) -> f32 {
    CURRENT.get(usize::from(index)).copied().unwrap_or(0.0)
}

pub(crate) fn earlier_entry(index: u16) -> f32 {
    EARLIER.get(usize::from(index)).copied().unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_earlier_table_differs_from_the_current_one_only_at_the_half_turn() {
        let differences: Vec<usize> = (0..SINE_TABLE_LEN)
            .filter(|&index| CURRENT[index].to_bits() != EARLIER[index].to_bits())
            .collect();
        assert_eq!(differences, [HALF_TURN]);
        assert_eq!(EARLIER[HALF_TURN].to_bits(), 0x250d_3132);
    }
}
