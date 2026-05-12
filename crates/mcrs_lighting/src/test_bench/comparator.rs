#[inline]
pub const fn nibble_index(x: usize, y: usize, z: usize) -> usize {
    debug_assert!(x < 16 && y < 16 && z < 16);
    (y << 8) | (z << 4) | x
}

#[inline]
pub fn get_nibble(arr: &[u8; 2048], x: usize, y: usize, z: usize) -> u8 {
    let idx = nibble_index(x, y, z);
    (arr[idx >> 1] >> ((idx & 1) * 4)) & 0x0F
}

#[inline]
pub fn set_nibble(arr: &mut [u8; 2048], x: usize, y: usize, z: usize, val: u8) {
    debug_assert!(val < 16);
    let idx = nibble_index(x, y, z);
    let byte = &mut arr[idx >> 1];
    let shift = (idx & 1) * 4;
    *byte = (*byte & !(0x0F << shift)) | ((val & 0x0F) << shift);
}

pub fn assert_nibbles_eq(actual: &[u8; 2048], expected: &[u8; 2048], label: &str) {
    if actual == expected {
        return;
    }

    let mut diffs: Vec<(usize, usize, usize, u8, u8)> = Vec::with_capacity(16);
    'outer: for y in 0..16 {
        for z in 0..16 {
            for x in 0..16 {
                let exp = get_nibble(expected, x, y, z);
                let act = get_nibble(actual, x, y, z);
                if exp != act {
                    diffs.push((x, y, z, exp, act));
                    if diffs.len() == 16 {
                        break 'outer;
                    }
                }
            }
        }
    }

    let mut msg = format!(
        "snapshot mismatch in '{}': {} cells differ",
        label,
        diffs.len()
    );
    for (x, y, z, exp, act) in &diffs {
        msg.push_str(&format!(
            "\n  ({}, {}, {}): expected {}, got {}",
            x, y, z, exp, act
        ));
    }
    panic!("{msg}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nibble_low_byte_index_zero() {
        let mut arr = [0u8; 2048];
        set_nibble(&mut arr, 0, 0, 0, 0x0F);
        assert_eq!(arr[0], 0x0F);
    }

    #[test]
    fn nibble_high_byte_index_one_preserves_low() {
        let mut arr = [0u8; 2048];
        set_nibble(&mut arr, 0, 0, 0, 0x0F);
        set_nibble(&mut arr, 1, 0, 0, 0x0A);
        assert_eq!(arr[0], 0xAF);
    }

    #[test]
    fn get_nibble_low_returns_value() {
        let mut arr = [0u8; 2048];
        set_nibble(&mut arr, 0, 0, 0, 0x0F);
        assert_eq!(get_nibble(&arr, 0, 0, 0), 0x0F);
    }

    #[test]
    fn get_nibble_high_returns_value() {
        let mut arr = [0u8; 2048];
        set_nibble(&mut arr, 0, 0, 0, 0x0F);
        set_nibble(&mut arr, 1, 0, 0, 0x0A);
        assert_eq!(get_nibble(&arr, 1, 0, 0), 0x0A);
    }

    #[test]
    fn nibble_index_corners() {
        assert_eq!(nibble_index(0, 0, 0), 0);
        assert_eq!(nibble_index(15, 15, 15), 4095);
        assert_eq!(nibble_index(0, 1, 0), 256);
        assert_eq!(nibble_index(0, 0, 1), 16);
    }

    #[test]
    fn round_trip_random_cells() {
        let cells: [(usize, usize, usize, u8); 5] = [
            (3, 7, 11, 0x5),
            (15, 0, 8, 0xC),
            (1, 14, 2, 0x9),
            (10, 10, 10, 0xF),
            (7, 3, 6, 0x1),
        ];
        let mut arr = [0u8; 2048];
        for &(x, y, z, v) in &cells {
            set_nibble(&mut arr, x, y, z, v);
        }
        for &(x, y, z, v) in &cells {
            assert_eq!(get_nibble(&arr, x, y, z), v, "mismatch at ({x}, {y}, {z})");
        }
    }

    #[test]
    fn assert_nibbles_eq_identical_does_not_panic() {
        let a = [0u8; 2048];
        let b = [0u8; 2048];
        assert_nibbles_eq(&a, &b, "identical_zero_arrays");
    }

    #[test]
    fn assert_nibbles_eq_difference_panics_with_coords() {
        let mut a = [0u8; 2048];
        let mut b = [0u8; 2048];
        set_nibble(&mut a, 4, 5, 6, 0x7);
        set_nibble(&mut b, 4, 5, 6, 0x3);

        let label = "diff_at_4_5_6";
        let result = std::panic::catch_unwind(|| {
            assert_nibbles_eq(&a, &b, label);
        });

        let err = result.expect_err("assert_nibbles_eq did not panic on mismatch");
        let payload = err
            .downcast_ref::<String>()
            .map(|s| s.as_str())
            .or_else(|| err.downcast_ref::<&str>().copied())
            .expect("panic payload not a string");

        assert!(
            payload.contains(label),
            "payload missing label: {payload}"
        );
        assert!(
            payload.contains("expected"),
            "payload missing 'expected' substring: {payload}"
        );
        assert!(
            payload.contains("(4, 5, 6)"),
            "payload missing coords: {payload}"
        );
    }
}
