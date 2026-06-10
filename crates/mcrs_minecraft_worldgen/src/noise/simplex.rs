use crate::noise::gradient::GRADIENTS;
use mcrs_random::Random;

/// Simplex noise shared by Beta worldgen (`NoiseGenerator2`) and modern vanilla
/// (`SimplexNoise`). The generator is parameterized over the RNG, so the same struct
/// serves the legacy (`LegacyRandom`) and modern (`Xoroshiro`) initialization paths.
#[derive(Clone, PartialEq, Debug)]
pub struct SimplexNoise {
    permutation: [u8; 256],
    pub origin_x: f64,
    pub origin_y: f64,
    pub origin_z: f64,
}

/// Single simplex corner contribution: `(distance - |d|²)⁴ · (grad · d)`, clamped at 0.
///
/// `distance` is the kernel radius — 0.5 for 2D, 0.6 for 3D. For 2D corners pass `z = 0.0`,
/// which zeroes the gradient's z component.
#[inline(always)]
fn corner(gradient_index: usize, x: f64, y: f64, z: f64, distance: f64) -> f64 {
    let t = distance - x * x - y * y - z * z;
    if t < 0.0 {
        0.0
    } else {
        let t2 = t * t;
        t2 * t2 * GRADIENTS[gradient_index].dot(x, y, z)
    }
}

impl SimplexNoise {
    const SKEW_2D: f64 = 0.3660254037844386;
    const UNSKEW_2D: f64 = 0.2113248654051871;

    const SKEW_3D: f64 = 0.3333333333333333;
    const UNSKEW_3D: f64 = 0.16666666666666666;
    const UNSKEW_3D_2: f64 = 0.3333333333333333;
    const UNSKEW_3D_3: f64 = 0.5;

    pub fn from_random<T: Random>(random: &mut T) -> Self {
        let origin_x = random.next_f64() * 256.0;
        let origin_y = random.next_f64() * 256.0;
        let origin_z = random.next_f64() * 256.0;
        let mut permutation = [0u8; 256];
        for i in 0..256 {
            permutation[i] = i as u8;
        }
        for i in 0..256u32 {
            let j = random.next_u32_bound(256 - i);
            permutation.swap(i as usize, (i + j) as usize);
        }
        Self { permutation, origin_x, origin_y, origin_z }
    }

    #[inline(always)]
    fn map(&self, input: i32) -> i32 {
        self.permutation[(input & 0xFF) as usize] as i32
    }

    pub fn sample(&self, x: f64, z: f64, scale_x: f64, scale_z: f64) -> f64 {
        let px = x * scale_x + self.origin_x;
        let py = z * scale_z + self.origin_y;

        let skew = (px + py) * Self::SKEW_2D;
        let i = (px + skew).floor() as i32;
        let j = (py + skew).floor() as i32;

        let unskew = (i + j) as f64 * Self::UNSKEW_2D;
        let x0 = px - (i as f64 - unskew);
        let y0 = py - (j as f64 - unskew);

        let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };

        let x1 = x0 - i1 as f64 + Self::UNSKEW_2D;
        let y1 = y0 - j1 as f64 + Self::UNSKEW_2D;
        let x2 = x0 - 1.0 + 2.0 * Self::UNSKEW_2D;
        let y2 = y0 - 1.0 + 2.0 * Self::UNSKEW_2D;

        let gi0 = (self.map(i.wrapping_add(self.map(j))) % 12) as usize;
        let gi1 = (self.map(i.wrapping_add(i1).wrapping_add(self.map(j.wrapping_add(j1)))) % 12) as usize;
        let gi2 = (self.map(i.wrapping_add(1).wrapping_add(self.map(j.wrapping_add(1)))) % 12) as usize;

        let n0 = corner(gi0, x0, y0, 0.0, 0.5);
        let n1 = corner(gi1, x1, y1, 0.0, 0.5);
        let n2 = corner(gi2, x2, y2, 0.0, 0.5);

