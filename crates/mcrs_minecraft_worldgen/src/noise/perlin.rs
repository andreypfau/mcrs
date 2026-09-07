use crate::jmath::{lerp, mul_add, mul_add64};
use crate::noise::gradient::{GradientNoise, NoiseFloat, wrap};
use crate::volume::Volume;
use mcrs_minecraft_random::Random;

/// One Perlin octave in the modern value width.
#[derive(Debug, Clone, PartialEq)]
pub struct PerlinNoise(GradientNoise);

/// A Perlin octave whose y fraction is quantised to a multiple of
/// `fudge_y_scale` before the gradient dot, which smears the lattice along the
/// vertical. Only `BlendedNoise` uses it; vanilla marks it deprecated.
#[derive(Debug, Clone, PartialEq)]
pub struct SmearedPerlinNoise {
    base: GradientNoise,
    fudge_y_scale: f64,
}

impl PerlinNoise {
    pub fn from_random<T: Random>(random: &mut T) -> Self {
        Self(GradientNoise::from_random(random))
    }

    pub fn from_gradient(base: GradientNoise) -> Self {
        Self(base)
    }

    #[inline(always)]
    pub fn get(&self, x: f64, y: f64, z: f64) -> f32 {
        let mut out = [0.0f32; 1];
        self.0.column::<false>(x, z, &[y], 0.0, &mut out);
        out[0]
    }

    /// Samples one x/z column: the lattice hashes and the x/z smoothsteps are computed once,
    /// and the eight corner gradients only when the y lattice cell changes.
    #[inline]
    pub fn get_column(&self, x: f64, z: f64, ys: &[f64], out: &mut [f32]) {
        debug_assert_eq!(ys.len(), out.len());
        self.0.column::<false>(x, z, ys, 0.0, out);
    }

    pub fn legacy_fill(
        &self,
        out: &mut [f32],
        offset: [f64; 3],
        size: [usize; 3],
        scale: [f64; 3],
        amplitude: f32,
    ) {
        self.0.legacy_fill(out, offset, size, scale, amplitude);
    }

    /// Accumulates `amplitude * sample` over `volume`, Z outer / X middle / Y inner.
    pub fn add_to_volume(
        &self,
        out: &mut [f32],
        volume: &Volume,
        xz_scale: f64,
        y_scale: f64,
        amplitude: f32,
    ) {
        self.0
            .volume::<false>(out, volume, xz_scale, y_scale, 0.0, amplitude);
    }
}

/// Beta's `ySize == 1` branch: the y lattice index is pinned to zero, the y
/// fraction to zero, and the y origin is not added at all. Sampling the 3D path
/// at y = 0 would add the octave's own y origin and land in a different lattice
/// cell, so this is a separate noise over the same lattice rather than a special
/// case of [`PerlinNoise`].
#[derive(Debug, Clone, PartialEq)]
pub struct LegacyPerlin2dNoise(GradientNoise);

impl LegacyPerlin2dNoise {
    pub fn from_gradient(base: GradientNoise) -> Self {
        Self(base)
    }

    /// Coordinates stay `f64` to the floor, as they do in Beta: narrowing them
    /// first would quantise the lattice to whole blocks a few hundred thousand
    /// blocks out.
    #[inline]
    pub fn get_xz(&self, x: f64, z: f64) -> f32 {
        let shifted_x = wrap(x) + self.0.offset_x;
        let shifted_z = wrap(z) + self.0.offset_z;
        let floor_x = shifted_x.floor();
        let floor_z = shifted_z.floor();
        self.0.sample_and_lerp(
            floor_x as i32,
            0,
            floor_z as i32,
            (shifted_x - floor_x) as f32,
            0.0,
            (shifted_z - floor_z) as f32,
            0.0,
        )
    }

