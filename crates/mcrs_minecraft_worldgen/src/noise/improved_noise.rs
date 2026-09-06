use crate::noise::gradient::{GRADIENTS, NoiseFloat};
use crate::noise::octave_perlin_noise::OctavePerlinNoise;
use crate::volume::Volume;
use mcrs_minecraft_random::{Random, RandomSource};
use num_traits::{Float, ToPrimitive};
use std::marker::PhantomData;

#[derive(Debug, Clone, PartialEq)]
pub struct ImprovedNoise<F: Float> {
    permutation: [u8; 256],
    pub origin_x: f64,
    pub origin_y: f64,
    pub origin_z: f64,
    marker: PhantomData<F>,
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
        let origin_x = random.next_f64() * 256.0;
        let origin_y = random.next_f64() * 256.0;
        let origin_z = random.next_f64() * 256.0;
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
            marker: PhantomData,
        }
    }
}

/// Java-exact 3D gradient, matching `NoiseGeneratorPerlin.a(int,double,double,double)`.
/// Java picks a component and a sign per axis; the shared table does it as a dot.
#[inline(always)]
fn grad3_java(hash: usize, x: f64, y: f64, z: f64) -> f64 {
    f64::grad_dot(hash, x, y, z)
}

/// Java-exact 2D gradient (ySize==1 branch), matching `NoiseGeneratorPerlin.a(int,double,double)`.
/// x_frac=d0, z_frac=d1 per Java's array-fill variable names.
#[inline(always)]
fn grad2_java(hash: usize, x_frac: f64, z_frac: f64) -> f64 {
    f64::grad_dot_xz(hash, x_frac, z_frac)
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
            let t = if y_max >= 0.0 && y_max < ly {
                y_max
            } else {
                ly
            };
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

        let d000 = grad3_java(h000, lx, fade_y, lz);
        let d100 = grad3_java(h100, lx1, fade_y, lz);
        let d010 = grad3_java(h010, lx, ly1, lz);
        let d110 = grad3_java(h110, lx1, ly1, lz);
        let d001 = grad3_java(h001, lx, fade_y, lz1);
        let d101 = grad3_java(h101, lx1, fade_y, lz1);
        let d011 = grad3_java(h011, lx, ly1, lz1);
        let d111 = grad3_java(h111, lx1, ly1, lz1);

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
        let d00 = grad2_java(h00, lx, lz);
        let d10 = grad3_java(h10, lx - 1.0, 0.0, lz);
        let d01 = grad3_java(h01, lx, 0.0, lz - 1.0);
        let d11 = grad3_java(h11, lx - 1.0, 0.0, lz - 1.0);

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
        let section_x = shifted_x
            .max(clamp_min)
            .min(clamp_max)
            .floor()
            .to_i32()
            .unwrap_or(i32::MAX);
        let section_y = shifted_y
            .max(clamp_min)
            .min(clamp_max)
            .floor()
            .to_i32()
            .unwrap_or(i32::MAX);
        let section_z = shifted_z
            .max(clamp_min)
            .min(clamp_max)
            .floor()
            .to_i32()
            .unwrap_or(i32::MAX);

        let local_x = shifted_x - section_x as f64;
        let local_y = shifted_y - section_y as f64;
        let local_z = shifted_z - section_z as f64;

        let mut fade = 0.0_f64;
        if y_scale != 0.0 {
            let t = if y_max >= 0.0 && y_max < local_y {
                y_max
            } else {
                local_y
            };
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
        let d100 = grad3(h100, lx1, fade_y, local_z);
        let d010 = grad3(h010, local_x, ly1, local_z);
        let d110 = grad3(h110, lx1, ly1, local_z);
        let d001 = grad3(h001, local_x, fade_y, lz1);
        let d101 = grad3(h101, lx1, fade_y, lz1);
        let d011 = grad3(h011, local_x, ly1, lz1);
        let d111 = grad3(h111, lx1, ly1, lz1);

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
        let shifted_x = x + self.origin_x as f32;
        let shifted_z = z + self.origin_z as f32;
        let section_x = shifted_x.floor() as i32;
        let section_z = shifted_z.floor() as i32;
        let local_x = shifted_x - section_x as f32;
        let local_z = shifted_z - section_z as f32;
        // y lattice pinned to 0, y fraction = 0 (no fade on y axis)
        self.sample_and_lerp(section_x, 0, section_z, local_x, 0.0, local_z, 0.0)
    }

    #[inline(always)]
    pub fn sample(&self, x: f64, y: f64, z: f64, y_scale: f64, y_max: f64) -> f32 {
        let mut out = [0.0f32; 1];
        self.sample_column(x, z, &[y], y_scale, &[y_max], &mut out);
        out[0]
    }

    /// Samples one x/z column: the lattice hashes and the x/z smoothsteps are computed once,
    /// and the eight corner gradients only when the y lattice cell changes.
    ///
    /// An empty `y_maxes` means y_max = 0.0 for every position.
    #[inline]
    pub fn sample_column(
        &self,
        x: f64,
        z: f64,
        ys: &[f64],
        y_scale: f64,
        y_maxes: &[f64],
        out: &mut [f32],
    ) {
        debug_assert_eq!(ys.len(), out.len());
        debug_assert!(y_maxes.is_empty() || y_maxes.len() == ys.len());
        self.sample_column_iter(
            x,
            z,
            ys.iter()
                .enumerate()
                .map(|(j, &y)| (y, if y_maxes.is_empty() { 0.0 } else { y_maxes[j] })),
            y_scale,
            out,
        );
    }

    #[inline]
    fn sample_column_iter<I>(&self, x: f64, z: f64, ys: I, y_scale: f64, out: &mut [f32])
    where
        I: Iterator<Item = (f64, f64)>,
    {
        let shifted_x = x + self.origin_x;
        let shifted_z = z + self.origin_z;
        let floor_x = shifted_x.floor();
        let floor_z = shifted_z.floor();
        let local_x = (shifted_x - floor_x) as f32;
        let local_z = (shifted_z - floor_z) as f32;
        let section_z = floor_z as i32;
        let fade_x = smoothstep(local_x);
        let fade_z = smoothstep(local_z);
        let (p0, p1) = self.x_perms(floor_x as i32);

        let mut cell: Option<(i32, CellBlend)> = None;
        for ((y, y_max), slot) in ys.zip(out.iter_mut()) {
            let shifted_y = y + self.origin_y;
            let floor_y = shifted_y.floor();
            let local_y = shifted_y - floor_y;
            let mut fade = 0.0_f64;
            if y_scale != 0.0 {
                let t = if y_max >= 0.0 && y_max < local_y {
                    y_max
                } else {
                    local_y
                };
                fade = ((t / y_scale + 1.0E-7f32 as f64).floor() as i32) as f64 * y_scale;
            }
            let section_y = floor_y as i32;
            let blend = match cell {
                Some((cached_y, blend)) if cached_y == section_y => blend,
                _ => {
                    let blend = cell_blend(
                        &self.corner_grads(p0, p1, section_y, section_z),
                        local_x,
                        local_z,
                        fade_x,
                        fade_z,
                    );
                    cell = Some((section_y, blend));
                    blend
                }
            };
            *slot = blend.sample(
                local_x,
                (local_y - fade) as f32,
                local_z,
                fade_x,
                smoothstep(local_y as f32),
                fade_z,
            );
        }
    }

    #[inline(always)]
    fn x_perms(&self, section_x: i32) -> (usize, usize) {
        // Masking with 0xFF is what proves the reads in bounds, as on the Beta path.
        let perm = &self.permutation;
        (
            perm[(section_x & 0xFF) as usize] as usize,
            perm[(section_x.wrapping_add(1) & 0xFF) as usize] as usize,
        )
    }

    #[inline(always)]
    fn corner_grads(&self, p0: usize, p1: usize, section_y: i32, section_z: i32) -> [usize; 8] {
        {
            let perm = &self.permutation;
            let sy = section_y as usize;
            let p4 = perm[p0.wrapping_add(sy) & 0xFF] as usize;
            let p5 = perm[p1.wrapping_add(sy) & 0xFF] as usize;
            let p6 = perm[p0.wrapping_add(sy).wrapping_add(1) & 0xFF] as usize;
            let p7 = perm[p1.wrapping_add(sy).wrapping_add(1) & 0xFF] as usize;
            let sz = section_z as usize;
            let grad = |base: usize| ((perm[base & 0xFF] & 15) as usize) << 2;
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
        let (p0, p1) = self.x_perms(section_x);
        let grads = self.corner_grads(p0, p1, section_y, section_z);
        lerp_corners(
            &grads,
            local_x,
            local_y,
            local_z,
            smoothstep(local_x),
            smoothstep(fade_local_x),
            smoothstep(local_z),
        )
    }
}

impl ImprovedNoise<f32> {
    /// Accumulates `amplitude * sample` over `volume`, Z outer / X middle / Y inner.
    ///
    /// The block coordinate is multiplied by the *already combined* scale, so this
    /// forms `block * (outer_scale * frequency)` where [`Self::sample_column`] forms
    /// `(block * outer_scale) * frequency`. The two differ by an f64 ulp at
    /// non-power-of-two frequencies, and vanilla ships that difference.
    pub fn add_to_volume(
        &self,
        out: &mut [f32],
        volume: &Volume,
        xz_scale: f64,
        y_scale: f64,
        smear_scale_y: f64,
        amplitude: f32,
    ) {
        let size = volume.size();
        let mut index = 0usize;
        for iz in 0..size.z {
            let shifted_z = wrap(volume.block_z(iz) as f64 * xz_scale) + self.origin_z;
            let floor_z = shifted_z.floor();
            let local_z = (shifted_z - floor_z) as f32;
            let section_z = floor_z as i32;
            let fade_z = smoothstep(local_z);

            for ix in 0..size.x {
                let shifted_x = wrap(volume.block_x(ix) as f64 * xz_scale) + self.origin_x;
                let floor_x = shifted_x.floor();
                let local_x = (shifted_x - floor_x) as f32;
                let fade_x = smoothstep(local_x);
                let (p0, p1) = self.x_perms(floor_x as i32);
                let mut cell: Option<(i32, CellBlend)> = None;

                for iy in 0..size.y {
                    let original_y = volume.block_y(iy) as f64 * y_scale;
                    let shifted_y = wrap(original_y) + self.origin_y;
                    let floor_y = shifted_y.floor();
                    let local_y = shifted_y - floor_y;
                    let mut fade = 0.0f64;
                    if smear_scale_y != 0.0 {
                        let t = if original_y >= 0.0 && original_y < local_y {
                            original_y
                        } else {
                            local_y
                        };
                        fade = ((t / smear_scale_y + 1.0E-7f32 as f64).floor() as i32) as f64
                            * smear_scale_y;
                    }
                    let section_y = floor_y as i32;
                    let blend = match cell {
                        Some((cached_y, blend)) if cached_y == section_y => blend,
                        _ => {
                            let blend = cell_blend(
                                &self.corner_grads(p0, p1, section_y, section_z),
                                local_x,
                                local_z,
                                fade_x,
                                fade_z,
                            );
                            cell = Some((section_y, blend));
                            blend
                        }
                    };
                    out[index] += amplitude
                        * blend.sample(
                            local_x,
                            (local_y - fade) as f32,
                            local_z,
                            fade_x,
                            smoothstep(local_y as f32),
                            fade_z,
                        );
                    index += 1;
                }
            }
        }
    }
}

#[inline(always)]
fn wrap(value: f64) -> f64 {
    OctavePerlinNoise::<f32>::maintain_precission(value)
}

#[inline(always)]
fn smoothstep(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline(always)]
fn lerp_corners(
    grads: &[usize; 8],
    local_x: f32,
    local_y: f32,
    local_z: f32,
    fade_x: f32,
    fade_y: f32,
    fade_z: f32,
) -> f32 {
    {
        let x1 = local_x - 1.0;
        let y1 = local_y - 1.0;
        let z1 = local_z - 1.0;

        let dot = |corner: usize, x: f32, y: f32, z: f32| {
            f32::grad_dot_at(grads[corner], x, y, z)
        };
        let d000 = dot(0, local_x, local_y, local_z);
        let d100 = dot(1, x1, local_y, local_z);
        let d010 = dot(2, local_x, y1, local_z);
        let d110 = dot(3, x1, y1, local_z);
        let d001 = dot(4, local_x, local_y, z1);
        let d101 = dot(5, x1, local_y, z1);
        let d011 = dot(6, local_x, y1, z1);
        let d111 = dot(7, x1, y1, z1);

        let lerp = |a: f32, p0: f32, p1: f32| p0 + a * (p1 - p0);
        let l00 = lerp(fade_x, d000, d100);
        let l10 = lerp(fade_x, d010, d110);
        let l01 = lerp(fade_x, d001, d101);
        let l11 = lerp(fade_x, d011, d111);
        let ll0 = lerp(fade_y, l00, l10);
        let ll1 = lerp(fade_y, l01, l11);
        lerp(fade_z, ll0, ll1)
    }
}

#[cfg(feature = "fast")]
type CellBlend = CellLine;
#[cfg(not(feature = "fast"))]
type CellBlend = CellCorners;

#[cfg(feature = "fast")]
#[inline(always)]
fn cell_blend(
    grads: &[usize; 8],
    local_x: f32,
    local_z: f32,
    fade_x: f32,
    fade_z: f32,
) -> CellBlend {
    CellLine::new(grads, local_x, local_z, fade_x, fade_z)
}

#[cfg(not(feature = "fast"))]
#[inline(always)]
fn cell_blend(
    grads: &[usize; 8],
    _local_x: f32,
    _local_z: f32,
    _fade_x: f32,
    _fade_z: f32,
) -> CellBlend {
    CellCorners(*grads)
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub(crate) struct CellCorners(pub [usize; 8]);

#[allow(dead_code)]
impl CellCorners {
    #[inline(always)]
    pub(crate) fn sample(
        self,
        local_x: f32,
        local_y: f32,
        local_z: f32,
        fade_x: f32,
        fade_y: f32,
        fade_z: f32,
    ) -> f32 {
        lerp_corners(&self.0, local_x, local_y, local_z, fade_x, fade_y, fade_z)
    }
}

/// The trilinear blend of one lattice cell restricted to a line along y, where x, z and
/// their smoothsteps are fixed: every corner dot is affine in y, and both the x and the z
/// lerps are affine with constant weights, so the whole cell collapses to two lines that
/// the y smoothstep interpolates between.
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub(crate) struct CellLine {
    lower_at_0: f32,
    lower_slope: f32,
    upper_at_0: f32,
    upper_slope: f32,
}

#[allow(dead_code)]
impl CellLine {
    #[inline(always)]
    pub(crate) fn new(
        grads: &[usize; 8],
        local_x: f32,
        local_z: f32,
        fade_x: f32,
        fade_z: f32,
    ) -> Self {
        {
            let x1 = local_x - 1.0;
            let z1 = local_z - 1.0;
            let split = |corner: usize, x: f32, z: f32| {
                let h = grads[corner];
                (f32::grad_dot_xz_at(h, x, z), f32::grad_y_at(h))
            };
            let (c000, y000) = split(0, local_x, local_z);
            let (c100, y100) = split(1, x1, local_z);
            let (c010, y010) = split(2, local_x, local_z);
            let (c110, y110) = split(3, x1, local_z);
            let (c001, y001) = split(4, local_x, z1);
            let (c101, y101) = split(5, x1, z1);
            let (c011, y011) = split(6, local_x, z1);
            let (c111, y111) = split(7, x1, z1);

            let lerp = |a: f32, p0: f32, p1: f32| p0 + a * (p1 - p0);
            let lerp_x = |p0: f32, p1: f32| lerp(fade_x, p0, p1);
            let lerp_z = |p0: f32, p1: f32| lerp(fade_z, p0, p1);
            Self {
                lower_at_0: lerp_z(lerp_x(c000, c100), lerp_x(c001, c101)),
                lower_slope: lerp_z(lerp_x(y000, y100), lerp_x(y001, y101)),
                upper_at_0: lerp_z(lerp_x(c010, c110), lerp_x(c011, c111)),
                upper_slope: lerp_z(lerp_x(y010, y110), lerp_x(y011, y111)),
            }
        }
    }

    #[inline(always)]
    pub(crate) fn sample(
        self,
        _local_x: f32,
        local_y: f32,
        _local_z: f32,
        _fade_x: f32,
        fade_y: f32,
        _fade_z: f32,
    ) -> f32 {
        let lower = self.lower_slope.mul_add(local_y, self.lower_at_0);
        let upper = self.upper_slope.mul_add(local_y - 1.0, self.upper_at_0);
        lower + fade_y * (upper - lower)
    }
}

#[cfg(test)]
mod collapsed_cell {
    use super::{CellCorners, CellLine, smoothstep};

    fn ordered(v: f32) -> i64 {
        let bits = v.to_bits() as i64;
        if bits < 0 { 0x8000_0000i64 - bits } else { bits }
    }

    #[test]
    fn matches_the_exact_blend() {
        let mut state = 0x2545_F491_4F6C_DD1Du64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 40) as f32 / 16777216.0
        };

        let (mut differing, mut total) = (0u64, 0u64);
        let (mut max_abs, mut max_ulps, mut sum_abs) = (0.0f32, 0i64, 0.0f64);
        for _ in 0..4096 {
            let local_x = next();
            let local_z = next();
            let mut grads = [0usize; 8];
            for g in &mut grads {
                *g = ((next() * 16.0) as usize & 15) << 2;
            }
            let fade_x = smoothstep(local_x);
            let fade_z = smoothstep(local_z);
            let corners = CellCorners(grads);
            let line = CellLine::new(&grads, local_x, local_z, fade_x, fade_z);
            for _ in 0..8 {
                let local_y = next();
                let y = local_y - next() * local_y;
                let fade_y = smoothstep(local_y);
                let exact = corners.sample(local_x, y, local_z, fade_x, fade_y, fade_z);
                let fast = line.sample(local_x, y, local_z, fade_x, fade_y, fade_z);
                total += 1;
                if exact != fast {
                    differing += 1;
                    let abs = (exact - fast).abs();
                    max_abs = max_abs.max(abs);
                    max_ulps = max_ulps.max((ordered(exact) - ordered(fast)).abs());
                    sum_abs += abs as f64;
                }
            }
        }

        println!(
            "samples={total} differing={differing} ({:.2}%) max_abs={max_abs:e} max_ulps={max_ulps} mean_abs={:e}",
            100.0 * differing as f64 / total as f64,
            sum_abs / total as f64
        );
        assert!(max_abs < 1.0e-5, "collapsed blend drifted by {max_abs}");
    }
}