        70.0 * (n0 + n1 + n2)
    }

    /// 3D simplex noise — faithful port of vanilla `SimplexNoise.getValue(x, y, z)`.
    ///
    /// Takes already-offset coordinates (no internal origin/scale, unlike the Beta 2D
    /// `sample`); the caller/octave wrapper applies the per-octave origin. Intended for
    /// modern consumers such as End island generation.
    pub fn sample_3d(&self, x: f64, y: f64, z: f64) -> f64 {
        let skew = (x + y + z) * Self::SKEW_3D;
        let i = (x + skew).floor() as i32;
        let j = (y + skew).floor() as i32;
        let k = (z + skew).floor() as i32;

        let unskew = (i + j + k) as f64 * Self::UNSKEW_3D;
        let x0 = x - (i as f64 - unskew);
        let y0 = y - (j as f64 - unskew);
        let z0 = z - (k as f64 - unskew);

        // Offsets of the second and third simplex corners, by the ordering of x0/y0/z0.
        let (i1, j1, k1, i2, j2, k2) = if x0 >= y0 {
            if y0 >= z0 {
                (1, 0, 0, 1, 1, 0)
            } else if x0 >= z0 {
                (1, 0, 0, 1, 0, 1)
            } else {
                (0, 0, 1, 1, 0, 1)
            }
        } else if y0 < z0 {
            (0, 0, 1, 0, 1, 1)
        } else if x0 < z0 {
            (0, 1, 0, 0, 1, 1)
        } else {
            (0, 1, 0, 1, 1, 0)
        };

        // Vanilla hardcodes these offsets as literals (G3, 2·G3, 3·G3) rather than recomputing
        // `n * G3` — `3.0 * G3` differs from `0.5` in the last bit, so the literals are load-bearing.
        let x1 = x0 - i1 as f64 + Self::UNSKEW_3D;
        let y1 = y0 - j1 as f64 + Self::UNSKEW_3D;
        let z1 = z0 - k1 as f64 + Self::UNSKEW_3D;
        let x2 = x0 - i2 as f64 + Self::UNSKEW_3D_2;
        let y2 = y0 - j2 as f64 + Self::UNSKEW_3D_2;
        let z2 = z0 - k2 as f64 + Self::UNSKEW_3D_2;
        let x3 = x0 - 1.0 + Self::UNSKEW_3D_3;
        let y3 = y0 - 1.0 + Self::UNSKEW_3D_3;
        let z3 = z0 - 1.0 + Self::UNSKEW_3D_3;

        let ii = i & 0xFF;
        let jj = j & 0xFF;
        let kk = k & 0xFF;

        let gi0 = (self.map(ii.wrapping_add(self.map(jj.wrapping_add(self.map(kk))))) % 12) as usize;
        let gi1 = (self.map(
            ii.wrapping_add(i1)
                .wrapping_add(self.map(jj.wrapping_add(j1).wrapping_add(self.map(kk.wrapping_add(k1))))),
        ) % 12) as usize;
        let gi2 = (self.map(
            ii.wrapping_add(i2)
                .wrapping_add(self.map(jj.wrapping_add(j2).wrapping_add(self.map(kk.wrapping_add(k2))))),
        ) % 12) as usize;
        let gi3 = (self.map(
            ii.wrapping_add(1)
                .wrapping_add(self.map(jj.wrapping_add(1).wrapping_add(self.map(kk.wrapping_add(1))))),
        ) % 12) as usize;

        let n0 = corner(gi0, x0, y0, z0, 0.6);
        let n1 = corner(gi1, x1, y1, z1, 0.6);
        let n2 = corner(gi2, x2, y2, z2, 0.6);
        let n3 = corner(gi3, x3, y3, z3, 0.6);

        32.0 * (n0 + n1 + n2 + n3)
    }
}

#[cfg(test)]
mod test {
    use super::SimplexNoise;
    use mcrs_random::legacy::LegacyRandom;

    #[test]
    fn simplex_reachable() {
        let noise = SimplexNoise::from_random(&mut LegacyRandom::new(845));
        let v = noise.sample(0.5, 0.5, 1.0, 1.0);
        assert!(v.is_finite(), "sample must return a finite f64");
    }

    #[test]
    fn simplex_3d_reachable() {
        let noise = SimplexNoise::from_random(&mut LegacyRandom::new(845));
        let v = noise.sample_3d(0.5, 0.5, 0.5);
        assert!(v.is_finite(), "sample_3d must return a finite f64");
    }

    // Golden vectors cross-checked against vanilla `SimplexNoise.getValue(x, y, z)`
    // (via the Pumpkin reference port). Xoroshiro seed 111 with one i32 drawn before
    // construction reproduces vanilla's sampler state exactly.
    #[test]
    fn sample_3d_matches_vanilla() {
        use mcrs_random::Random;
        use mcrs_random::xoroshiro::XoroshiroRandom;

        let mut rng = XoroshiroRandom::new(111);
        assert_eq!(rng.next_i32(), -1467508761);
        let noise = SimplexNoise::from_random(&mut rng);

        assert_eq!(noise.origin_x, 48.58072036717974);
        assert_eq!(noise.origin_y, 110.73235882678037);
        assert_eq!(noise.origin_z, 65.26438852860176);

        let cases = [
            ((-3.134738528791615E8, 5.676610095659718E7, 2.011711832498507E8), -0.07626353895981935),
            ((6.439373693833767E8, -3.36218773041759E8, -3.265494249695775E8), -0.5919400355725402),
            ((1.353820060118252E8, -3.204701624793043E8, -4.612474746056331E8), -0.5220477236433517),
            ((1.0915760091641709E8, 1.932642099859593E7, -3.405060533753616E8), 0.37747828159811136),
        ];
        for ((x, y, z), expected) in cases {
            assert_eq!(noise.sample_3d(x, y, z), expected);
        }
    }
}
