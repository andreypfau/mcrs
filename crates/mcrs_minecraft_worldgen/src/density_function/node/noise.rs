use super::*;
use crate::density_function::branch_schedule::{BranchSchedule, Step};
use crate::density_function::proto::{
    ALL_AXES, AXIS_X, AXIS_Y, AXIS_Z, Axis, ClampArguments, ConstantValue, DensityFunctionHolder,
    DistanceMetric, GradientArguments, HashableF64, InlineReference, IntervalSelectArguments,
    NoiseHolder, NoiseParam, NoiseValue, Normalization, PowFunctionArguments, ProtoDensityFunction,
    RewriteRule, RoundFunctionArguments, RoundingMode, SingleArgumentFunction, SliceUniformAxes,
    SplineHolder, TilingMode, TwoArgumentFunction, Visitor, noise_scale_axes,
};
use crate::noise::normal_noise::{ColumnScratch, NoiseSampler};
use crate::noise::octave_perlin_noise::OctavePerlinNoise;
use crate::noise::simplex::SimplexNoise;
use crate::proto::NoiseGeneratorSettings;
use crate::spline::{RangeFunction, SplineFunction};
use bevy_math::{Curve, FloatExt, IVec3};
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::{Random, RandomSource};
use mcrs_voxel_storage::VoxelId;
use std::collections::{BTreeMap, HashMap};
use std::fmt::{Debug, Formatter};
use std::mem::swap;
use std::ops::Index;
use std::sync::Arc;
use tracing::info;

#[derive(Clone, PartialEq)]
pub(crate) struct BlendedNoise {
    pub(crate) xz_scale: f64,
    pub(crate) y_scale: f64,
    pub(crate) xz_factor: f64,
    pub(crate) y_factor: f64,
    pub(crate) smear_scale_multiplier: f32,
    pub(crate) xz_multiplier: f64,
    pub(crate) y_multiplier: f64,
    pub(crate) max_value: f32,
    pub(crate) limit_smear: f64,
    pub(crate) main_smear: f64,
    /// The fBm value factor folded into each main-noise layer, narrowed to f32
    /// per layer. Applying it once after the sum instead rounds differently.
    pub(crate) main_amplitudes: [f32; MAIN_OCTAVES],
    /// Trailing divisor applied after the combine. Modern path passes 128.0; Beta path
    /// passes 1.0 (no division) — verified against ChunkProviderGenerate.java:280-297
    /// which has NO /128 vs BlendedNoise.java:159 which does.
    pub(crate) final_divisor: f32,
    pub(crate) lower_interpolated_noise: OctavePerlinNoise<f32>,
    pub(crate) upper_interpolated_noise: OctavePerlinNoise<f32>,
    pub(crate) interpolated_noise: OctavePerlinNoise<f32>,
}

impl Debug for BlendedNoise {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlendedNoise")
            .field("xz_scale", &self.xz_scale)
            .field("y_scale", &self.y_scale)
            .field("xz_factor", &self.xz_factor)
            .field("y_factor", &self.y_factor)
            .field("smear_scale_multiplier", &self.smear_scale_multiplier)
            .field("xz_multiplier", &self.xz_multiplier)
            .field("y_multiplier", &self.y_multiplier)
            .field("max_value", &self.max_value)
            .field("final_divisor", &self.final_divisor)
            .finish()
    }
}

const LIMIT_OCTAVES: u32 = 16;
const MAIN_OCTAVES: usize = 8;
const MAIN_VALUE_FACTOR: f64 = 12.75;

fn fbm_amplitudes<const N: usize>(value_factor: f64) -> [f32; N] {
    let base = value_factor / ((1u64 << N) - 1) as f64;
    std::array::from_fn(|i| (base * (1u64 << i) as f64) as f32)
}

