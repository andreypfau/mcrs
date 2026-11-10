use crate::sine_table::{ENTRIES_PER_RADIAN, current_entry, earlier_entry};

const QUARTER_TURN_ENTRIES: f64 = 16384.0;
const BETA_ENTRIES_PER_RADIAN: f32 = f32::from_bits(0x4622_f983);

/// The sine of `radians` from the current table, at the nearest entry. The result is odd in
/// `radians`, and a NaN gives `-0`.
#[inline]
#[must_use]
pub fn sin_nearest(radians: f64) -> f32 {
    if radians >= 0.0 {
        current_entry(nearest_index(radians, 0.0))
    } else {
        -current_entry(nearest_index(-radians, 0.0))
    }
}

/// The cosine of `radians` from the current table, at the nearest entry. A NaN gives `0`.
#[inline]
#[must_use]
pub fn cos_nearest(radians: f64) -> f32 {
    let magnitude = if radians >= 0.0 { radians } else { -radians };
    current_entry(nearest_index(magnitude, QUARTER_TURN_ENTRIES))
}

#[inline]
fn nearest_index(radians: f64, offset: f64) -> u16 {
    (radians * ENTRIES_PER_RADIAN + offset + 0.5) as i64 as u16
}

/// The sine of `radians` from the earlier table. The index truncates toward zero, so a negative
/// angle takes the entry below the nearest one.
#[must_use]
pub fn sin_truncated(radians: f64) -> f32 {
    earlier_entry((radians * ENTRIES_PER_RADIAN) as i64 as u16)
}

/// The cosine of `radians` from the earlier table, with the index of [`sin_truncated`].
#[must_use]
pub fn cos_truncated(radians: f64) -> f32 {
    earlier_entry((radians * ENTRIES_PER_RADIAN + QUARTER_TURN_ENTRIES) as i64 as u16)
}

/// The sine of `radians` from the earlier table, with the f32 index of the Beta releases.
#[must_use]
pub fn sin_beta(radians: f32) -> f32 {
    earlier_entry((radians * BETA_ENTRIES_PER_RADIAN) as i32 as u16)
}

/// The cosine of `radians` from the earlier table, with the f32 index of the Beta releases.
#[must_use]
pub fn cos_beta(radians: f32) -> f32 {
    earlier_entry((radians * BETA_ENTRIES_PER_RADIAN + 16384.0) as i32 as u16)
}
