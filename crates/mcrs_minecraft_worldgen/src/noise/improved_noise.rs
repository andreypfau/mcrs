use mcrs_random::{Random, RandomSource};
use num_traits::{Float, NumCast, ToPrimitive};

const FLAT_SIMPLEX_GRAD: [f32; 64] = [
    1.0, 1.0, 0.0, 0.0, -1.0, 1.0, 0.0, 0.0, 1.0, -1.0, 0.0, 0.0, -1.0, -1.0, 0.0, 0.0, 1.0, 0.0,
    1.0, 0.0, -1.0, 0.0, 1.0, 0.0, 1.0, 0.0, -1.0, 0.0, -1.0, 0.0, -1.0, 0.0, 0.0, 1.0, 1.0, 0.0,
    0.0, -1.0, 1.0, 0.0, 0.0, 1.0, -1.0, 0.0, 0.0, -1.0, -1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, -1.0,
    1.0, 0.0, -1.0, 1.0, 0.0, 0.0, 0.0, -1.0, -1.0, 0.0,
];

#[derive(Debug, Clone, PartialEq)]
pub struct ImprovedNoise<F: Float, const BETA: bool> {
    permutation: [u8; 256],
    pub origin_x: F,
    pub origin_y: F,
    pub origin_z: F,
}

impl Default for ImprovedNoise<f32, false> {
    fn default() -> Self {
        Self::from_random(&mut RandomSource::new(0, true))
    }
}

impl<F: Float, const BETA: bool> ImprovedNoise<F, BETA> {
    pub fn from_random<T>(random: &mut T) -> Self
    where
        T: Random,
    {
        let scale: F = F::from(256.0_f64).unwrap();
        let (origin_x, origin_y, origin_z) = if BETA {
            (
                F::from(random.next_f64()).unwrap() * scale,
                F::from(random.next_f64()).unwrap() * scale,
                F::from(random.next_f64()).unwrap() * scale,
            )
        } else {
            (
                F::from(random.next_f32()).unwrap() * scale,
                F::from(random.next_f32()).unwrap() * scale,
                F::from(random.next_f32()).unwrap() * scale,
            )
        };
        let mut permutation = [0u8; 256];
        for i in 0..256 {
            permutation[i] = i as u8;
        }
        for i in 0..256 {
            let j = random.next_u32_bound(256 - i);
            permutation.swap(i as usize, (i + j) as usize);
        }
        Self {
            permutation,
            origin_x,
            origin_y,
            origin_z,
        }
    }
}

impl ImprovedNoise<f64, true> {
    /// Scalar trilinear-lerp sample for the Beta f64 path.
    ///
    /// Applies the failurePoint clamp (full i32 range, no-op near origin) before floor,
    /// then mirrors the trilinear-lerp algorithm of `sample_and_lerp` in safe scalar f64.
    #[inline(always)]
    pub fn sample(&self, x: f64, y: f64, z: f64, y_scale: f64, y_max: f64) -> f64 {
        let shifted_x = x + self.origin_x;
        let shifted_y = y + self.origin_y;
        let shifted_z = z + self.origin_z;

        let clamp_max = i32::MAX as f64;
        let clamp_min = i32::MIN as f64;
        let section_x = shifted_x.max(clamp_min).min(clamp_max).floor().to_i32().unwrap_or(i32::MAX);
        let section_y = shifted_y.max(clamp_min).min(clamp_max).floor().to_i32().unwrap_or(i32::MAX);
        let section_z = shifted_z.max(clamp_min).min(clamp_max).floor().to_i32().unwrap_or(i32::MAX);

        let local_x = shifted_x - section_x as f64;
        let local_y = shifted_y - section_y as f64;
        let local_z = shifted_z - section_z as f64;

        let mut fade = 0.0_f64;
        if y_scale != 0.0 {
            let t = if y_max >= 0.0 && y_max < local_y { y_max } else { local_y };
            fade = (t / y_scale + 1.0E-7_f64).floor() * y_scale;
        }

        let fade_y = local_y - fade;

        let perm = &self.permutation;
        let p = |idx: usize| perm[idx & 0xFF] as usize;

        let x0 = (section_x & 0xFF) as usize;
        let x1 = (section_x.wrapping_add(1) & 0xFF) as usize;
        let p0 = p(x0);
        let p1 = p(x1);

        let sy = section_y as usize;
        let p00 = p(p0.wrapping_add(sy));
        let p10 = p(p1.wrapping_add(sy));
        let p01 = p(p0.wrapping_add(sy).wrapping_add(1));
        let p11 = p(p1.wrapping_add(sy).wrapping_add(1));

        let sz = section_z as usize;
        let h000 = p(p00.wrapping_add(sz)) & 15;
        let h100 = p(p10.wrapping_add(sz)) & 15;
        let h010 = p(p01.wrapping_add(sz)) & 15;
        let h110 = p(p11.wrapping_add(sz)) & 15;
        let h001 = p(p00.wrapping_add(sz).wrapping_add(1)) & 15;
        let h101 = p(p10.wrapping_add(sz).wrapping_add(1)) & 15;
        let h011 = p(p01.wrapping_add(sz).wrapping_add(1)) & 15;
        let h111 = p(p11.wrapping_add(sz).wrapping_add(1)) & 15;

        let lx1 = local_x - 1.0;
        let ly1 = fade_y - 1.0;
        let lz1 = local_z - 1.0;

        let d000 = grad3(h000, local_x, fade_y, local_z);
        let d100 = grad3(h100, lx1,     fade_y, local_z);
        let d010 = grad3(h010, local_x, ly1,    local_z);
        let d110 = grad3(h110, lx1,     ly1,    local_z);
        let d001 = grad3(h001, local_x, fade_y, lz1);
        let d101 = grad3(h101, lx1,     fade_y, lz1);
        let d011 = grad3(h011, local_x, ly1,    lz1);
        let d111 = grad3(h111, lx1,     ly1,    lz1);

        let fx = fade_curve(local_x);
        let fy = fade_curve(local_y);
        let fz = fade_curve(local_z);

        let l00 = lerp(fx, d000, d100);
        let l10 = lerp(fx, d010, d110);
        let l01 = lerp(fx, d001, d101);
        let l11 = lerp(fx, d011, d111);
        let ll0 = lerp(fy, l00, l10);
        let ll1 = lerp(fy, l01, l11);
        lerp(fz, ll0, ll1)
    }
}