    /// One value per column, repeated down it.
    pub fn add_to_volume(
        &self,
        out: &mut [f32],
        volume: &Volume,
        xz_scale: f64,
        amplitude: f32,
    ) {
        let size = volume.size();
        let mut index = 0usize;
        for iz in 0..size.z {
            let z = volume.block_z(iz) as f64 * xz_scale;
            for ix in 0..size.x {
                let value = amplitude * self.get_xz(volume.block_x(ix) as f64 * xz_scale, z);
                for _ in 0..size.y {
                    out[index] += value;
                    index += 1;
                }
            }
        }
    }
}

impl SmearedPerlinNoise {
    pub fn from_random<T: Random>(random: &mut T, fudge_y_scale: f64) -> Self {
        Self {
            base: GradientNoise::from_random(random),
            fudge_y_scale,
        }
    }

    pub fn from_gradient(base: GradientNoise, fudge_y_scale: f64) -> Self {
        Self {
            base,
            fudge_y_scale,
        }
    }

    pub fn fudge_y_scale(&self) -> f64 {
        self.fudge_y_scale
    }

    /// `ys` arrive unfolded: the lattice reads them through [`wrap`], while the
    /// smear quantises against the value before it.
    #[inline]
    pub fn get_column(&self, x: f64, z: f64, ys: &[f64], out: &mut [f32]) {
        debug_assert_eq!(ys.len(), out.len());
        self.base
            .column::<true>(x, z, ys, self.fudge_y_scale, out);
    }

    pub fn add_to_volume(
        &self,
        out: &mut [f32],
        volume: &Volume,
        xz_scale: f64,
        y_scale: f64,
        amplitude: f32,
    ) {
        self.base.volume::<true>(
            out,
            volume,
            xz_scale,
            y_scale,
            self.fudge_y_scale,
            amplitude,
        );
    }
}

/// The y fraction after the smear. `original_y` is the coordinate before
/// [`wrap`]; at `SMEARED == false` the whole thing folds away.
#[inline(always)]
fn fudged_local_y<const SMEARED: bool>(
    local_y: f64,
    original_y: f64,
    fudge_y_scale: f64,
) -> f64 {
    if !SMEARED || fudge_y_scale == 0.0 {
        return local_y;
    }
    let t = if original_y >= 0.0 && original_y < local_y {
        original_y
    } else {
        local_y
    };
    local_y - ((t / fudge_y_scale + 1.0E-7f32 as f64).floor() as i32) as f64 * fudge_y_scale
}

impl GradientNoise {
    #[inline(always)]
    pub(crate) fn sample_and_lerp(
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

    #[inline]
    fn column<const SMEARED: bool>(
        &self,
        x: f64,
        z: f64,
        ys: &[f64],
        fudge_y_scale: f64,
        out: &mut [f32],
    ) {
        let shifted_x = wrap(x) + self.offset_x;
        let shifted_z = wrap(z) + self.offset_z;
        let floor_x = shifted_x.floor();
        let floor_z = shifted_z.floor();
        let local_x = (shifted_x - floor_x) as f32;
        let local_z = (shifted_z - floor_z) as f32;
        let section_z = floor_z as i32;
        let fade_x = smoothstep(local_x);
        let fade_z = smoothstep(local_z);
        let (p0, p1) = self.x_perms(floor_x as i32);

        let mut cell: Option<(i32, CellBlend)> = None;
        for (&y, slot) in ys.iter().zip(out.iter_mut()) {
            let shifted_y = wrap(y) + self.offset_y;
            let floor_y = shifted_y.floor();
            let local_y = shifted_y - floor_y;
            let fudged = fudged_local_y::<SMEARED>(local_y, y, fudge_y_scale);
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
                fudged as f32,
                local_z,
                fade_x,
                smoothstep(local_y as f32),
                fade_z,
            );
        }
    }

