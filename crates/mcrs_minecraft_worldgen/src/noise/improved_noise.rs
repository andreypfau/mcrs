use crate::noise::gradient::GRADIENTS;
use mcrs_random::{Random, RandomSource};
use num_traits::{Float, ToPrimitive};

// SIMD-packed f32 mirror of `GRADIENTS` for the hot f32 path (`sample_and_lerp`):
// 16 gradients × {x, y, z, pad} so each lookup is a contiguous slice. Kept in sync with
// `GRADIENTS` by the `flat_grad_matches_gradients` test.
const FLAT_SIMPLEX_GRAD: [f32; 64] = [
    1.0, 1.0, 0.0, 0.0, -1.0, 1.0, 0.0, 0.0, 1.0, -1.0, 0.0, 0.0, -1.0, -1.0, 0.0, 0.0, 1.0, 0.0,
    1.0, 0.0, -1.0, 0.0, 1.0, 0.0, 1.0, 0.0, -1.0, 0.0, -1.0, 0.0, -1.0, 0.0, 0.0, 1.0, 1.0, 0.0,
    0.0, -1.0, 1.0, 0.0, 0.0, 1.0, -1.0, 0.0, 0.0, -1.0, -1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, -1.0,
    1.0, 0.0, -1.0, 1.0, 0.0, 0.0, 0.0, -1.0, -1.0, 0.0,
];

#[derive(Debug, Clone, PartialEq)]
pub struct ImprovedNoise<F: Float> {
    permutation: [u8; 256],
    pub origin_x: F,
    pub origin_y: F,
    pub origin_z: F,
}

impl Default for ImprovedNoise<f32> {
    fn default() -> Self {
        Self::from_random(&mut RandomSource::new(0, true))
    }
}

