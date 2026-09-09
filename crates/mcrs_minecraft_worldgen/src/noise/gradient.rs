use crate::jmath::mul_add64;
use mcrs_minecraft_random::Random;

/// The 16-entry Ken Perlin gradient table shared by the Perlin (`ImprovedNoise`) and
/// simplex (`SimplexNoise`) generators, flattened for the hot paths.
///
/// `ImprovedNoise` indexes it with `hash & 15` and dots against the full 3D offset.
/// `SimplexNoise` indexes it with `hash % 12` and dots against a 2D offset (z = 0), which
/// selects the first twelve gradients projected onto the XY plane — the classic 12-entry
/// simplex gradient set. Both Beta (`NoiseGenerator2`/`NoiseGeneratorPerlin`) and modern
/// vanilla (`SimplexNoise`/`ImprovedNoise`) use these same vectors.
///
/// Both generators dot the same 16 vectors against an offset; only the width of
/// the arithmetic differs, because each path is pinned to a different oracle —
/// modern vanilla computes noise in float, Beta 1.7.3 in double. Keeping the
/// table and the index arithmetic here means an optimization to either lands on
/// both paths, and the width stays a type parameter rather than a fork.
pub trait NoiseFloat:
    Copy + std::ops::Add<Output = Self> + std::ops::Sub<Output = Self> + std::ops::Mul<Output = Self>
{
    /// 16 gradients × {x, y, z, pad}, so a lookup is a shift instead of a multiply.
    const GRAD_FLAT: [Self; 64];

    /// Narrow a coordinate to the value width. Positions stay `f64` on both
    /// paths — as they do in vanilla — and only the sampled value follows `Self`.
    fn from_f64(value: f64) -> Self;

    /// Dot the gradient at `index` — already `(hash & 15) << 2` — against the offset.
    ///
    /// Masking with 60 keeps the three reads provably inside the table, which is
    /// what lets the bounds checks go without `unsafe`.
    fn grad_dot_at(index: usize, x: Self, y: Self, z: Self) -> Self;

    /// Dot without the y term — vanilla's `dotXz`. The 2D fills and the modern
    /// path's column split both need the xz plane on its own.
    fn grad_dot_xz_at(index: usize, x: Self, z: Self) -> Self;

    /// The gradient's y component, which the modern path carries as the slope of
    /// a column rather than folding it into the dot.
    #[inline(always)]
    fn grad_y_at(index: usize) -> Self {
        Self::GRAD_FLAT[(index & 60) | 1]
    }

    #[inline(always)]
    fn grad_dot(hash: usize, x: Self, y: Self, z: Self) -> Self {
        Self::grad_dot_at((hash & 15) << 2, x, y, z)
    }

    #[inline(always)]
    fn grad_dot_xz(hash: usize, x: Self, z: Self) -> Self {
        Self::grad_dot_xz_at((hash & 15) << 2, x, z)
    }
}

macro_rules! flat_gradients {
    () => {
        [
            1.0, 1.0, 0.0, 0.0, -1.0, 1.0, 0.0, 0.0, 1.0, -1.0, 0.0, 0.0, -1.0, -1.0, 0.0, 0.0,
            1.0, 0.0, 1.0, 0.0, -1.0, 0.0, 1.0, 0.0, 1.0, 0.0, -1.0, 0.0, -1.0, 0.0, -1.0, 0.0,
            0.0, 1.0, 1.0, 0.0, 0.0, -1.0, 1.0, 0.0, 0.0, 1.0, -1.0, 0.0, 0.0, -1.0, -1.0, 0.0,
            1.0, 1.0, 0.0, 0.0, 0.0, -1.0, 1.0, 0.0, -1.0, 1.0, 0.0, 0.0, 0.0, -1.0, -1.0, 0.0,
        ]
    };
}

impl NoiseFloat for f64 {
    const GRAD_FLAT: [f64; 64] = flat_gradients!();

    #[inline(always)]
    fn from_f64(value: f64) -> f64 {
        value
    }

