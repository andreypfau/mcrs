use crate::kernel::{Runs, at, each_column};
use crate::noise::normal::NoiseSampler;
use crate::noise::stack::ColumnScratch;
use crate::volume::Volume;
use bevy_math::IVec3;
use std::cell::RefCell;
use std::sync::Arc;

const SHIFT_B_VALUE_FACTOR: f32 = 4.0;

thread_local! {
    static COLUMN: RefCell<(Vec<f64>, ColumnScratch)> = RefCell::new(Default::default());
}

#[derive(Clone, Debug)]
pub struct NoiseFunctionParams {
    noise: Arc<NoiseSampler>,
    xz_scale: f64,
    y_scale: f64,
}

impl NoiseFunctionParams {
    pub fn new(noise: Arc<NoiseSampler>, xz_scale: f64, y_scale: f64) -> Self {
        Self {
            noise,
            xz_scale,
            y_scale,
        }
    }

    /// `shift_b` reads the noise transposed and scales its result by 4, but the
    /// coordinate factor is the same 0.25 a plain `shift` uses.
    pub fn shift_b(noise: Arc<NoiseSampler>) -> Self {
        Self::new(noise, 0.25, 0.25)
    }

    /// The whole extent in one batch, which is the point: every octave hoists its
    /// per-layer setup across all of it rather than across one column.
    pub fn eval_plain(&self, out: &mut [f32], ext: &Volume) {
        out.fill(0.0);
        self.noise
            .add_to_volume(out, ext, self.xz_scale, self.y_scale, 1.0);
    }

    /// World Z becomes the noise X axis and world X its Y axis, so the extent is
    /// transposed rather than walked column by column. Y-fastest in the transposed
    /// volume is X-fastest in the stratum, which is the layout an `X|Z` buffer has.
    pub fn eval_shift_b(&self, out: &mut [f32], ext: &Volume) {
        let (size, min, step) = (ext.size(), ext.min_block(), ext.step_block());
        debug_assert_eq!(size.y, 1, "shift_b does not vary along Y");
        let transposed = Volume::new(
            IVec3::new(size.z, size.x, 1),
            IVec3::new(min.z, min.x, 0),
            IVec3::new(step.z, step.x, 1),
        );
        out.fill(0.0);
        self.noise.add_to_volume(
            out,
            &transposed,
            self.xz_scale,
            self.y_scale,
            SHIFT_B_VALUE_FACTOR,
        );
    }

