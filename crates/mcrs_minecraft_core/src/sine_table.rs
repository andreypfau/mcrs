//! The two sine tables: the current table and the earlier one that differs from it at one entry.

use std::sync::LazyLock;

/// The number of entries in each sine table.
pub const SINE_TABLE_LEN: usize = 65536;

/// The number of table entries for each radian, `65536 / 2π`.
pub(crate) const ENTRIES_PER_RADIAN: f64 = 10430.378350470453;

const HALF_TURN: usize = 32768;

static EARLIER: LazyLock<[f32; SINE_TABLE_LEN]> = LazyLock::new(|| {
    let mut table = [0.0; SINE_TABLE_LEN];
    for (index, entry) in table.iter_mut().enumerate() {
        *entry = (index as f64 / ENTRIES_PER_RADIAN).sin() as f32;
    }
    table
});

static CURRENT: LazyLock<[f32; SINE_TABLE_LEN]> = LazyLock::new(|| {
    let mut table = *EARLIER;
    for (index, value) in [(0, 0.0), (16384, 1.0), (HALF_TURN, 0.0), (49152, -1.0)] {
        if let Some(entry) = table.get_mut(index) {
            *entry = value;
        }
    }
    table
});

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

pub(crate) fn current_entry(index: u16) -> f32 {
    CURRENT.get(usize::from(index)).copied().unwrap_or(0.0)
}

pub(crate) fn earlier_entry(index: u16) -> f32 {
    EARLIER.get(usize::from(index)).copied().unwrap_or(0.0)
}