    /// Every gradient has one zero component, so one of the three products is
    /// always ±0 and the nesting cannot move the strict result.
    #[inline(always)]
    fn grad_dot_at(index: usize, x: f64, y: f64, z: f64) -> f64 {
        let i = index & 60;
        mul_add64(
            Self::GRAD_FLAT[i | 2],
            z,
            mul_add64(Self::GRAD_FLAT[i | 1], y, Self::GRAD_FLAT[i] * x),
        )
    }

    #[inline(always)]
    fn grad_dot_xz_at(index: usize, x: f64, z: f64) -> f64 {
        let i = index & 60;
        mul_add64(Self::GRAD_FLAT[i | 2], z, Self::GRAD_FLAT[i] * x)
    }
}

impl NoiseFloat for f32 {
    const GRAD_FLAT: [f32; 64] = flat_gradients!();

    #[inline(always)]
    fn from_f64(value: f64) -> f32 {
        value as f32
    }

    /// The f32 path's parity was captured against vanilla with these three
    /// products fused, so the rounding of the fused form is part of the contract.
    #[inline(always)]
    fn grad_dot_at(index: usize, x: f32, y: f32, z: f32) -> f32 {
        let i = index & 60;
        Self::GRAD_FLAT[i | 2].mul_add(z, Self::GRAD_FLAT[i | 1].mul_add(y, Self::GRAD_FLAT[i] * x))
    }

    #[inline(always)]
    fn grad_dot_xz_at(index: usize, x: f32, z: f32) -> f32 {
        let i = index & 60;
        Self::GRAD_FLAT[i | 2].mul_add(z, Self::GRAD_FLAT[i] * x)
    }
}

/// Perlin lattice state: the permutation table and the per-instance offset.
/// `PerlinNoise`, `SmearedPerlinNoise` and the Beta sampler all build on it.
#[derive(Debug, Clone, PartialEq)]
pub struct GradientNoise {
    pub(crate) perms: [u8; 256],
    pub offset_x: f64,
    pub offset_y: f64,
    pub offset_z: f64,
}

impl GradientNoise {
    pub fn from_random<T: Random>(random: &mut T) -> Self {
        Self::from_random_scaled(random, 256.0)
    }

    /// Vanilla's second constructor: the three offset draws still happen, so the
    /// stream advances the same way, but a scale of zero pins the lattice to the
    /// world origin.
    pub fn from_random_scaled<T: Random>(random: &mut T, offset_scale: f64) -> Self {
        let offset_x = random.next_f64() * offset_scale;
        let offset_y = random.next_f64() * offset_scale;
        let offset_z = random.next_f64() * offset_scale;
        let mut perms = [0u8; 256];
        for i in 0..256 {
            perms[i] = i as u8;
        }
        for i in 0..256 {
            let j = random.next_u32_bound(256 - i);
            perms.swap(i as usize, (i + j) as usize);
        }
        Self {
            perms,
            offset_x,
            offset_y,
            offset_z,
        }
    }

    /// One lattice per non-zero amplitude, in the draw order the seed defines.
    ///
    /// The draw is width-independent — a permutation and three origins — so both
    /// the modern and the Beta samplers are built from the same stream.
    pub fn octaves<T>(random: &mut T, first_octave: i32, amplitudes: &[f64]) -> Vec<Option<Self>>
    where
        T: Random + Clone,
    {
        let octaves = amplitudes
            .iter()
            .enumerate()
            .map(|(i, amplitude)| {
                (*amplitude != 0.0).then(|| {
                    let octave = i as i32 + first_octave;
                    let mut octave_random = random
                        .clone()
                        .fork_hash(format!("octave_{octave}").as_bytes());
                    Self::from_random(&mut octave_random)
                })
            })
            .collect();
        random.fork();
        octaves
    }

    /// The pre-26.3 draw, which vanilla keeps as a deprecated class of its own
    /// rather than a mode of [`Self::octaves`]: it walks the octaves in reverse
    /// and burns 262 ints per skipped one, which is what keeps an unmodified
    /// octave from shifting the stream.
    pub fn legacy_octaves<T: Random>(
        random: &mut T,
        first_octave: i32,
        amplitudes: &[f64],
    ) -> Vec<Option<Self>> {
        let mut octaves: Vec<Option<Self>> = (0..=-first_octave as usize)
            .rev()
            .map(|i| {
                if amplitudes.get(i).is_some_and(|a| *a != 0.0) {
                    Some(Self::from_random(random))
                } else {
                    for _ in 0..262 {
                        random.next_i32();
                    }
                    None
                }
            })
            .collect();
        octaves.reverse();
        octaves
    }

