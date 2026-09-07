use crate::noise::gradient::{GradientNoise, NoiseFloat};
use mcrs_minecraft_random::Random;

/// One Perlin octave in the Beta 1.7.3 value width. Beta computes noise in
/// double where the modern path uses float, and its gradient function differs
/// for hashes 4..7, so the two widths are separate types rather than one
/// generic.
#[derive(Debug, Clone, PartialEq)]
pub struct BetaPerlinNoise(GradientNoise);

impl BetaPerlinNoise {
    pub fn from_random<T: Random>(random: &mut T) -> Self {
        Self(GradientNoise::from_random(random))
    }

    pub fn from_gradient(base: GradientNoise) -> Self {
        Self(base)
    }

    pub fn gradient(&self) -> &GradientNoise {
        &self.0
    }

    #[inline(always)]
    pub fn sample_beta_3d(&self, x: f64, y: f64, z: f64) -> f64 {
        self.0.sample_beta_3d_f64(x, y, z)
    }

    #[inline(always)]
    pub fn sample(&self, x: f64, y: f64, z: f64) -> f64 {
        self.0.sample_f64(x, y, z)
    }
}

impl GradientNoise {
    /// 3D sample matching Java's `NoiseGeneratorPerlin.a(double[],x,y,z,xSize,ySize,zSize,...)`
    /// 3D branch (ySize > 1). Uses the Java-exact gradient function — gradients for j=4..7
    /// differ from the standard Ken Perlin table.
    ///
    #[inline(always)]
    pub fn sample_beta_3d_f64(&self, x: f64, y: f64, z: f64) -> f64 {
        let shifted_x = x + self.origin_x;
        let shifted_y = y + self.origin_y;
        let shifted_z = z + self.origin_z;

        let sx = shifted_x.floor() as i32;
        let sy = shifted_y.floor() as i32;
        let sz = shifted_z.floor() as i32;

        let lx = shifted_x - sx as f64;
        let ly = shifted_y - sy as f64;
        let lz = shifted_z - sz as f64;

        let fade_y = ly;

        let perm = &self.permutation;
        let p = |i: usize| perm[i & 0xFF] as usize;

        let x0 = (sx & 0xFF) as usize;
        let x1 = (sx.wrapping_add(1) & 0xFF) as usize;
        let p0 = p(x0);
        let p1 = p(x1);

        let iy = sy as usize;
        let p00 = p(p0.wrapping_add(iy));
        let p10 = p(p1.wrapping_add(iy));
        let p01 = p(p0.wrapping_add(iy).wrapping_add(1));
        let p11 = p(p1.wrapping_add(iy).wrapping_add(1));

        let iz = sz as usize;
        let h000 = p(p00.wrapping_add(iz));
        let h100 = p(p10.wrapping_add(iz));
        let h010 = p(p01.wrapping_add(iz));
        let h110 = p(p11.wrapping_add(iz));
        let h001 = p(p00.wrapping_add(iz).wrapping_add(1));
        let h101 = p(p10.wrapping_add(iz).wrapping_add(1));
        let h011 = p(p01.wrapping_add(iz).wrapping_add(1));
        let h111 = p(p11.wrapping_add(iz).wrapping_add(1));

        let lx1 = lx - 1.0;
        let ly1 = fade_y - 1.0;
        let lz1 = lz - 1.0;

        let d000 = f64::grad_dot(h000, lx, fade_y, lz);
        let d100 = f64::grad_dot(h100, lx1, fade_y, lz);
        let d010 = f64::grad_dot(h010, lx, ly1, lz);
        let d110 = f64::grad_dot(h110, lx1, ly1, lz);
        let d001 = f64::grad_dot(h001, lx, fade_y, lz1);
        let d101 = f64::grad_dot(h101, lx1, fade_y, lz1);
        let d011 = f64::grad_dot(h011, lx, ly1, lz1);
        let d111 = f64::grad_dot(h111, lx1, ly1, lz1);

        let fx = fade_curve(lx);
        let fy = fade_curve(ly);
        let fz = fade_curve(lz);

        let l00 = lerp(fx, d000, d100);
        let l10 = lerp(fx, d010, d110);
        let l01 = lerp(fx, d001, d101);
        let l11 = lerp(fx, d011, d111);
        let ll0 = lerp(fy, l00, l10);
        let ll1 = lerp(fy, l01, l11);
        lerp(fz, ll0, ll1)
    }