    /// Unlike [`Self::eval_plain`], this forms each octave's coordinate as
    /// `coordinate * frequency` rather than folding the scale into the frequency:
    /// vanilla's shifted volume path calls `Noise.get` per position instead of
    /// `addToVolume`, and the two associations differ by an f64 ulp. Widening the
    /// batch past one column would move it onto the other association, so the
    /// columns stay the unit here however large the extent is.
    pub fn eval_shifted(
        &self,
        out: &mut [f32],
        xs: Runs<'_>,
        ys: Runs<'_>,
        zs: Runs<'_>,
        ext: &Volume,
    ) {
        COLUMN.with_borrow_mut(|(scaled_ys, scratch)| {
            each_column(out, ext, |run, ix, iz| {
                let (xs, ys, zs) = (xs.col(ix, iz), ys.col(ix, iz), zs.col(ix, iz));
                let base_x = ext.block_x(ix as i32) as f64 * self.xz_scale;
                let base_z = ext.block_z(iz as i32) as f64 * self.xz_scale;
                let block_y = |i: usize| ext.block_y(i as i32) as f64 * self.y_scale;
                if xs.len() > 1 || zs.len() > 1 {
                    for (i, slot) in run.iter_mut().enumerate() {
                        *slot = self.noise.get(
                            base_x + at(xs, i) as f64,
                            block_y(i) + at(ys, i) as f64,
                            base_z + at(zs, i) as f64,
                        );
                    }
                    return;
                }
                let x = base_x + xs[0] as f64;
                let z = base_z + zs[0] as f64;
                scaled_ys.clear();
                scaled_ys.extend((0..run.len()).map(|i| block_y(i) + at(ys, i) as f64));
                self.noise.get_column(x, z, scaled_ys, run, scratch);
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strata::{AXIS_X, AXIS_Y, AXIS_Z, NO_AXES};
    use mcrs_minecraft_random::RandomSource;

    fn sampler(seed: u64, first_octave: i32, amplitudes: Vec<f32>) -> Arc<NoiseSampler> {
        Arc::new(NoiseSampler::new(
            &mut RandomSource::new(seed, false),
            first_octave,
            amplitudes,
        ))
    }

    fn noise(seed: u64) -> Arc<NoiseSampler> {
        sampler(seed, -6, vec![1.0, 1.0, 1.0])
    }

    fn column(bx: i32, bz: i32, min_y: i32, step_y: i32, sy: usize) -> Volume {
        Volume::new(
            IVec3::new(1, sy as i32, 1),
            IVec3::new(bx, min_y, bz),
            IVec3::new(1, step_y, 1),
        )
    }

    fn shifted(p: &NoiseFunctionParams, ext: &Volume, xs: &[f32], ys: &[f32], zs: &[f32]) -> Vec<f32> {
        let axes = |run: &[f32]| if run.len() == 1 { NO_AXES } else { AXIS_Y };
        let mut out = vec![0.0f32; ext.len()];
        p.eval_shifted(
            &mut out,
            Runs::new(xs, axes(xs), ext),
            Runs::new(ys, axes(ys), ext),
            Runs::new(zs, axes(zs), ext),
            ext,
        );
        out
    }

    #[test]
    fn a_scalar_run_agrees_with_the_head_of_the_full_run() {
        let p = NoiseFunctionParams::new(noise(7), 0.25, 0.125);
        let c = column(-9, 13, -8, 2, 5);
        let head = column(-9, 13, -8, 2, 1);

        let mut run = [0.0f32; 5];
        p.eval_plain(&mut run, &c);
        let mut first = [0.0f32; 1];
        p.eval_plain(&mut first, &head);
        assert_eq!(first[0].to_bits(), run[0].to_bits());

        let shifted_run = shifted(&p, &c, &[0.3], &[-0.2], &[0.7]);
        let shifted_head = shifted(&p, &head, &[0.3], &[-0.2], &[0.7]);
        assert_eq!(shifted_head[0].to_bits(), shifted_run[0].to_bits());
    }

    /// Widening the batch is the whole point of the node-major fill, so a box and
    /// the columns it is made of must not disagree by a bit.
    #[test]
    fn a_whole_extent_batch_agrees_with_the_columns_it_covers() {
        let p = NoiseFunctionParams::new(noise(23), 0.37, 0.11);
        let ext = Volume::new(
            IVec3::new(3, 5, 2),
            IVec3::new(-9, -8, 13),
            IVec3::new(2, 4, 3),
        );
        let mut box_out = vec![0.0f32; ext.len()];
        p.eval_plain(&mut box_out, &ext);

        for iz in 0..2 {
            for ix in 0..3 {
                let c = column(ext.block_x(ix), ext.block_z(iz), -8, 4, 5);
                let mut run = [0.0f32; 5];
                p.eval_plain(&mut run, &c);
                let base = ext.index_unchecked(ix, 0, iz);
                for (i, value) in run.iter().enumerate() {
                    assert_eq!(
                        box_out[base + i].to_bits(),
                        value.to_bits(),
                        "{ix},{i},{iz}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_hoisted_column_is_bit_exact_against_point_by_point_get() {
        let n = noise(11);
        let p = NoiseFunctionParams::new(n.clone(), 0.25, 0.125);
        let c = column(-9, 13, -8, 2, 5);
        let (sx, sy, sz) = (0.3f32, -0.2f32, 0.7f32);

        let out = shifted(&p, &c, &[sx], &[sy], &[sz]);
        for (i, value) in out.iter().enumerate() {
            let expected = n.get(
                -9.0 * 0.25 + sx as f64,
                c.block_y(i as i32) as f64 * 0.125 + sy as f64,
                13.0 * 0.25 + sz as f64,
            );
            assert_eq!(value.to_bits(), expected.to_bits());
        }
    }

    #[test]
    fn a_shift_that_varies_along_y_falls_back_to_the_point_path() {
        let n = noise(11);
        let p = NoiseFunctionParams::new(n.clone(), 0.25, 0.125);
        let c = column(4, -6, 0, 1, 3);
        let xs = [0.1f32, 0.9, -0.4];
        let zs = [0.5f32, -0.5, 0.25];

        let out = shifted(&p, &c, &xs, &[0.0], &zs);
        for (i, value) in out.iter().enumerate() {
            let expected = n.get(
                4.0 * 0.25 + xs[i] as f64,
                c.block_y(i as i32) as f64 * 0.125,
                -6.0 * 0.25 + zs[i] as f64,
            );
            assert_eq!(value.to_bits(), expected.to_bits());
        }
    }

    #[test]
    fn a_power_of_two_scale_makes_the_two_coordinate_associations_coincide() {
        let n = noise(5);
        let p = NoiseFunctionParams::new(n.clone(), 0.25, 0.125);
        for bx in -40..40 {
            let c = column(bx, 3, -2, 1, 4);
            let mut out = [0.0f32; 4];
            p.eval_plain(&mut out, &c);
            for (i, value) in out.iter().enumerate() {
                let scalar = n.get(
                    bx as f64 * 0.25,
                    c.block_y(i as i32) as f64 * 0.125,
                    3.0 * 0.25,
                );
                assert_eq!(value.to_bits(), scalar.to_bits());
            }
        }
    }

    #[test]
    fn eval_plain_folds_the_scale_into_the_frequency() {
        // `block * (xz_scale * frequency)` and `(block * xz_scale) * frequency` differ
        // by an f64 ulp, which survives the narrowing to f32 only rarely; a sweep this
        // wide at a non-power-of-two scale is what it takes to catch one. Reusing the
        // point kernel here would silently unify them and zero this count.
        let n = sampler(9, 0, vec![1.0; 9]);
        let p = NoiseFunctionParams::new(n.clone(), 0.6, 0.0);
        let mut differing = 0;
        for bx in -10000..10000 {
            let mut out = [0.0f32; 1];
            p.eval_plain(&mut out, &column(bx, 7333, 0, 1, 1));
            let scalar = n.get(bx as f64 * 0.6, 0.0, 7333.0 * 0.6);
            differing += (out[0].to_bits() != scalar.to_bits()) as usize;
        }
        assert_eq!(differing, 1);
    }

    fn shift_b_at(p: &NoiseFunctionParams, bx: i32, bz: i32, by: i32) -> f32 {
        let mut out = [0.0f32; 1];
        p.eval_shift_b(&mut out, &Volume::point(IVec3::new(bx, by, bz)));
        out[0]
    }

    #[test]
    fn shift_b_transposes_x_and_z_and_ignores_y() {
        let n = noise(3);
        let p = NoiseFunctionParams::shift_b(n.clone());

        let low = shift_b_at(&p, 5, -17, -60);
        assert_eq!(low.to_bits(), shift_b_at(&p, 5, -17, 100).to_bits());
        assert_ne!(low, shift_b_at(&p, -17, 5, 0));

        // The volume path folds the factor of 4 into every layer's amplitude where the
        // point path applies it once after the sum, so this is close, not bit-exact.
        let point = n.get(-17.0 * 0.25, 5.0 * 0.25, 0.0) * SHIFT_B_VALUE_FACTOR;
        assert!((low - point).abs() < 1e-4, "{low} vs {point}");
    }

    #[test]
    fn a_shift_b_extent_lays_its_columns_out_x_fastest() {
        let p = NoiseFunctionParams::shift_b(noise(3));
        let ext = Volume::new(
            IVec3::new(3, 1, 2),
            IVec3::new(11, -64, -4),
            IVec3::new(4, 1, 4),
        );
        let mut out = vec![0.0f32; 6];
        p.eval_shift_b(&mut out, &ext);
        for iz in 0..2 {
            for ix in 0..3 {
                let want = shift_b_at(&p, 11 + 4 * ix, -4 + 4 * iz, -64);
                assert_eq!(out[(ix + iz * 3) as usize].to_bits(), want.to_bits());
            }
        }
    }

    /// `Runs` is what makes an absent axis free: the same buffer is read by every
    /// column that shares the value.
    #[test]
    fn an_x_z_shift_run_is_shared_by_every_position_in_the_column() {
        let ext = Volume::new(IVec3::new(2, 3, 2), IVec3::ZERO, IVec3::ONE);
        let data = [1.0f32, 2.0, 3.0, 4.0];
        let runs = Runs::new(&data, AXIS_X | AXIS_Z, &ext);
        assert_eq!(runs.col(1, 1), &[4.0]);
        assert_eq!(
            Runs::new(&data[..3], AXIS_Y, &ext).col(1, 1),
            &[1.0, 2.0, 3.0]
        );
    }
}