    #[inline(always)]
    pub(crate) fn permute(&self, index: usize) -> usize {
        self.perms[index & 0xFF] as usize
    }

    #[inline(always)]
    pub(crate) fn x_perms(&self, section_x: i32) -> (usize, usize) {
        (
            self.permute((section_x & 0xFF) as usize),
            self.permute((section_x.wrapping_add(1) & 0xFF) as usize),
        )
    }

    /// The eight corner gradient indices of one lattice cell, pre-shifted by two
    /// so a lookup into `GRAD_FLAT` is an add rather than a multiply.
    #[inline(always)]
    pub(crate) fn corner_grads(
        &self,
        p0: usize,
        p1: usize,
        section_y: i32,
        section_z: i32,
    ) -> [usize; 8] {
        let sy = section_y as usize;
        let p4 = self.permute(p0.wrapping_add(sy));
        let p5 = self.permute(p1.wrapping_add(sy));
        let p6 = self.permute(p0.wrapping_add(sy).wrapping_add(1));
        let p7 = self.permute(p1.wrapping_add(sy).wrapping_add(1));
        let sz = section_z as usize;
        let grad = |base: usize| (self.permute(base) & 15) << 2;
        [
            grad(p4.wrapping_add(sz)),
            grad(p5.wrapping_add(sz)),
            grad(p6.wrapping_add(sz)),
            grad(p7.wrapping_add(sz)),
            grad(p4.wrapping_add(sz).wrapping_add(1)),
            grad(p5.wrapping_add(sz).wrapping_add(1)),
            grad(p6.wrapping_add(sz).wrapping_add(1)),
            grad(p7.wrapping_add(sz).wrapping_add(1)),
        ]
    }
}

/// Folds a coordinate back into the range where a double still resolves single
/// blocks. Vanilla's `GradientNoise.wrap`.
#[inline(always)]
pub fn wrap(value: f64) -> f64 {
    const FACTOR: f64 = 3.3554432E7;
    // Inside half a period the round-off is the identity, which is every
    // coordinate short of the far lands; the check is cheaper than the divide,
    // floor and multiply it skips.
    const HALF: f64 = 1.6777216E7;
    if value >= -HALF && value < HALF {
        value
    } else {
        value - (value / FACTOR + 0.5).floor() * FACTOR
    }
}

#[cfg(test)]
mod grad_tests {
    use super::NoiseFloat;

    /// The table must reproduce Java's branchy `grad` bit for bit, for every
    /// hash and both signs of every component.
    ///
    /// The one exception is the sign of zero: the dot product carries a third
    /// `0.0 * z` term Java never evaluates, so a result that is exactly zero can
    /// come out `+0.0` where Java produced `-0.0`. That needs all three
    /// fractional coordinates to land exactly on the lattice, and the value then
    /// only ever feeds lerps and threshold comparisons, where the two zeroes are
    /// indistinguishable.
    #[test]
    fn the_gradient_table_matches_javas_branchy_grad() {
        fn java_grad(hash: usize, x: f64, y: f64, z: f64) -> f64 {
            let j = hash & 15;
            let d3 = if j < 8 { x } else { y };
            let d4 = if j < 4 {
                y
            } else if j == 12 || j == 14 {
                x
            } else {
                z
            };
            let r = if (j & 1) == 0 { d3 } else { -d3 };
            let s = if (j & 2) == 0 { d4 } else { -d4 };
            r + s
        }

        for hash in 0..256usize {
            for &(x, y, z) in &[
                (0.37, -0.81, 0.62),
                (-0.5, 0.25, -0.125),
                (1.0, 1.0, 1.0),
                (0.0, -0.0, 0.0),
            ] {
                let ours = f64::grad_dot(hash, x, y, z);
                let java = java_grad(hash, x, y, z);
                assert_eq!(ours, java, "hash {hash} at {x},{y},{z}");
                if java != 0.0 {
                    assert_eq!(ours.to_bits(), java.to_bits(), "hash {hash} at {x},{y},{z}");
                }
            }
        }
    }
}