    /// Bulk fill matching Java's `NoiseGeneratorPerlin.a(double[], d0, d1, d2, i, j, k, d3, d4, d5, d6)`
    /// 3D branch (ySize > 1).
    ///
    /// Replicates the Java optimization where the y-lattice cache (`i1`) persists across all
    /// (x, z) column iterations. This differs from per-point evaluation: when consecutive (x,z)
    /// columns have overlapping y-lattice indices, the x-z gradient planes are reused from the
    /// previous column. This matches Java Beta terrain generation exactly.
    ///
    /// `out` is accumulated (+=), caller must zero it first.
    /// `x_start, y_start, z_start`: grid origin (d0, d1, d2)
    /// `x_size, y_size, z_size`: grid dimensions (i, j, k)
    /// `x_scale, y_scale, z_scale`: pre-multiplied scales (d3, d4, d5 = scale * d6)
    /// `inv_freq`: 1.0 / d6
    pub fn fill_3d_bulk(
        &self,
        out: &mut [f64],
        x_start: f64,
        y_start: f64,
        z_start: f64,
        x_size: usize,
        y_size: usize,
        z_size: usize,
        x_scale: f64,
        y_scale: f64,
        z_scale: f64,
        inv_freq: f64,
    ) {
        self.fill_3d_bulk_at::<f64>(
            out, x_start, y_start, z_start, x_size, y_size, z_size, x_scale, y_scale, z_scale,
            inv_freq,
        );
    }

