use crate::Noise;
use crate::gradient::{GradientNoise, NoiseFloat};
use crate::interval::Interval;
use crate::jmath::mul_add64;
use crate::sample_grid::SampleGrid;
use mcrs_minecraft_random::Random;

/// Simplex noise shared by Beta worldgen (`NoiseGenerator2`) and modern vanilla
/// (`SimplexNoise`). The generator is parameterized over the RNG, so the same struct
/// serves the legacy (`LegacyRandom`) and modern (`Xoroshiro`) initialization paths.
#[derive(Clone, PartialEq, Debug)]
pub struct SimplexNoise(GradientNoise);

/// Single simplex corner contribution: `(distance - |d|²)⁴ · (grad · d)`, clamped at 0.
///
/// `distance` is the kernel radius — 0.5 for 2D, 0.6 for 3D. For 2D corners pass `z = 0.0`,
/// which zeroes the gradient's z component.
#[inline(always)]
fn corner(gradient_index: usize, x: f64, y: f64, z: f64, distance: f64) -> f64 {
    let t = mul_add64(-z, z, mul_add64(-y, y, mul_add64(-x, x, distance)));
    if t < 0.0 {
        0.0
    } else {
        let t2 = t * t;
        t2 * t2 * f64::grad_dot_at(gradient_index << 2, x, y, z)
    }
}

impl SimplexNoise {
    const SKEW_2D: f64 = 0.3660254037844386;
    const UNSKEW_2D: f64 = 0.2113248654051871;

    pub fn from_random<T: Random>(random: &mut T) -> Self {
        Self(GradientNoise::from_random(random))
    }

    /// Vanilla's `new SimplexNoise(random, true)`.
    pub fn from_random_at_origin<T: Random>(random: &mut T) -> Self {
        Self(GradientNoise::from_random_scaled(random, 0.0))
    }

    #[inline(always)]
    fn map(&self, input: i32) -> i32 {
        self.0.permute(input as usize) as i32
    }

    pub fn sample_2d(&self, x: f64, z: f64, scale_x: f64, scale_z: f64) -> f64 {
        let px = mul_add64(x, scale_x, self.0.offset_x);
        let py = mul_add64(z, scale_z, self.0.offset_y);

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
        let gi1 = (self.map(
            i.wrapping_add(i1)
                .wrapping_add(self.map(j.wrapping_add(j1))),
        ) % 12) as usize;
        let gi2 =
            (self.map(i.wrapping_add(1).wrapping_add(self.map(j.wrapping_add(1)))) % 12) as usize;

        let n0 = corner(gi0, x0, y0, 0.0, 0.5);
        let n1 = corner(gi1, x1, y1, 0.0, 0.5);
        let n2 = corner(gi2, x2, y2, 0.0, 0.5);

        70.0 * (n0 + n1 + n2)
    }
}

impl Noise for SimplexNoise {
    fn range(&self) -> Interval {
        Interval::symmetric(2.0)
    }

    #[inline(always)]
    fn get(&self, x: f64, _y: f64, z: f64) -> f32 {
        self.sample_2d(x, z, 1.0, 1.0) as f32
    }

    #[inline]
    fn get_column(&self, x: f64, z: f64, ys: &[f64], out: &mut [f32]) {
        debug_assert_eq!(ys.len(), out.len());
        out.fill(self.sample_2d(x, z, 1.0, 1.0) as f32);
    }

    fn add_to_volume(
        &self,
        out: &mut [f32],
        volume: &SampleGrid,
        xz_scale: f64,
        _y_scale: f64,
        amplitude: f32,
    ) {
        let size = volume.size();
        let mut index = 0usize;
        for iz in 0..size.z {
            let z = volume.block_z(iz) as f64 * xz_scale;
            for ix in 0..size.x {
                let x = volume.block_x(ix) as f64 * xz_scale;
                let value = amplitude * self.sample_2d(x, z, 1.0, 1.0) as f32;
                for _ in 0..size.y {
                    out[index] += value;
                    index += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::SimplexNoise;
    use mcrs_minecraft_random::legacy::LegacyRandom;

    #[test]
    fn simplex_reachable() {
        let noise = SimplexNoise::from_random(&mut LegacyRandom::new(845));
        let v = noise.sample_2d(0.5, 0.5, 1.0, 1.0);
        assert!(v.is_finite(), "sample must return a finite f64");
    }
}
