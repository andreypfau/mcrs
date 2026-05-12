//! Compile-time L1-attenuated field generator for golden-snapshot fixtures.
//!
//! This is NOT a reference BFS — only the all-air-section case where
//! `level = max(0, emission - manhattan_distance_to_emitter)` holds. Per cell
//! the helper takes the per-emitter max.

/// Compute the all-air-section L1 (Manhattan) attenuated field for the given
/// emitters. Returns the packed nibble array in vanilla YZX-major byte layout.
///
/// `level(x,y,z) = max over emitters of max(0, emission - manhattan_distance)`
/// where `manhattan_distance = |x-ex| + |y-ey| + |z-ez|`.
pub const fn compute_l1_attenuated_field(
    emitters: &[((usize, usize, usize), u8)],
) -> [u8; 2048] {
    let mut out = [0u8; 2048];
    let mut y = 0;
    while y < 16 {
        let mut z = 0;
        while z < 16 {
            let mut x = 0;
            while x < 16 {
                let mut best: u8 = 0;
                let mut e = 0;
                while e < emitters.len() {
                    let ((ex, ey, ez), emission) = emitters[e];
                    let dx = if x > ex { x - ex } else { ex - x };
                    let dy = if y > ey { y - ey } else { ey - y };
                    let dz = if z > ez { z - ez } else { ez - z };
                    let dist = dx + dy + dz;
                    let contribution = if (emission as usize) > dist {
                        (emission as usize - dist) as u8
                    } else {
                        0u8
                    };
                    if contribution > best {
                        best = contribution;
                    }
                    e += 1;
                }
                let idx = (y << 8) | (z << 4) | x;
                let byte_index = idx >> 1;
                let shift = (idx & 1) * 4;
                out[byte_index] |= (best & 0x0F) << shift;
                x += 1;
            }
            z += 1;
        }
        y += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expected_torch_self_cell_reads_14() {
        let arr = compute_l1_attenuated_field(&[((8, 8, 8), 14)]);
        let idx = (8 << 8) | (8 << 4) | 8;
        assert_eq!((arr[idx >> 1] >> ((idx & 1) * 4)) & 0x0F, 14);
    }

    #[test]
    fn expected_torch_step_one_reads_13() {
        let arr = compute_l1_attenuated_field(&[((8, 8, 8), 14)]);
        let idx = (8 << 8) | (9 << 4) | 8;
        assert_eq!((arr[idx >> 1] >> ((idx & 1) * 4)) & 0x0F, 13);
    }

    #[test]
    fn expected_torch_far_corner_reads_zero() {
        let arr = compute_l1_attenuated_field(&[((8, 8, 8), 14)]);
        let idx = 0;
        assert_eq!((arr[idx >> 1] >> ((idx & 1) * 4)) & 0x0F, 0);
    }
}