impl BlendedNoise {
    pub fn new(
        random: &mut RandomSource,
        xz_scale: f32,
        y_scale: f32,
        xz_factor: f64,
        y_factor: f64,
        smear_scale_multiplier: f32,
        final_divisor: f32,
    ) -> Self {
        let xz_multiplier = 684.412 * xz_scale as f64;
        let y_multiplier = 684.412 * y_scale as f64;
        let limit_smear = y_multiplier * smear_scale_multiplier as f64;
        let main_smear = limit_smear / y_factor;
        let lower_interpolated_noise = OctavePerlinNoise::<f32>::new(
            random,
            1 - LIMIT_OCTAVES as i32,
            vec![1.0; LIMIT_OCTAVES as usize],
            true,
        );
        // Every FLAT_SIMPLEX_GRAD entry has two unit components and one zero, and each
        // lattice offset it dots against stays within [-1, 1], so one octave never leaves
        // [-2, 2]; the 2^i octave weights sum to 2^16 - 1 against the /512 combine.
        let max_value = 2.0 * ((1u32 << LIMIT_OCTAVES) - 1) as f32 / 512.0 / final_divisor;
        BlendedNoise {
            xz_scale: xz_scale as f64,
            y_scale: y_scale as f64,
            xz_factor,
            y_factor,
            smear_scale_multiplier,
            xz_multiplier,
            y_multiplier,
            max_value,
            limit_smear,
            main_smear,
            main_amplitudes: fbm_amplitudes(MAIN_VALUE_FACTOR),
            final_divisor,
            lower_interpolated_noise,
            upper_interpolated_noise: OctavePerlinNoise::<f32>::new(
                random,
                1 - LIMIT_OCTAVES as i32,
                vec![1.0; LIMIT_OCTAVES as usize],
                true,
            ),
            interpolated_noise: OctavePerlinNoise::<f32>::new(random, -7, vec![1.0; 8], true),
        }
    }
}