    /// Scalar trilinear-lerp sample for the Beta f64 path.
    ///
    /// Applies the failurePoint clamp (full i32 range, no-op near origin) before floor,
    /// then mirrors the trilinear-lerp algorithm of `sample_and_lerp` in safe scalar f64.
    #[inline(always)]
    pub fn sample_f64(&self, x: f64, y: f64, z: f64) -> f64 {
        let shifted_x = x + self.origin_x;
        let shifted_y = y + self.origin_y;
        let shifted_z = z + self.origin_z;

        let clamp_max = i32::MAX as f64;
        let clamp_min = i32::MIN as f64;
        let section_x = shifted_x
            .max(clamp_min)
            .min(clamp_max)
            .floor() as i32;
        let section_y = shifted_y
            .max(clamp_min)
            .min(clamp_max)
            .floor() as i32;
        let section_z = shifted_z
            .max(clamp_min)
            .min(clamp_max)
            .floor() as i32;

        let local_x = shifted_x - section_x as f64;
        let local_y = shifted_y - section_y as f64;
        let local_z = shifted_z - section_z as f64;

        let fade_y = local_y;

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

        let d000 = f64::grad_dot(h000, local_x, fade_y, local_z);
        let d100 = f64::grad_dot(h100, lx1, fade_y, local_z);
        let d010 = f64::grad_dot(h010, local_x, ly1, local_z);
        let d110 = f64::grad_dot(h110, lx1, ly1, local_z);
        let d001 = f64::grad_dot(h001, local_x, fade_y, lz1);
        let d101 = f64::grad_dot(h101, lx1, fade_y, lz1);
        let d011 = f64::grad_dot(h011, local_x, ly1, lz1);
        let d111 = f64::grad_dot(h111, lx1, ly1, lz1);

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
#[inline(always)]
fn fade_curve<V: NoiseFloat>(t: V) -> V {
    let six = V::from_f64(6.0);
    let fifteen = V::from_f64(15.0);
    let ten = V::from_f64(10.0);
    t * t * t * (t * (t * six - fifteen) + ten)
}

#[inline(always)]
fn lerp<V: NoiseFloat>(t: V, a: V, b: V) -> V {
    a + t * (b - a)
}
impl GradientNoise {
    /// The bulk fill at a chosen value width.
    ///
    /// Lattice and coordinates stay `f64` — vanilla keeps positions in double on
    /// every path — and only the sampled value, its fades and the accumulation
    /// follow `V`. At `V = f64` this is the Beta kernel unchanged; at `V = f32`
    /// it is the same kernel in the modern path's width.
    pub fn fill_3d_bulk_at<V: NoiseFloat>(
        &self,
        out: &mut [V],
        x_start: f64,
        y_start: f64,
        z_start: f64,
        x_size: usize,
        y_size: usize,
        z_size: usize,
        x_scale: f64,
        y_scale: f64,
        z_scale: f64,
        inv_freq: f64,
    ) {
        let perm = &self.permutation;
        let p = |i: usize| perm[i & 0xFF] as usize;
        let one = V::from_f64(1.0);
        let inv_freq = V::from_f64(inv_freq);

        let mut idx = 0usize;
        let mut y_lattice_cache: i32 = -1;
        let mut d16 = V::from_f64(0.0);
        let mut d7c = V::from_f64(0.0);
        let mut d17 = V::from_f64(0.0);
        let mut d8c = V::from_f64(0.0);

        for j1 in 0..x_size {
            let x_coord = (x_start + j1 as f64) * x_scale + self.origin_x;
            let xi = x_coord.floor() as i32;
            let l1 = (xi & 0xFF) as usize;
            let lx = V::from_f64(x_coord - xi as f64);
            let fade_x = fade_curve(lx);

            for l3 in 0..z_size {
                let z_coord = (z_start + l3 as f64) * z_scale + self.origin_z;
                let zi = z_coord.floor() as i32;
                let j4 = (zi & 0xFF) as usize;
                let lz = V::from_f64(z_coord - zi as f64);
                let fade_z = fade_curve(lz);

                for k4 in 0..y_size {
                    let y_coord = (y_start + k4 as f64) * y_scale + self.origin_y;
                    let yi = y_coord.floor() as i32;
                    let i5 = yi & 0xFF;
                    let ly = V::from_f64(y_coord - yi as f64);
                    let fade_y = fade_curve(ly);

                    if k4 == 0 || i5 != y_lattice_cache {
                        y_lattice_cache = i5;
                        let i5u = i5 as usize;
                        // Java: j5 = d[l1] + i5  (add outside permutation lookup)
                        let j5 = p(l1).wrapping_add(i5u);
                        let k5 = p(j5).wrapping_add(j4);
                        let l5 = p(j5.wrapping_add(1)).wrapping_add(j4);
                        let i6 = p(l1.wrapping_add(1)).wrapping_add(i5u);
                        let j2 = p(i6).wrapping_add(j4);
                        let j6 = p(i6.wrapping_add(1)).wrapping_add(j4);

                        d16 = lerp(
                            fade_x,
                            V::grad_dot(p(k5), lx, ly, lz),
                            V::grad_dot(p(j2), lx - one, ly, lz),
                        );
                        d7c = lerp(
                            fade_x,
                            V::grad_dot(p(l5), lx, ly - one, lz),
                            V::grad_dot(p(j6), lx - one, ly - one, lz),
                        );
                        d17 = lerp(
                            fade_x,
                            V::grad_dot(p(k5.wrapping_add(1)), lx, ly, lz - one),
                            V::grad_dot(p(j2.wrapping_add(1)), lx - one, ly, lz - one),
                        );
                        d8c = lerp(
                            fade_x,
                            V::grad_dot(p(l5.wrapping_add(1)), lx, ly - one, lz - one),
                            V::grad_dot(p(j6.wrapping_add(1)), lx - one, ly - one, lz - one),
                        );
                    }

                    let d22 = lerp(fade_y, d16, d7c);
                    let d23 = lerp(fade_y, d17, d8c);
                    let d24 = lerp(fade_z, d22, d23);
                    out[idx] = out[idx] + d24 * inv_freq;
                    idx += 1;
                }
            }
        }
    }
}#[cfg(test)]
mod test {
    use crate::noise::beta::perlin::BetaPerlinNoise;
    use mcrs_minecraft_random::legacy::LegacyRandom;
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
        serde_json::from_str(include_str!("fixtures/seed_845.json"))
            .expect("valid fixture JSON")
    }

    #[test]
    fn beta_improved_noise_origin() {
        let fx = load_fixture().improved_noise_beta;
        let mut random = LegacyRandom::new(845);
        let noise = BetaPerlinNoise::from_random(&mut random);
        assert_eq!(
            random.seed, fx.rng_seed_after_construction,
            "rng seed after construction mismatch"
        );
        assert!(
            (noise.gradient().origin_x - fx.origin_x).abs() < 1e-6,
            "origin_x mismatch: got {}, expected {}",
            noise.gradient().origin_x,
            fx.origin_x
        );
        assert!(
            (noise.gradient().origin_y - fx.origin_y).abs() < 1e-6,
            "origin_y mismatch: got {}, expected {}",
            noise.gradient().origin_y,
            fx.origin_y
        );
        assert!(
            (noise.gradient().origin_z - fx.origin_z).abs() < 1e-6,
            "origin_z mismatch: got {}, expected {}",
            noise.gradient().origin_z,
            fx.origin_z
        );
    }

    #[test]
    fn beta_improved_noise_permutation() {
        let fx = load_fixture().improved_noise_beta;
        let noise = BetaPerlinNoise::from_random(&mut LegacyRandom::new(845));
        assert_eq!(
            &noise.gradient().permutation[0..10],
            fx.permutation_first_10.as_slice()
        );
    }

    #[test]
    fn beta_improved_noise_sample() {
        let fx = load_fixture().improved_noise_beta;
        let noise = BetaPerlinNoise::from_random(&mut LegacyRandom::new(845));
        let got = noise.sample(0.5, 0.5, 0.5);
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
        let noise = BetaPerlinNoise::from_random(&mut rng);
        let rng_seed_after = rng.seed;
        let sample = noise.sample(0.5, 0.5, 0.5);
        println!("origin_x: {:.15}", noise.gradient().origin_x);
        println!("origin_y: {:.15}", noise.gradient().origin_y);
        println!("origin_z: {:.15}", noise.gradient().origin_z);
        println!("permutation[0..10]: {:?}", &noise.gradient().permutation[0..10]);
        println!("sample(0.5,0.5,0.5): {:.15}", sample);
        println!("rng_seed_after_construction: {}", rng_seed_after);
    }

    #[test]
    fn beta_failure_point_clamp() {
        let noise = BetaPerlinNoise::from_random(&mut LegacyRandom::new(845));
        // Normal coordinate — must not panic and return a finite value
        let v = noise.sample(100.0, 100.0, 100.0);
        assert!(v.is_finite());
        // Far coordinate beyond i32 range — must not panic
        let far = 3.0e10_f64;
        let v2 = noise.sample(far, far, far);
        assert!(
            v2.is_finite(),
            "sample at far coordinate must not panic or produce NaN"
        );
    }

}
