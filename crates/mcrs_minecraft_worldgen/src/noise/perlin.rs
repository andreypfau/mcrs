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
    pub fn sample_2d(&self, x: f32, z: f32) -> f32 {
        self.0.sample_2d_f32(x, z)
    }

    #[inline(always)]
    pub fn sample(&self, x: f64, y: f64, z: f64) -> f32 {
        self.0.sample_f32(x, y, z)
    }

    /// Samples one x/z column: the lattice hashes and the x/z smoothsteps are computed once,
    /// and the eight corner gradients only when the y lattice cell changes.
    #[inline]
    pub fn sample_column(&self, x: f64, z: f64, ys: &[f64], out: &mut [f32]) {
        debug_assert_eq!(ys.len(), out.len());
        self.0.column::<false>(x, z, ys, &[], 0.0, out);
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

    /// `ys` are the wrapped coordinates the lattice is read at; `unwrapped_ys`
    /// are the same coordinates before [`wrap`], which is what the smear
    /// quantises against.
    #[inline]
    pub fn sample_column(&self, x: f64, z: f64, ys: &[f64], unwrapped_ys: &[f64], out: &mut [f32]) {
        debug_assert_eq!(ys.len(), out.len());
        debug_assert_eq!(unwrapped_ys.len(), ys.len());
        self.base
            .column::<true>(x, z, ys, unwrapped_ys, self.fudge_y_scale, out);
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
    /// 2D sample per the Beta `ySize == 1` array-sampler branch of `NoiseGeneratorPerlin.java:105-147`.
    ///
    /// Only xo/zo are added; the y origin is NOT added and the y lattice is pinned to index 0
    /// with y fraction 0. Sampling the 3D path at y = 0 would add the per-octave y origin and
    /// land in a different lattice cell — wrong for Beta scale/depth 2D nodes.
    #[inline(always)]
    pub(crate) fn sample_2d_f32(&self, x: f32, z: f32) -> f32 {
        let shifted_x = x + self.origin_x as f32;
        let shifted_z = z + self.origin_z as f32;
        let section_x = shifted_x.floor() as i32;
        let section_z = shifted_z.floor() as i32;
        let local_x = shifted_x - section_x as f32;
        let local_z = shifted_z - section_z as f32;
        self.sample_and_lerp(section_x, 0, section_z, local_x, 0.0, local_z, 0.0)
    }

    #[inline(always)]
    pub(crate) fn sample_f32(&self, x: f64, y: f64, z: f64) -> f32 {
        let mut out = [0.0f32; 1];
        self.column::<false>(x, z, &[y], &[], 0.0, &mut out);
        out[0]
    }

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
        unwrapped_ys: &[f64],
        fudge_y_scale: f64,
        out: &mut [f32],
    ) {
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
        for (j, (&y, slot)) in ys.iter().zip(out.iter_mut()).enumerate() {
            let shifted_y = y + self.origin_y;
            let floor_y = shifted_y.floor();
            let local_y = shifted_y - floor_y;
            let original_y = if SMEARED { unwrapped_ys[j] } else { 0.0 };
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
                    out[index] += amplitude
                        * blend.sample(
                            local_x,
                            fudged as f32,
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
pub(crate) fn smoothstep(t: f32) -> f32 {
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
    let x1 = local_x - 1.0;
    let y1 = local_y - 1.0;
    let z1 = local_z - 1.0;

    let dot = |corner: usize, x: f32, y: f32, z: f32| f32::grad_dot_at(grads[corner], x, y, z);
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
    fn modern_origin_is_vanilla() {
        use mcrs_minecraft_random::Random;
        let noise = GradientNoise::from_random(&mut LegacyRandom::new(845));
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
    fn column_matches_per_position_bit_for_bit() {
        let lattice = GradientNoise::from_random(&mut LegacyRandom::new(845));
        let plain = PerlinNoise::from_gradient(lattice.clone());
        // Steps far below one lattice cell, so consecutive entries reuse the hoisted corners.
        for (y_step, fudge, y_max) in [
            (0.03_f64, 0.0_f64, 0.0_f64),
            (0.03, 0.25, 0.4),
            (0.37, 2.0, -1.0),
            (1.5, 0.0, 0.0),
        ] {
            let smeared = SmearedPerlinNoise::from_gradient(lattice.clone(), fudge);
            for xi in 0..7 {
                for zi in 0..7 {
                    let x = -12.5 + xi as f64 * 3.7;
                    let z = 7.25 + zi as f64 * 5.3;
                    let ys: Vec<f64> = (0..49).map(|k| -64.0 + k as f64 * y_step).collect();
                    let maxes = vec![y_max; ys.len()];
                    let mut column = vec![0.0f32; ys.len()];
                    if fudge == 0.0 {
                        plain.sample_column(x, z, &ys, &mut column);
                    } else {
                        smeared.sample_column(x, z, &ys, &maxes, &mut column);
                    }
                    for (j, &y) in ys.iter().enumerate() {
                        let mut one = [0.0f32; 1];
                        if fudge == 0.0 {
                            one[0] = plain.sample(x, y, z);
                        } else {
                            smeared.sample_column(x, z, &[y], &[y_max], &mut one);
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