/// Gradient dot-product for the 16-entry Ken Perlin gradient table (same layout as FLAT_SIMPLEX_GRAD).
#[inline(always)]
fn grad3(hash: usize, x: f64, y: f64, z: f64) -> f64 {
    // The gradient table used in `sample_and_lerp` stores 16 3-component gradients packed as
    // 4 f32s (with a zero pad): index = (hash & 15) << 2.
    // Reproduced here as a scalar f64 lookup matching the same 16 gradient vectors.
    match hash {
        0  =>  x + y,
        1  => -x + y,
        2  =>  x - y,
        3  => -x - y,
        4  =>  x + z,
        5  => -x + z,
        6  =>  x - z,
        7  => -x - z,
        8  =>  y + z,
        9  => -y + z,
        10 =>  y - z,
        11 => -y - z,
        12 =>  x + y,
        13 => -y + z,
        14 => -x + y,
        15 => -y - z,
        _  => unreachable!(),
    }
}

#[inline(always)]
fn fade_curve(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline(always)]
fn lerp(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}

impl ImprovedNoise<f32, false> {
    #[inline(always)]
    pub fn sample(&self, x: f32, y: f32, z: f32, y_scale: f32, y_max: f32) -> f32 {
        let shifted_x = x + self.origin_x;
        let shifted_y = y + self.origin_y;
        let shifted_z = z + self.origin_z;
        let section_x = shifted_x.floor() as i32;
        let section_y = shifted_y.floor() as i32;
        let section_z = shifted_z.floor() as i32;
        let local_x = shifted_x - section_x as f32;
        let local_y = shifted_y - section_y as f32;
        let local_z = shifted_z - section_z as f32;
        let mut fade = 0.0;
        if y_scale != 0.0 {
            let t = if y_max >= 0.0 && y_max < local_y {
                y_max
            } else {
                local_y
            };
            fade = (t / y_scale + 1.0E-7f32).floor() * y_scale
        }
        self.sample_and_lerp(
            section_x,
            section_y,
            section_z,
            local_x,
            local_y - fade,
            local_z,
            local_y,
        )
    }

