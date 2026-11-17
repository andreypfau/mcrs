use crate::jmath::mul_add64;
use mcrs_minecraft_random::Random;

pub trait NoiseFloat:
    Copy + std::ops::Add<Output = Self> + std::ops::Sub<Output = Self> + std::ops::Mul<Output = Self>
{
    const GRAD_FLAT: [Self; 64];

    fn from_f64(value: f64) -> Self;

    fn grad_dot_at(index: usize, x: Self, y: Self, z: Self) -> Self;

    fn grad_dot_xz_at(index: usize, x: Self, z: Self) -> Self;

    #[inline(always)]
    fn grad_y_at(index: usize) -> Self {
        let [_, y, _] = Self::gradient_at(index);
        y
    }

    #[inline(always)]
    fn gradient_at(index: usize) -> [Self; 3] {
        match Self::GRAD_FLAT
            .get(index & 60..)
            .and_then(<[Self]>::first_chunk)
        {
            Some(&gradient) => gradient,
            None => [Self::from_f64(0.0); 3],
        }
    }

    #[inline(always)]
    fn grad_dot(hash: usize, x: Self, y: Self, z: Self) -> Self {
        Self::grad_dot_at((hash & 15) << 2, x, y, z)
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

    #[inline(always)]
    fn grad_dot_at(index: usize, x: f64, y: f64, z: f64) -> f64 {
        let [gx, gy, gz] = Self::gradient_at(index);
        mul_add64(gz, z, mul_add64(gy, y, gx * x))
    }

    #[inline(always)]
    fn grad_dot_xz_at(index: usize, x: f64, z: f64) -> f64 {
        let [gx, _, gz] = Self::gradient_at(index);
        mul_add64(gz, z, gx * x)
    }
}

impl NoiseFloat for f32 {
    const GRAD_FLAT: [f32; 64] = flat_gradients!();

    #[inline(always)]
    fn from_f64(value: f64) -> f32 {
        value as f32
    }

    #[inline(always)]
    fn grad_dot_at(index: usize, x: f32, y: f32, z: f32) -> f32 {
        let [gx, gy, gz] = Self::gradient_at(index);
        gz.mul_add(z, gy.mul_add(y, gx * x))
    }

    #[inline(always)]
    fn grad_dot_xz_at(index: usize, x: f32, z: f32) -> f32 {
        let [gx, _, gz] = Self::gradient_at(index);
        gz.mul_add(z, gx * x)
    }
}

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

    pub fn from_random_scaled<T: Random>(random: &mut T, offset_scale: f64) -> Self {
        let offset_x = random.next_f64() * offset_scale;
        let offset_y = random.next_f64() * offset_scale;
        let offset_z = random.next_f64() * offset_scale;
        let mut perms: [u8; 256] = std::array::from_fn(|i| i as u8);
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
        usize::from(self.perms.get(index & 0xFF).copied().unwrap_or_default())
    }

    #[inline(always)]
    pub(crate) fn x_perms(&self, section_x: i32) -> (usize, usize) {
        (
            self.permute((section_x & 0xFF) as usize),
            self.permute((section_x.wrapping_add(1) & 0xFF) as usize),
        )
    }

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

#[inline(always)]
pub fn wrap(value: f64) -> f64 {
    const FACTOR: f64 = 3.3554432E7;
    const HALF: f64 = 1.6777216E7;
    if (-HALF..HALF).contains(&value) {
        value
    } else {
        value - (value / FACTOR + 0.5).floor() * FACTOR
    }
}

#[cfg(test)]
mod grad_tests {
    use super::NoiseFloat;

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