impl RangeFunction for BlendedNoise {
    #[inline]
    fn min_value(&self) -> f32 {
        -self.max_value()
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

impl DensityFunction for BlendedNoise {
    fn sample(&self, pos: IVec3) -> f32 {
        let scaled_x = pos.x as f64 * self.xz_multiplier;
        let scaled_y = pos.y as f64 * self.y_multiplier;
        let scaled_z = pos.z as f64 * self.xz_multiplier;

        // Strength-reduce: halve coordinates each iteration instead of
        // multiplying by a separate factor variable.
        let mut fx = scaled_x / self.xz_factor;
        let mut fy = scaled_y / self.y_factor;
        let mut fz = scaled_z / self.xz_factor;
        let mut main_smear = self.main_smear;

        // Interpolated noise: 8 octaves.
        let mut value = 0.0f32;
        for i in 0..MAIN_OCTAVES {
            let s = self.interpolated_noise.sample_octave(
                i,
                OctavePerlinNoise::maintain_precission(fx),
                OctavePerlinNoise::maintain_precission(fy),
                OctavePerlinNoise::maintain_precission(fz),
                main_smear,
                fy,
            );
            value += s * self.main_amplitudes[i];
            fx *= 0.5;
            fy *= 0.5;
            fz *= 0.5;
            main_smear *= 0.5;
        }

        value += 0.5;
        let need_lower = value < 1.0;
        let need_upper = value > 0.0;
        let mut min = 0.0f32;
        let mut max = 0.0f32;

        // Separate loops for lower/upper noise to keep each OctavePerlinNoise's
        // permutation tables cache-warm during evaluation.
        if need_lower {
            let mut sx = scaled_x;
            let mut sy = scaled_y;
            let mut sz = scaled_z;
            let mut sm: f64 = self.limit_smear;
            let mut amplitude = 1.0f32;
            for i in 0..16 {
                let s = self.lower_interpolated_noise.sample_octave(
                    i,
                    OctavePerlinNoise::maintain_precission(sx),
                    OctavePerlinNoise::maintain_precission(sy),
                    OctavePerlinNoise::maintain_precission(sz),
                    sm,
                    sy,
                );
                min += s * amplitude;
                sx *= 0.5;
                sy *= 0.5;
                sz *= 0.5;
                sm *= 0.5;
                amplitude *= 2.0;
            }
        }
        if need_upper {
            let mut sx = scaled_x;
            let mut sy = scaled_y;
            let mut sz = scaled_z;
            let mut sm: f64 = self.limit_smear;
            let mut amplitude = 1.0f32;
            for i in 0..16 {
                let s = self.upper_interpolated_noise.sample_octave(
                    i,
                    OctavePerlinNoise::maintain_precission(sx),
                    OctavePerlinNoise::maintain_precission(sy),
                    OctavePerlinNoise::maintain_precission(sz),
                    sm,
                    sy,
                );
                max += s * amplitude;
                sx *= 0.5;
                sy *= 0.5;
                sz *= 0.5;
                sm *= 0.5;
                amplitude *= 2.0;
            }
        }

        let start = min / 512.0;
        let end = max / 512.0;
        value = if value < 0.0 {
            start
        } else if value > 1.0 {
            end
        } else {
            start + value * (end - start)
        };
        value / self.final_divisor
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct Noise {
    pub(crate) noise_name: String,
    pub(crate) sampler: NoiseSampler,
    pub(crate) xz_scale: f64,
    pub(crate) y_scale: f64,
}

impl Debug for Noise {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Noise")
            .field("noise_name", &self.noise_name)
            .field("xz_scale", &self.xz_scale)
            .field("y_scale", &self.y_scale)
            .field("min_value", &self.min_value())
            .field("max_value", &self.max_value())
            .finish()
    }
}

impl RangeFunction for Noise {
    #[inline]
    fn min_value(&self) -> f32 {
        -self.max_value()
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.sampler.max_value() as f32
    }
}

impl DensityFunction for Noise {
    fn sample(&self, pos: IVec3) -> f32 {
        let xz_scale = self.xz_scale;
        let y_scale = self.y_scale;
        self.sampler.get(
            pos.x as f64 * xz_scale,
            pos.y as f64 * y_scale,
            pos.z as f64 * xz_scale,
        )
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct ShiftB {
    pub(crate) noise_name: String,
    pub(crate) sampler: NoiseSampler,
}

impl Debug for ShiftB {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShiftB")
            .field("noise_name", &self.noise_name)
            .field("min_value", &self.min_value())
            .field("max_value", &self.max_value())
            .finish()
    }
}

impl RangeFunction for ShiftB {
    #[inline]
    fn min_value(&self) -> f32 {
        -self.max_value()
    }

    #[inline]
    fn max_value(&self) -> f32 {
        (self.sampler.max_value() * 4.0) as f32
    }
}

impl DensityFunction for ShiftB {
    fn sample(&self, pos: IVec3) -> f32 {
        self.sampler
            .get(pos.z as f64 * 0.25, pos.x as f64 * 0.25, 0.0)
            * 4.0
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct ShiftedNoise {
    pub(crate) noise_name: String,
    pub(crate) input_x_index: usize,
    pub(crate) input_y_index: usize,
    pub(crate) input_z_index: usize,
    pub(crate) xz_scale: f64,
    pub(crate) y_scale: f64,
    pub(crate) sampler: NoiseSampler,
}

impl Debug for ShiftedNoise {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShiftedNoise")
            .field("noise_name", &self.noise_name)
            .field("input_x_index", &self.input_x_index)
            .field("input_y_index", &self.input_y_index)
            .field("input_z_index", &self.input_z_index)
            .field("xz_scale", &self.xz_scale)
            .field("y_scale", &self.y_scale)
            .field("min_value", &self.min_value())
            .field("max_value", &self.max_value())
            .finish()
    }
}

impl RangeFunction for ShiftedNoise {
    #[inline]
    fn min_value(&self) -> f32 {
        -self.max_value()
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.sampler.max_value()
    }
}

impl DensitySampler for ShiftedNoise {
    fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
        let shift_x = ctx.row(self.input_x_index);
        let shift_y = ctx.row(self.input_y_index);
        let shift_z = ctx.row(self.input_z_index);
        for ((((slot, pos), &dx), &dy), &dz) in out
            .iter_mut()
            .zip(ctx.positions)
            .zip(shift_x)
            .zip(shift_y)
            .zip(shift_z)
        {
            *slot = self.sampler.get(
                pos.x as f64 * self.xz_scale + dx as f64,
                pos.y as f64 * self.y_scale + dy as f64,
                pos.z as f64 * self.xz_scale + dz as f64,
            );
        }
    }
}

impl DensitySampler for Noise {
    /// Y is the volume's fastest axis, so a run of positions is a column and
    /// every octave can hoist its lattice hashes across it.
    fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
        let height = ctx.volume.size().y as usize;
        if height < 2 {
            for (p, slot) in out.iter_mut().enumerate() {
                *slot = self.sample(ctx.positions[p]);
            }
            return;
        }
        let mut ys = vec![0.0f64; height];
        let mut scratch = ColumnScratch::default();
        for (column, slots) in out.chunks_mut(height).enumerate() {
            let run = &ctx.positions[column * height..column * height + height];
            for (slot, pos) in ys.iter_mut().zip(run) {
                *slot = pos.y as f64 * self.y_scale;
            }
            self.sampler.get_column(
                run[0].x as f64 * self.xz_scale,
                run[0].z as f64 * self.xz_scale,
                &ys,
                slots,
                &mut scratch,
            );
        }
    }
}

impl DensitySampler for ShiftB {
    /// Reads the noise at (z, x, 0), so its value is constant down a column.
    fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
        let height = ctx.volume.size().y as usize;
        for (column, slots) in out.chunks_mut(height).enumerate() {
            slots.fill(self.sample(ctx.positions[column * height]));
        }
    }
}