    /// The block coordinate is multiplied by the *already combined* scale, so this
    /// forms `block * (outer_scale * frequency)` where [`Self::column`] forms
    /// `(block * outer_scale) * frequency`. The two differ by an f64 ulp at
    /// non-power-of-two frequencies, and vanilla ships that difference.
    fn volume<const SMEARED: bool>(
        &self,
        out: &mut [f32],
        volume: &Volume,
        xz_scale: f64,
        y_scale: f64,
        fudge_y_scale: f64,
        amplitude: f32,
    ) {
        let size = volume.size();
        let mut index = 0usize;
        for iz in 0..size.z {
            let shifted_z = wrap(volume.block_z(iz) as f64 * xz_scale) + self.offset_z;
            let floor_z = shifted_z.floor();
            let local_z = (shifted_z - floor_z) as f32;
            let section_z = floor_z as i32;
            let fade_z = smoothstep(local_z);

            for ix in 0..size.x {
                let shifted_x = wrap(volume.block_x(ix) as f64 * xz_scale) + self.offset_x;
                let floor_x = shifted_x.floor();
                let local_x = (shifted_x - floor_x) as f32;
                let fade_x = smoothstep(local_x);
                let (p0, p1) = self.x_perms(floor_x as i32);
                let mut cell: Option<(i32, CellBlend)> = None;

                for iy in 0..size.y {
                    let original_y = volume.block_y(iy) as f64 * y_scale;
                    let shifted_y = wrap(original_y) + self.offset_y;
                    let floor_y = shifted_y.floor();
                    let local_y = shifted_y - floor_y;
                    let fudged = fudged_local_y::<SMEARED>(local_y, original_y, fudge_y_scale);
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
                    out[index] = mul_add(
                        amplitude,
                        blend.sample(
                            local_x,
                            fudged as f32,
                            local_z,
                            fade_x,
                            smoothstep(local_y as f32),
                            fade_z,
                        ),
                        out[index],
                    );
                    index += 1;
                }
            }
        }
    }
}

impl GradientNoise {
    /// Beta's bulk fill, which is **not** a function of position: the eight
    /// corner dot products are computed once per y lattice cell and reused for
    /// every later sample in that cell, keeping the first sample's fractional y
    /// even as the fade advances. Sampling the same coordinates pointwise gives
    /// different terrain, so the grid — its origin, extent and step — is part of
    /// the definition.
    ///
    /// `out` is accumulated into, so the caller zeroes it. The axes are the
    /// caller's to permute: Beta hands world Z to the y argument for the surface
    /// noises. Iteration is x outer, z middle, y inner.
    pub fn legacy_fill(
        &self,
        out: &mut [f32],
        offset: [f64; 3],
        size: [usize; 3],
        scale: [f64; 3],
        amplitude: f32,
    ) {
        let mut index = 0usize;
        let mut cached_cell = -1i32;
        let (mut lower_near, mut upper_near) = (0.0f32, 0.0f32);
        let (mut lower_far, mut upper_far) = (0.0f32, 0.0f32);

        for ix in 0..size[0] {
            let x = mul_add64(offset[0] + ix as f64, scale[0], self.offset_x);
            let floor_x = x.floor();
            let perm_x = (floor_x as i32 & 0xFF) as usize;
            let local_x = (x - floor_x) as f32;
            let fade_x = smoothstep(local_x);

            for iz in 0..size[2] {
                let z = mul_add64(offset[2] + iz as f64, scale[2], self.offset_z);
                let floor_z = z.floor();
                let perm_z = (floor_z as i32 & 0xFF) as usize;
                let local_z = (z - floor_z) as f32;
                let fade_z = smoothstep(local_z);

                for iy in 0..size[1] {
                    let y = mul_add64(offset[1] + iy as f64, scale[1], self.offset_y);
                    let floor_y = y.floor();
                    let cell = floor_y as i32 & 0xFF;
                    let local_y = (y - floor_y) as f32;
                    let fade_y = smoothstep(local_y);

                    if iy == 0 || cell != cached_cell {
                        cached_cell = cell;
                        let a = self.permute(perm_x).wrapping_add(cell as usize);
                        let a0 = self.permute(a).wrapping_add(perm_z);
                        let a1 = self.permute(a.wrapping_add(1)).wrapping_add(perm_z);
                        let b = self
                            .permute(perm_x.wrapping_add(1))
                            .wrapping_add(cell as usize);
                        let b0 = self.permute(b).wrapping_add(perm_z);
                        let b1 = self.permute(b.wrapping_add(1)).wrapping_add(perm_z);
                        let x1 = local_x - 1.0;
                        let y1 = local_y - 1.0;
                        let z1 = local_z - 1.0;

                        let d000 = f32::grad_dot(self.permute(a0), local_x, local_y, local_z);
                        let d100 = f32::grad_dot(self.permute(b0), x1, local_y, local_z);
                        let d010 = f32::grad_dot(self.permute(a1), local_x, y1, local_z);
                        let d110 = f32::grad_dot(self.permute(b1), x1, y1, local_z);
                        let d001 =
                            f32::grad_dot(self.permute(a0.wrapping_add(1)), local_x, local_y, z1);
                        let d101 = f32::grad_dot(self.permute(b0.wrapping_add(1)), x1, local_y, z1);
                        let d011 = f32::grad_dot(self.permute(a1.wrapping_add(1)), local_x, y1, z1);
                        let d111 = f32::grad_dot(self.permute(b1.wrapping_add(1)), x1, y1, z1);

                        lower_near = lerp(fade_x, d000, d100);
                        upper_near = lerp(fade_x, d010, d110);
                        lower_far = lerp(fade_x, d001, d101);
                        upper_far = lerp(fade_x, d011, d111);
                    }

                    let near = lerp(fade_y, lower_near, upper_near);
                    let far = lerp(fade_y, lower_far, upper_far);
                    out[index] = mul_add(amplitude, lerp(fade_z, near, far), out[index]);
                    index += 1;
                }
            }
        }
    }
}

