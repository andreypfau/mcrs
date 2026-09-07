/// Where water sits in one Beta column, one bit per Y.
///
/// Carving replaces only stone, dirt and grass and writes only lava, air and
/// grass, so no ellipsoid can create or remove water: the abort predicate is a
/// pure function of the pre-carve column and can be answered from a bitmask
/// built once instead of re-reading blocks around every ellipsoid.
///
/// The vertical span is Beta's 0..128 exactly, which is also the range the
/// ellipsoid bounds are clamped to; positions outside it are dropped, matching
/// the reference scan's own `0 <= y < 128` guard.
#[derive(Clone)]
pub struct WaterMask {
    columns: Box<[u128; 256]>,
    any: u128,
}

impl Default for WaterMask {
    fn default() -> Self {
        WaterMask {
            columns: Box::new([0; 256]),
            any: 0,
        }
    }
}

impl WaterMask {
    pub fn insert(&mut self, local_x: i32, world_y: i32, local_z: i32) {
        if !(0..16).contains(&local_x) || !(0..16).contains(&local_z) {
            return;
        }
        if !(0..128).contains(&world_y) {
            return;
        }
        let bit = 1u128 << world_y;
        self.columns[(local_x * 16 + local_z) as usize] |= bit;
        self.any |= bit;
    }

    #[inline]
    fn column(&self, local_x: i32, local_z: i32) -> u128 {
        self.columns[(local_x * 16 + local_z) as usize]
    }
}

#[inline]
fn y_bit(y: i32) -> u128 {
    if (0..128).contains(&y) { 1u128 << y } else { 0 }
}

#[inline]
fn y_band(low: i32, high: i32) -> u128 {
    let low = low.max(0);
    let high = high.min(127);
    if low > high {
        return 0;
    }
    let width = (high - low + 1) as u32;
    if width == 128 {
        u128::MAX
    } else {
        ((1u128 << width) - 1) << low
    }
}

/// Whether carving must abort because water touches the shell of the clipped
/// ellipsoid bounds.
///
/// The shell is what the reference scans: every Y of `[y_min - 1, y_max + 1]`
/// on the border strips, and only the two caps in the interior.
pub fn water_abort_scan(
    water: &WaterMask,
    x_min: i32,
    x_max: i32,
    y_min: i32,
    y_max: i32,
    z_min: i32,
    z_max: i32,
) -> bool {
    let band = y_band(y_min - 1, y_max + 1);
    if water.any & band == 0 {
        return false;
    }
    let caps = y_bit(y_min - 1) | y_bit(y_max + 1);
    for x in x_min..x_max {
        let on_x_border = x == x_min || x == x_max - 1;
        for z in z_min..z_max {
            let mask = if on_x_border || z == z_min || z == z_max - 1 {
                band
            } else {
                caps
            };
            if water.column(x, z) & mask != 0 {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_random::Random;
    use mcrs_minecraft_random::legacy::LegacyRandom;

    /// The scan as `MapGenCaves` writes it: an outer Y loop from `y_max + 1`
    /// down to `y_min - 1` that jumps to the bottom cap once it is past the
    /// border strips. Kept as the oracle the bitmask is checked against.
    fn reference_scan(
        is_water: &impl Fn(i32, i32, i32) -> bool,
        x_min: i32,
        x_max: i32,
        y_min: i32,
        y_max: i32,
        z_min: i32,
        z_max: i32,
    ) -> bool {
        let mut abort = false;
        let mut x = x_min;
        while !abort && x < x_max {
            let mut z = z_min;
            while !abort && z < z_max {
                let mut y = y_max + 1;
                while !abort && y >= y_min - 1 {
                    if (0..128).contains(&y) {
                        if is_water(x, y, z) {
                            abort = true;
                        }
                        if y != y_min - 1
                            && x != x_min
                            && x != x_max - 1
                            && z != z_min
                            && z != z_max - 1
                        {
                            y = y_min;
                        }
                    }
                    y -= 1;
                }
                z += 1;
            }
            x += 1;
        }
        abort
    }

    #[test]
    fn oracle_matches_the_reference_scan() {
        let mut rng = LegacyRandom::new(0xC0FFEE);
        let mut checked_aborts = 0u32;
        let mut checked_passes = 0u32;

        for case in 0..400 {
            let mut mask = WaterMask::default();
            let mut water = vec![false; 16 * 16 * 128];
            // Sparse at first, then dense enough that both verdicts occur often.
            let drops = 1 + rng.next_i32_bound(if case % 2 == 0 { 8 } else { 600 });
            for _ in 0..drops {
                let x = rng.next_i32_bound(16);
                let z = rng.next_i32_bound(16);
                let y = rng.next_i32_bound(128);
                water[((x * 16 + z) * 128 + y) as usize] = true;
                mask.insert(x, y, z);
            }
            let is_water = |x: i32, y: i32, z: i32| water[((x * 16 + z) * 128 + y) as usize];

            for _ in 0..40 {
                let x_min = rng.next_i32_bound(16);
                let x_max = (x_min + 1 + rng.next_i32_bound(16 - x_min)).min(16);
                let z_min = rng.next_i32_bound(16);
                let z_max = (z_min + 1 + rng.next_i32_bound(16 - z_min)).min(16);
                let y_min = 1 + rng.next_i32_bound(119);
                let y_max = (y_min + rng.next_i32_bound(20)).min(120);

                let want = reference_scan(&is_water, x_min, x_max, y_min, y_max, z_min, z_max);
                let got = water_abort_scan(&mask, x_min, x_max, y_min, y_max, z_min, z_max);
                assert_eq!(
                    want, got,
                    "box x {x_min}..{x_max} y {y_min}..={y_max} z {z_min}..{z_max}"
                );
                if want {
                    checked_aborts += 1;
                } else {
                    checked_passes += 1;
                }
            }
        }

        assert!(checked_aborts > 1000, "only {checked_aborts} aborts seen");
        assert!(checked_passes > 1000, "only {checked_passes} passes seen");
    }

    #[test]
    fn empty_mask_never_aborts() {
        let mask = WaterMask::default();
        assert!(!water_abort_scan(&mask, 0, 16, 1, 120, 0, 16));
    }

    #[test]
    fn interior_water_between_the_caps_does_not_abort() {
        let mut mask = WaterMask::default();
        mask.insert(8, 60, 8);
        assert!(!water_abort_scan(&mask, 4, 12, 50, 70, 4, 12));
        assert!(water_abort_scan(&mask, 8, 12, 50, 70, 4, 12));
        assert!(water_abort_scan(&mask, 4, 12, 61, 70, 4, 12));
    }
}
