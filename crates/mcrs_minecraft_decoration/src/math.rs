use std::sync::LazyLock;

/// Beta's `MathHelper` sine table: 65536 entries of `(float)Math.sin(i * 2π / 65536)`,
/// built once at first use the way the Java class builds it at class load.
///
/// The reference quantizes the argument to a table index, so reading the table is
/// not an approximation of that: it is the same value, without paying a `sin` per
/// call. Carve and vein boundaries land on the same blocks either way.
static SIN_TABLE: LazyLock<Box<[f32; 65536]>> = LazyLock::new(|| {
    let mut table = vec![0.0f32; 65536].into_boxed_slice();
    for (index, entry) in table.iter_mut().enumerate() {
        *entry = f64::sin(index as f64 * (std::f64::consts::TAU / 65536.0)) as f32;
    }
    table.try_into().expect("65536 entries")
});

/// Java beta `MathHelper.sin(x)`: `SIN_TABLE[(int)(x * 10430.378F) & 65535]`.
#[inline]
pub fn sin(x: f32) -> f32 {
    SIN_TABLE[(((x * 10430.378_f32) as i32 as u32) & 0xFFFF) as usize]
}

/// Java beta `MathHelper.cos(x)`: the same table, offset by a quarter turn.
#[inline]
pub fn cos(x: f32) -> f32 {
    SIN_TABLE[(((x * 10430.378_f32 + 16384.0_f32) as i32 as u32) & 0xFFFF) as usize]
}

#[cfg(test)]
mod tests {
    /// The table has to answer exactly what computing the entry on the fly did.
    #[test]
    fn the_table_matches_the_computed_entry() {
        for step in 0..10_000 {
            let x = (step as f32 - 5_000.0) * 0.0037;
            let sin_index = ((x * 10430.378_f32) as i32 as u32) & 0xFFFF;
            let cos_index = ((x * 10430.378_f32 + 16384.0_f32) as i32 as u32) & 0xFFFF;
            let entry = |i: u32| f64::sin(i as f64 * (std::f64::consts::TAU / 65536.0)) as f32;
            assert_eq!(super::sin(x), entry(sin_index), "sin({x})");
            assert_eq!(super::cos(x), entry(cos_index), "cos({x})");
        }
    }
}