#[inline(always)]
pub(crate) fn smoothstep(t: f32) -> f32 {
    t * t * t * mul_add(t, mul_add(t, 6.0, -15.0), 10.0)
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
    let x1 = local_x - 1.0;
    let y1 = local_y - 1.0;
    let z1 = local_z - 1.0;

    let d000 = f32::grad_dot_at(grads[0], local_x, local_y, local_z);
    let d100 = f32::grad_dot_at(grads[1], x1, local_y, local_z);
    let d010 = f32::grad_dot_at(grads[2], local_x, y1, local_z);
    let d110 = f32::grad_dot_at(grads[3], x1, y1, local_z);
    let d001 = f32::grad_dot_at(grads[4], local_x, local_y, z1);
    let d101 = f32::grad_dot_at(grads[5], x1, local_y, z1);
    let d011 = f32::grad_dot_at(grads[6], local_x, y1, z1);
    let d111 = f32::grad_dot_at(grads[7], x1, y1, z1);

    let l00 = lerp(fade_x, d000, d100);
    let l10 = lerp(fade_x, d010, d110);
    let l01 = lerp(fade_x, d001, d101);
    let l11 = lerp(fade_x, d011, d111);
    let ll0 = lerp(fade_y, l00, l10);
    let ll1 = lerp(fade_y, l01, l11);
    lerp(fade_z, ll0, ll1)
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
        let x1 = local_x - 1.0;
        let z1 = local_z - 1.0;
        let c000 = f32::grad_dot_xz_at(grads[0], local_x, local_z);
        let c100 = f32::grad_dot_xz_at(grads[1], x1, local_z);
        let c010 = f32::grad_dot_xz_at(grads[2], local_x, local_z);
        let c110 = f32::grad_dot_xz_at(grads[3], x1, local_z);
        let c001 = f32::grad_dot_xz_at(grads[4], local_x, z1);
        let c101 = f32::grad_dot_xz_at(grads[5], x1, z1);
        let c011 = f32::grad_dot_xz_at(grads[6], local_x, z1);
        let c111 = f32::grad_dot_xz_at(grads[7], x1, z1);

        let y000 = f32::grad_y_at(grads[0]);
        let y100 = f32::grad_y_at(grads[1]);
        let y010 = f32::grad_y_at(grads[2]);
        let y110 = f32::grad_y_at(grads[3]);
        let y001 = f32::grad_y_at(grads[4]);
        let y101 = f32::grad_y_at(grads[5]);
        let y011 = f32::grad_y_at(grads[6]);
        let y111 = f32::grad_y_at(grads[7]);

        Self {
            lower_at_0: lerp(fade_z, lerp(fade_x, c000, c100), lerp(fade_x, c001, c101)),
            lower_slope: lerp(fade_z, lerp(fade_x, y000, y100), lerp(fade_x, y001, y101)),
            upper_at_0: lerp(fade_z, lerp(fade_x, c010, c110), lerp(fade_x, c011, c111)),
            upper_slope: lerp(fade_z, lerp(fade_x, y010, y110), lerp(fade_x, y011, y111)),
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
        lerp(fade_y, lower, upper)
    }
}#[cfg(test)]
mod collapsed_cell {
    use super::{CellCorners, CellLine, smoothstep};