    #[inline(always)]
    pub fn sample_and_lerp(
        &self,
        section_x: i32,
        section_y: i32,
        section_z: i32,
        local_x: f32,
        local_y: f32,
        local_z: f32,
        fade_local_x: f32,
    ) -> f32 {
        // SAFETY: All permutation indices are masked with & 0xFF, guaranteeing [0, 255].
        // All gradient indices are (perm & 15) << 2 = [0, 60], accessed with offsets +0/+1/+2,
        // so max index is 62, within FLAT_SIMPLEX_GRAD's 64 elements.
        unsafe {
            let perm = &self.permutation;

            // Hash lookups — first level (X)
            let var0 = (section_x & 0xFF) as usize;
            let var1 = (section_x.wrapping_add(1) & 0xFF) as usize;
            let p0 = *perm.get_unchecked(var0) as usize;
            let p1 = *perm.get_unchecked(var1) as usize;

            // Second level (X+Y)
            let sy = section_y as usize;
            let var4 = (p0.wrapping_add(sy)) & 0xFF;
            let var5 = (p1.wrapping_add(sy)) & 0xFF;
            let var6 = (p0.wrapping_add(sy).wrapping_add(1)) & 0xFF;
            let var7 = (p1.wrapping_add(sy).wrapping_add(1)) & 0xFF;
            let p4 = *perm.get_unchecked(var4) as usize;
            let p5 = *perm.get_unchecked(var5) as usize;
            let p6 = *perm.get_unchecked(var6) as usize;
            let p7 = *perm.get_unchecked(var7) as usize;

            // Third level (X+Y+Z) — 8 corner gradient indices
            let sz = section_z as usize;
            let h000 = ((*perm.get_unchecked((p4.wrapping_add(sz)) & 0xFF) & 15) as usize) << 2;
            let h100 = ((*perm.get_unchecked((p5.wrapping_add(sz)) & 0xFF) & 15) as usize) << 2;
            let h010 = ((*perm.get_unchecked((p6.wrapping_add(sz)) & 0xFF) & 15) as usize) << 2;
            let h110 = ((*perm.get_unchecked((p7.wrapping_add(sz)) & 0xFF) & 15) as usize) << 2;
            let h001 = ((*perm.get_unchecked((p4.wrapping_add(sz).wrapping_add(1)) & 0xFF) & 15) as usize) << 2;
            let h101 = ((*perm.get_unchecked((p5.wrapping_add(sz).wrapping_add(1)) & 0xFF) & 15) as usize) << 2;
            let h011 = ((*perm.get_unchecked((p6.wrapping_add(sz).wrapping_add(1)) & 0xFF) & 15) as usize) << 2;
            let h111 = ((*perm.get_unchecked((p7.wrapping_add(sz).wrapping_add(1)) & 0xFF) & 15) as usize) << 2;

            // Relative offsets for the far corner
            let x1 = local_x - 1.0;
            let y1 = local_y - 1.0;
            let z1 = local_z - 1.0;

            // Gradient dot products using FMA (grad · offset)
            let g = &FLAT_SIMPLEX_GRAD;
            let d000 = g.get_unchecked(h000 + 2).mul_add(local_z, g.get_unchecked(h000 + 1).mul_add(local_y, *g.get_unchecked(h000) * local_x));
            let d100 = g.get_unchecked(h100 + 2).mul_add(local_z, g.get_unchecked(h100 + 1).mul_add(local_y, *g.get_unchecked(h100) * x1));
            let d010 = g.get_unchecked(h010 + 2).mul_add(local_z, g.get_unchecked(h010 + 1).mul_add(y1, *g.get_unchecked(h010) * local_x));
            let d110 = g.get_unchecked(h110 + 2).mul_add(local_z, g.get_unchecked(h110 + 1).mul_add(y1, *g.get_unchecked(h110) * x1));
            let d001 = g.get_unchecked(h001 + 2).mul_add(z1, g.get_unchecked(h001 + 1).mul_add(local_y, *g.get_unchecked(h001) * local_x));
            let d101 = g.get_unchecked(h101 + 2).mul_add(z1, g.get_unchecked(h101 + 1).mul_add(local_y, *g.get_unchecked(h101) * x1));
            let d011 = g.get_unchecked(h011 + 2).mul_add(z1, g.get_unchecked(h011 + 1).mul_add(y1, *g.get_unchecked(h011) * local_x));
            let d111 = g.get_unchecked(h111 + 2).mul_add(z1, g.get_unchecked(h111 + 1).mul_add(y1, *g.get_unchecked(h111) * x1));

            // Fade curves: t³(6t² - 15t + 10)
            let fade_x = local_x * local_x * local_x * local_x.mul_add(local_x.mul_add(6.0, -15.0), 10.0);
            let fade_y = fade_local_x * fade_local_x * fade_local_x * fade_local_x.mul_add(fade_local_x.mul_add(6.0, -15.0), 10.0);
            let fade_z = local_z * local_z * local_z * local_z.mul_add(local_z.mul_add(6.0, -15.0), 10.0);

            // Trilinear interpolation using FMA
            let l00 = (d100 - d000).mul_add(fade_x, d000);
            let l10 = (d110 - d010).mul_add(fade_x, d010);
            let l01 = (d101 - d001).mul_add(fade_x, d001);
            let l11 = (d111 - d011).mul_add(fade_x, d011);
            let ll0 = (l10 - l00).mul_add(fade_y, l00);
            let ll1 = (l11 - l01).mul_add(fade_y, l01);
            (ll1 - ll0).mul_add(fade_z, ll0)
        }
    }
}

#[cfg(test)]
mod test {
    use crate::noise::improved_noise::ImprovedNoise;
    use mcrs_random::legacy::LegacyRandom;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct ImprovedNoiseBetaFixture {
        origin_x: f64,
        origin_y: f64,
        origin_z: f64,
        permutation_first_10: Vec<u8>,
        sample_05_05_05: f64,
        rng_seed_after_construction: u64,
    }