#[cfg(test)]
mod test {
    use crate::noise::improved_noise::ImprovedNoise;
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
        assert_eq!(
            &noise.permutation[0..10],
            fx.permutation_first_10.as_slice()
        );
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
        assert!(
            v2.is_finite(),
            "sample at far coordinate must not panic or produce NaN"
        );
    }

    /// Vanilla `GradientNoise` keeps its offsets in double regardless of the lattice
    /// precision, so the modern sampler must hold the undegraded f64 draws.
    #[test]
    fn modern_origin_is_vanilla() {
        use mcrs_minecraft_random::Random;
        let noise = ImprovedNoise::<f32>::from_random(&mut LegacyRandom::new(845));
        let mut rng = LegacyRandom::new(845);
        let expected_x = rng.next_f64() * 256.0;
        let expected_y = rng.next_f64() * 256.0;
        let expected_z = rng.next_f64() * 256.0;
        assert_eq!(
            noise.origin_x, expected_x,
            "origin_x must equal vanilla next_f64()*256"
        );
        assert_eq!(
            noise.origin_y, expected_y,
            "origin_y must equal vanilla next_f64()*256"
        );
        assert_eq!(
            noise.origin_z, expected_z,
            "origin_z must equal vanilla next_f64()*256"
        );
    }

    #[test]
    fn flat_grad_matches_gradients() {
        use crate::noise::gradient::{GRADIENTS, NoiseFloat};

        fn check<F: NoiseFloat + std::fmt::Debug>() {
            for (i, grad) in GRADIENTS.iter().enumerate() {
                let flat = F::GRAD_FLAT;
                assert_eq!(flat[i * 4].to_f64().unwrap(), grad.x, "x at {i}");
                assert_eq!(flat[i * 4 + 1].to_f64().unwrap(), grad.y, "y at {i}");
                assert_eq!(flat[i * 4 + 2].to_f64().unwrap(), grad.z, "z at {i}");
                assert_eq!(flat[i * 4 + 3].to_f64().unwrap(), 0.0, "pad at {i}");
            }
        }

        check::<f32>();
        check::<f64>();
    }

    #[test]
    fn column_matches_per_position_bit_for_bit() {
        use crate::noise::improved_noise::ImprovedNoise;
        let noise = ImprovedNoise::<f32>::from_random(&mut LegacyRandom::new(845));
        // Steps far below one lattice cell, so consecutive entries reuse the hoisted corners.
        for (y_step, y_scale, y_max) in [
            (0.03_f64, 0.0_f64, 0.0_f64),
            (0.03, 0.25, 0.4),
            (0.37, 2.0, -1.0),
            (1.5, 0.0, 0.0),
        ] {
            for xi in 0..7 {
                for zi in 0..7 {
                    let x = -12.5 + xi as f64 * 3.7;
                    let z = 7.25 + zi as f64 * 5.3;
                    let ys: Vec<f64> = (0..49).map(|k| -64.0 + k as f64 * y_step).collect();
                    let maxes = vec![y_max; ys.len()];
                    let mut column = vec![0.0f32; ys.len()];
                    noise.sample_column(x, z, &ys, y_scale, &maxes, &mut column);
                    for (j, &y) in ys.iter().enumerate() {
                        let scalar = noise.sample(x, y, z, y_scale, y_max);
                        assert_eq!(
                            column[j].to_bits(),
                            scalar.to_bits(),
                            "column hoist diverged at x={x} z={z} y={y} y_scale={y_scale}"
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod grad_tests {
    use super::grad3_java;

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
                let ours = grad3_java(hash, x, y, z);
                let java = java_grad(hash, x, y, z);
                assert_eq!(ours, java, "hash {hash} at {x},{y},{z}");
                if java != 0.0 {
                    assert_eq!(
                        ours.to_bits(),
                        java.to_bits(),
                        "hash {hash} at {x},{y},{z}"
                    );
                }
            }
        }
    }
}

impl<F: Float> ImprovedNoise<F> {
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
        let fade = |t: V| {
            let six = V::from_f64(6.0);
            let fifteen = V::from_f64(15.0);
            let ten = V::from_f64(10.0);
            t * t * t * (t * (t * six - fifteen) + ten)
        };
        let lerp_v = |t: V, a: V, b: V| a + t * (b - a);
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
            let fade_x = fade(lx);

            for l3 in 0..z_size {
                let z_coord = (z_start + l3 as f64) * z_scale + self.origin_z;
                let zi = z_coord.floor() as i32;
                let j4 = (zi & 0xFF) as usize;
                let lz = V::from_f64(z_coord - zi as f64);
                let fade_z = fade(lz);

                for k4 in 0..y_size {
                    let y_coord = (y_start + k4 as f64) * y_scale + self.origin_y;
                    let yi = y_coord.floor() as i32;
                    let i5 = yi & 0xFF;
                    let ly = V::from_f64(y_coord - yi as f64);
                    let fade_y = fade(ly);

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

                        d16 = lerp_v(
                            fade_x,
                            V::grad_dot(p(k5), lx, ly, lz),
                            V::grad_dot(p(j2), lx - one, ly, lz),
                        );
                        d7c = lerp_v(
                            fade_x,
                            V::grad_dot(p(l5), lx, ly - one, lz),
                            V::grad_dot(p(j6), lx - one, ly - one, lz),
                        );
                        d17 = lerp_v(
                            fade_x,
                            V::grad_dot(p(k5.wrapping_add(1)), lx, ly, lz - one),
                            V::grad_dot(p(j2.wrapping_add(1)), lx - one, ly, lz - one),
                        );
                        d8c = lerp_v(
                            fade_x,
                            V::grad_dot(p(l5.wrapping_add(1)), lx, ly - one, lz - one),
                            V::grad_dot(p(j6.wrapping_add(1)), lx - one, ly - one, lz - one),
                        );
                    }

                    let d22 = lerp_v(fade_y, d16, d7c);
                    let d23 = lerp_v(fade_y, d17, d8c);
                    let d24 = lerp_v(fade_z, d22, d23);
                    out[idx] = out[idx] + d24 * inv_freq;
                    idx += 1;
                }
            }
        }
    }
}
