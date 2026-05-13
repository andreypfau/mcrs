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

/// Compute the ground-truth sky-light field for a chunk section with
/// optional obstacles. Each obstacle is `((x, y, z), dampening)`. The
/// algorithm mirrors the vertical-drop and unified attenuation rule
/// implemented by `propagate_increase_sky`: walking each `(x, z)` column
/// from `y = 15` downward, the cell stays at 15 only while the running
/// level is still 15 and the cell's dampening is 0; any other cell
/// attenuates by `max(1, dampening)`.
pub const fn compute_sky_field(
    obstacles: &[((usize, usize, usize), u8)],
) -> [u8; 2048] {
    let mut field = [0u8; 4096];

    let mut z = 0;
    while z < 16 {
        let mut x = 0;
        while x < 16 {
            let mut current_level: u8 = 15;
            let mut y: i32 = 15;
            while y >= 0 {
                let mut dampening: u8 = 0;
                let mut o = 0;
                while o < obstacles.len() {
                    let ((ox, oy, oz), od) = obstacles[o];
                    if ox == x && oy as i32 == y && oz == z {
                        dampening = od;
                    }
                    o += 1;
                }

                let target_level = if dampening == 0 && current_level == 15 {
                    15
                } else {
                    let attenuation = if dampening < 1 { 1 } else { dampening };
                    current_level.saturating_sub(attenuation)
                };
                current_level = target_level;

                let cell_idx = (y as usize * 256) + (z * 16) + x;
                field[cell_idx] = current_level;
                y -= 1;
            }
            x += 1;
        }
        z += 1;
    }

    let mut out = [0u8; 2048];
    let mut y = 0;
    while y < 16 {
        let mut z = 0;
        while z < 16 {
            let mut x = 0;
            while x < 16 {
                let idx = (y << 8) | (z << 4) | x;
                let byte_index = idx >> 1;
                let shift = (idx & 1) * 4;
                let cell_idx = (y * 256) + (z * 16) + x;
                let level = field[cell_idx];
                out[byte_index] |= (level & 0x0F) << shift;
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

    fn nibble_at(arr: &[u8; 2048], x: usize, y: usize, z: usize) -> u8 {
        let idx = (y << 8) | (z << 4) | x;
        (arr[idx >> 1] >> ((idx & 1) * 4)) & 0x0F
    }

    #[test]
    fn compute_sky_field_all_air_is_fifteen() {
        let arr = compute_sky_field(&[]);
        for &byte in arr.iter() {
            assert_eq!(byte, 0xFF, "every byte must encode two nibbles of 15");
        }
    }

    #[test]
    fn compute_sky_field_water_column_attenuates_one_per_cell() {
        let arr = compute_sky_field(&[((8, 10, 8), 1)]);

        assert_eq!(nibble_at(&arr, 8, 15, 8), 15);
        assert_eq!(nibble_at(&arr, 8, 11, 8), 15);
        assert_eq!(nibble_at(&arr, 8, 10, 8), 14);
        assert_eq!(nibble_at(&arr, 8, 9, 8), 13);
        assert_eq!(nibble_at(&arr, 8, 0, 8), 4);

        // Adjacent column unaffected.
        for y in 0..16 {
            assert_eq!(
                nibble_at(&arr, 0, y, 0),
                15,
                "non-water column at y={y} must still read 15"
            );
        }
    }
}