    #[derive(Deserialize)]
    struct Seed845Fixture {
        improved_noise_beta: ImprovedNoiseBetaFixture,
    }

    fn load_fixture() -> Seed845Fixture {
        serde_json::from_str(include_str!("beta/fixtures/seed_845.json"))
            .expect("valid fixture JSON")
    }

    #[test]
    fn beta_improved_noise_origin() {
        let fx = load_fixture().improved_noise_beta;
        let noise = ImprovedNoise::<f64, true>::from_random(&mut LegacyRandom::new(845));
        assert!(
            (noise.origin_x - fx.origin_x).abs() < 1e-6,
            "origin_x mismatch: got {}, expected {}",
            noise.origin_x,
            fx.origin_x
        );
        assert!(
            (noise.origin_y - fx.origin_y).abs() < 1e-6,
            "origin_y mismatch: got {}, expected {}",
            noise.origin_y,
            fx.origin_y
        );
        assert!(
            (noise.origin_z - fx.origin_z).abs() < 1e-6,
            "origin_z mismatch: got {}, expected {}",
            noise.origin_z,
            fx.origin_z
        );
    }

    #[test]
    fn beta_improved_noise_permutation() {
        let fx = load_fixture().improved_noise_beta;
        let noise = ImprovedNoise::<f64, true>::from_random(&mut LegacyRandom::new(845));
        assert_eq!(&noise.permutation[0..10], fx.permutation_first_10.as_slice());
    }

    #[test]
    fn beta_improved_noise_sample() {
        let fx = load_fixture().improved_noise_beta;
        let noise = ImprovedNoise::<f64, true>::from_random(&mut LegacyRandom::new(845));
        let got = noise.sample(0.5, 0.5, 0.5, 0.0, 0.0);
        assert!(
            (got - fx.sample_05_05_05).abs() < 1e-6,
            "sample mismatch: got {:.8}, expected {:.8}",
            got,
            fx.sample_05_05_05
        );
    }

    #[test]
    #[ignore = "bootstrap: run once to capture fixture values"]
    fn bootstrap_seed_845_improved_noise() {
        let mut rng = LegacyRandom::new(845);
        let noise = ImprovedNoise::<f64, true>::from_random(&mut rng);
        let rng_seed_after = rng.seed;
        let sample = noise.sample(0.5, 0.5, 0.5, 0.0, 0.0);
        println!("origin_x: {:.15}", noise.origin_x);
        println!("origin_y: {:.15}", noise.origin_y);
        println!("origin_z: {:.15}", noise.origin_z);
        println!("permutation[0..10]: {:?}", &noise.permutation[0..10]);
        println!("sample(0.5,0.5,0.5): {:.15}", sample);
        println!("rng_seed_after_construction: {}", rng_seed_after);
    }

    #[test]
    fn beta_failure_point_clamp() {
        let noise = ImprovedNoise::<f64, true>::from_random(&mut LegacyRandom::new(845));
        // Normal coordinate — must not panic and return a finite value
        let v = noise.sample(100.0, 100.0, 100.0, 0.0, 0.0);
        assert!(v.is_finite());
        // Far coordinate beyond i32 range — must not panic
        let far = 3.0e10_f64;
        let v2 = noise.sample(far, far, far, 0.0, 0.0);
        assert!(v2.is_finite(), "sample at far coordinate must not panic or produce NaN");
    }

    #[test]
    fn modern_parity() {
        let noise = ImprovedNoise::<f32, false>::from_random(&mut LegacyRandom::new(845));
        assert_eq!(
            format!("{:.4}", noise.origin_x),
            format!("{:.4}", 179.49111938476562)
        );
        assert_eq!(
            format!("{:.4}", noise.origin_y),
            format!("{:.4}", 107.30737304687500)
        );
        assert_eq!(
            format!("{:.4}", noise.origin_z),
            format!("{:.4}", 178.89801025390625)
        );
        assert_eq!(
            noise.permutation[0..10],
            [94, 33, 237, 68, 205, 82, 207, 125, 202, 111]
        );

        let noise = ImprovedNoise::<f32, false>::from_random(&mut LegacyRandom::new(845));
        assert_eq!(
            format!("{:.4}", noise.sample(0.0, 0.0, 0.0, 0.0, 0.0)),
            format!("{:.4}", 0.107102148234844)
        );
        assert_eq!(
            format!("{:.4}", noise.sample(0.5, 4.0, -2.0, 0.0, 0.0)),
            format!("{:.4}", -0.055061601102352)
        );
        assert_eq!(
            format!("{:.4}", noise.sample(-204.0, 28.0, 12.0, 0.0, 0.0)),
            format!("{:.4}", 0.150881990790367)
        );
    }
}