impl<F: Float> ImprovedNoise<F> {
    pub fn from_random<T>(random: &mut T) -> Self
    where
        T: Random,
    {
        let scale: F = F::from(256.0_f64).unwrap();
        let origin_x = F::from(random.next_f64()).unwrap() * scale;
        let origin_y = F::from(random.next_f64()).unwrap() * scale;
        let origin_z = F::from(random.next_f64()).unwrap() * scale;
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

/// Java-exact 3D gradient dot-product, matching `NoiseGeneratorPerlin.a(int,double,double,double)`.
/// Indices 4-7 differ from the standard Ken Perlin table stored in GRADIENTS; using the standard
/// table produces correct signs statistically but diverges by up to ~1.4 per sample at those indices.
#[inline(always)]
fn grad3_java(hash: usize, x: f64, y: f64, z: f64) -> f64 {
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

/// Java-exact 2D gradient (ySize==1 branch), matching `NoiseGeneratorPerlin.a(int,double,double)`.
/// x_frac=d0, z_frac=d1 per Java's array-fill variable names.
#[inline(always)]
fn grad2_java(hash: usize, x_frac: f64, z_frac: f64) -> f64 {
    let j = hash & 15;
    let d2 = (1 - ((j & 8) >> 3)) as f64 * x_frac;
    let d3 = if j < 4 {
        0.0
    } else if j == 12 || j == 14 {
        x_frac
    } else {
        z_frac
    };
    let r = if (j & 1) == 0 { d2 } else { -d2 };
    let s = if (j & 2) == 0 { d3 } else { -d3 };
    r + s
}

impl ImprovedNoise<f64> {
    /// 3D sample matching Java's `NoiseGeneratorPerlin.a(double[],x,y,z,xSize,ySize,zSize,...)`
    /// 3D branch (ySize > 1). Uses the Java-exact gradient function — gradients for j=4..7
    /// differ from the standard Ken Perlin table.
    ///
    /// y_scale/y_max mirror the smear-scale parameters; pass (0.0, 0.0) when not needed.
    #[inline(always)]
    pub fn sample_beta_3d(&self, x: f64, y: f64, z: f64, y_scale: f64, y_max: f64) -> f64 {
        let shifted_x = x + self.origin_x;
        let shifted_y = y + self.origin_y;
        let shifted_z = z + self.origin_z;

        let sx = shifted_x.floor() as i32;
        let sy = shifted_y.floor() as i32;
        let sz = shifted_z.floor() as i32;

        let lx = shifted_x - sx as f64;
        let ly = shifted_y - sy as f64;
        let lz = shifted_z - sz as f64;

        let mut fade = 0.0_f64;
        if y_scale != 0.0 {
            let t = if y_max >= 0.0 && y_max < ly { y_max } else { ly };
            fade = (t / y_scale + 1.0e-7).floor() * y_scale;
        }
        let fade_y = ly - fade;

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

        let d000 = grad3_java(h000, lx,  fade_y, lz);
        let d100 = grad3_java(h100, lx1, fade_y, lz);
        let d010 = grad3_java(h010, lx,  ly1,    lz);
        let d110 = grad3_java(h110, lx1, ly1,    lz);
        let d001 = grad3_java(h001, lx,  fade_y, lz1);
        let d101 = grad3_java(h101, lx1, fade_y, lz1);
        let d011 = grad3_java(h011, lx,  ly1,    lz1);
        let d111 = grad3_java(h111, lx1, ly1,    lz1);

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
        x_start: f64, y_start: f64, z_start: f64,
        x_size: usize, y_size: usize, z_size: usize,
        x_scale: f64, y_scale: f64, z_scale: f64,
        inv_freq: f64,
    ) {
        let perm = &self.permutation;
        let p = |i: usize| perm[i & 0xFF] as usize;

        let mut idx = 0usize;
        let mut y_lattice_cache: i32 = -1;
        let mut d16 = 0.0f64;
        let mut d7c = 0.0f64;
        let mut d17 = 0.0f64;
        let mut d8c = 0.0f64;

        for j1 in 0..x_size {
            let x_coord = (x_start + j1 as f64) * x_scale + self.origin_x;
            let xi = x_coord.floor() as i32;
            let l1 = (xi & 0xFF) as usize;
            let lx = x_coord - xi as f64;
            let fade_x = fade_curve(lx);

            for l3 in 0..z_size {
                let z_coord = (z_start + l3 as f64) * z_scale + self.origin_z;
                let zi = z_coord.floor() as i32;
                let j4 = (zi & 0xFF) as usize;
                let lz = z_coord - zi as f64;
                let fade_z = fade_curve(lz);

                for k4 in 0..y_size {
                    let y_coord = (y_start + k4 as f64) * y_scale + self.origin_y;
                    let yi = y_coord.floor() as i32;
                    let i5 = yi & 0xFF;
                    let ly = y_coord - yi as f64;
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

                        d16 = lerp(fade_x,
                            grad3_java(p(k5),   lx,       ly,       lz),
                            grad3_java(p(j2),   lx - 1.0, ly,       lz));
                        d7c = lerp(fade_x,
                            grad3_java(p(l5),   lx,       ly - 1.0, lz),
                            grad3_java(p(j6),   lx - 1.0, ly - 1.0, lz));
                        d17 = lerp(fade_x,
                            grad3_java(p(k5.wrapping_add(1)), lx,       ly,       lz - 1.0),
                            grad3_java(p(j2.wrapping_add(1)), lx - 1.0, ly,       lz - 1.0));
                        d8c = lerp(fade_x,
                            grad3_java(p(l5.wrapping_add(1)), lx,       ly - 1.0, lz - 1.0),
                            grad3_java(p(j6.wrapping_add(1)), lx - 1.0, ly - 1.0, lz - 1.0));
                    }

                    let d22 = lerp(fade_y, d16, d7c);
                    let d23 = lerp(fade_y, d17, d8c);
                    let d24 = lerp(fade_z, d22, d23);
                    out[idx] += d24 * inv_freq;
                    idx += 1;
                }
            }
        }
    }

    /// 2D sample matching Java's `NoiseGeneratorPerlin.a(double[],...)` ySize==1 branch.
    ///
    /// y origin is NOT added; y lattice is pinned to index 0; y fraction is 0.
    /// Uses Java's 2D gradient function which differs from the 3D one.
    #[inline(always)]
    pub fn sample_beta_2d(&self, x: f64, z: f64) -> f64 {
        let shifted_x = x + self.origin_x;
        let shifted_z = z + self.origin_z;

        let sx = shifted_x.floor() as i32;
        let sz = shifted_z.floor() as i32;

        let lx = shifted_x - sx as f64;
        let lz = shifted_z - sz as f64;

        let perm = &self.permutation;
        let p = |i: usize| perm[i & 0xFF] as usize;

        let x0 = (sx & 0xFF) as usize;
        let x1 = (sx.wrapping_add(1) & 0xFF) as usize;
        let p0 = p(x0);
        let p1 = p(x1);

        // y lattice index is 0 (Java: `int l = this.d[i3] + 0`)
        let p00 = p(p0);
        let p10 = p(p1);

        let iz = sz as usize;
        let h00 = p(p00.wrapping_add(iz));
        let h10 = p(p10.wrapping_add(iz));
        let h01 = p(p00.wrapping_add(iz).wrapping_add(1));
        let h11 = p(p10.wrapping_add(iz).wrapping_add(1));

        // Java ySize==1 branch uses a mixed gradient strategy:
        // corner (x0,z0) calls the 2D gradient a(hash, x_frac, z_frac);
        // the other three corners call the 3D gradient a(hash, x_frac, 0.0, z_frac).
        let d00 = grad2_java(h00, lx,        lz);
        let d10 = grad3_java(h10, lx - 1.0,  0.0, lz);
        let d01 = grad3_java(h01, lx,         0.0, lz - 1.0);
        let d11 = grad3_java(h11, lx - 1.0,  0.0, lz - 1.0);

        let fx = fade_curve(lx);
        let fz = fade_curve(lz);

        let l0 = lerp(fx, d00, d10);
        let l1 = lerp(fx, d01, d11);
        lerp(fz, l0, l1)
    }

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

/// Gradient dot-product for the 16-entry Ken Perlin gradient table, shared with the simplex path.
#[inline(always)]
fn grad3(hash: usize, x: f64, y: f64, z: f64) -> f64 {
    GRADIENTS[hash & 15].dot(x, y, z)
}

#[inline(always)]
fn fade_curve(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline(always)]
fn lerp(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}

impl ImprovedNoise<f32> {
    /// 2D sample per the Beta `ySize == 1` array-sampler branch of `NoiseGeneratorPerlin.java:105-147`.
    ///
    /// Only xo/zo are added; the y origin is NOT added and the y lattice is pinned to index 0
    /// with y fraction 0. Calling the regular `sample(x, 0.0, z, ..)` would add the per-octave
    /// y origin and land in a different lattice cell — wrong for Beta scale/depth 2D nodes.
    #[inline(always)]
    pub fn sample_2d(&self, x: f32, z: f32) -> f32 {
        let shifted_x = x + self.origin_x;
        let shifted_z = z + self.origin_z;
        let section_x = shifted_x.floor() as i32;
        let section_z = shifted_z.floor() as i32;
        let local_x = shifted_x - section_x as f32;
        let local_z = shifted_z - section_z as f32;
        // y lattice pinned to 0, y fraction = 0 (no fade on y axis)
        self.sample_and_lerp(section_x, 0, section_z, local_x, 0.0, local_z, 0.0)
    }

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

    /// Batch sample: identical per-position math to `sample`, looped so the
    /// permutation table stays L1-hot across all positions of one octave.
    /// An empty `y_maxes` means y_max = 0.0 for every position.
    #[cfg(feature = "batch-noise")]
    pub fn sample_batch(
        &self,
        positions: &[(f32, f32, f32)],
        y_scale: f32,
        y_maxes: &[f32],
        results: &mut [f32],
    ) {
        debug_assert_eq!(positions.len(), results.len());
        debug_assert!(y_maxes.is_empty() || y_maxes.len() == positions.len());
        for (j, &(x, y, z)) in positions.iter().enumerate() {
            let y_max = if y_maxes.is_empty() { 0.0 } else { y_maxes[j] };
            results[j] = self.sample(x, y, z, y_scale, y_max);
        }
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
        let noise = ImprovedNoise::<f64>::from_random(&mut LegacyRandom::new(845));
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
        let noise = ImprovedNoise::<f64>::from_random(&mut LegacyRandom::new(845));
        assert_eq!(&noise.permutation[0..10], fx.permutation_first_10.as_slice());
    }

    #[test]
    fn beta_improved_noise_sample() {
        let fx = load_fixture().improved_noise_beta;
        let noise = ImprovedNoise::<f64>::from_random(&mut LegacyRandom::new(845));
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
        let noise = ImprovedNoise::<f64>::from_random(&mut rng);
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
        let noise = ImprovedNoise::<f64>::from_random(&mut LegacyRandom::new(845));
        // Normal coordinate — must not panic and return a finite value
        let v = noise.sample(100.0, 100.0, 100.0, 0.0, 0.0);
        assert!(v.is_finite());
        // Far coordinate beyond i32 range — must not panic
        let far = 3.0e10_f64;
        let v2 = noise.sample(far, far, far, 0.0, 0.0);
        assert!(v2.is_finite(), "sample at far coordinate must not panic or produce NaN");
    }

    /// Smoke test: modern f32 origin is now vanilla nextDouble()*256 (re-baselined from next_f32).
    /// Pins the new vanilla-aligned origin: f32 cast of the same f64 draws LegacyRandom(845) uses.
    #[test]
    fn modern_origin_is_vanilla() {
        use mcrs_random::Random;
        let noise = ImprovedNoise::<f32>::from_random(&mut LegacyRandom::new(845));
        let mut rng = LegacyRandom::new(845);
        let expected_x = (rng.next_f64() * 256.0) as f32;
        let expected_y = (rng.next_f64() * 256.0) as f32;
        let expected_z = (rng.next_f64() * 256.0) as f32;
        assert_eq!(noise.origin_x, expected_x, "origin_x must equal vanilla next_f64()*256 cast to f32");
        assert_eq!(noise.origin_y, expected_y, "origin_y must equal vanilla next_f64()*256 cast to f32");
        assert_eq!(noise.origin_z, expected_z, "origin_z must equal vanilla next_f64()*256 cast to f32");
    }

    #[test]
    fn flat_grad_matches_gradients() {
        use crate::noise::gradient::GRADIENTS;
        use crate::noise::improved_noise::FLAT_SIMPLEX_GRAD;
        for (i, grad) in GRADIENTS.iter().enumerate() {
            assert_eq!(FLAT_SIMPLEX_GRAD[i * 4] as f64, grad.x, "x mismatch at {i}");
            assert_eq!(FLAT_SIMPLEX_GRAD[i * 4 + 1] as f64, grad.y, "y mismatch at {i}");
            assert_eq!(FLAT_SIMPLEX_GRAD[i * 4 + 2] as f64, grad.z, "z mismatch at {i}");
            assert_eq!(FLAT_SIMPLEX_GRAD[i * 4 + 3], 0.0, "pad nonzero at {i}");
        }
    }
}