    fn ordered(v: f32) -> i64 {
        let bits = v.to_bits() as i64;
        if bits < 0 {
            0x8000_0000i64 - bits
        } else {
            bits
        }
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
}#[cfg(test)]
mod modern {
    use crate::noise::gradient::GradientNoise;
    use crate::noise::perlin::{PerlinNoise, SmearedPerlinNoise};
    use mcrs_minecraft_random::legacy::LegacyRandom;

    /// Vanilla `GradientNoise` keeps its offsets in double regardless of the lattice
    /// precision, so the modern sampler must hold the undegraded f64 draws.
    #[test]
    fn modern_offset_is_vanilla() {
        use mcrs_minecraft_random::Random;
        let noise = GradientNoise::from_random(&mut LegacyRandom::new(845));
        let mut rng = LegacyRandom::new(845);
        let expected_x = rng.next_f64() * 256.0;
        let expected_y = rng.next_f64() * 256.0;
        let expected_z = rng.next_f64() * 256.0;
        assert_eq!(
            noise.offset_x, expected_x,
            "offset_x must equal vanilla next_f64()*256"
        );
        assert_eq!(
            noise.offset_y, expected_y,
            "offset_y must equal vanilla next_f64()*256"
        );
        assert_eq!(
            noise.offset_z, expected_z,
            "offset_z must equal vanilla next_f64()*256"
        );
    }

    #[test]
    fn column_matches_per_position_bit_for_bit() {
        let lattice = GradientNoise::from_random(&mut LegacyRandom::new(845));
        let plain = PerlinNoise::from_gradient(lattice.clone());
        // Steps far below one lattice cell, so consecutive entries reuse the hoisted corners.
        for (y_step, fudge) in [(0.03_f64, 0.0_f64), (0.03, 0.25), (0.37, 2.0), (1.5, 0.0)] {
            let smeared = SmearedPerlinNoise::from_gradient(lattice.clone(), fudge);
            for xi in 0..7 {
                for zi in 0..7 {
                    let x = -12.5 + xi as f64 * 3.7;
                    let z = 7.25 + zi as f64 * 5.3;
                    let ys: Vec<f64> = (0..49).map(|k| -64.0 + k as f64 * y_step).collect();
                    let mut column = vec![0.0f32; ys.len()];
                    if fudge == 0.0 {
                        plain.get_column(x, z, &ys, &mut column);
                    } else {
                        smeared.get_column(x, z, &ys, &mut column);
                    }
                    for (j, &y) in ys.iter().enumerate() {
                        let mut one = [0.0f32; 1];
                        if fudge == 0.0 {
                            one[0] = plain.get(x, y, z);
                        } else {
                            smeared.get_column(x, z, &[y], &mut one);
                        }
                        assert_eq!(
                            column[j].to_bits(),
                            one[0].to_bits(),
                            "column hoist diverged at x={x} z={z} y={y} fudge={fudge}"
                        );
                    }
                }
            }
        }
    }
}
