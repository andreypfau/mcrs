use crate::density_function::branch_schedule::{BranchSchedule, Step};
use crate::density_function::proto::{
    ALL_AXES, AXIS_X, AXIS_Y, AXIS_Z, Axis, ClampArguments, DensityFunctionHolder, DistanceMetric,
    GradientArguments, HashableF64, InlineReference, IntervalSelectArguments, NoiseHolder,
    NoiseParam, NoiseValue, Normalization, PowFunctionArguments, ProtoDensityFunction, RewriteRule,
    RoundFunctionArguments, RoundingMode, SingleArgumentFunction, SliceUniformAxes, SplineHolder,
    TilingMode, TwoArgumentFunction, Visitor, noise_scale_axes,
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

pub mod beta_seed;
pub mod beta_terrain_f64;
mod branch_schedule;
mod compile;
#[cfg(test)]
mod interval_prune;
pub mod proto;
mod range;
#[cfg(test)]
mod tests;
pub mod volume;

use range::{binary_range, mul_range, pow_narrowed, unary_range};

pub use compile::build_functions;
pub use volume::{FillScratch, Volume};

trait DensityFunction: RangeFunction {
    fn sample(&self, pos: IVec3) -> f32;
}

pub struct NoiseRouter {
    temperature_index: usize,
    vegetation_index: usize,
    continents_index: usize,
    erosion_index: usize,
    depth_index: usize,
    ridges_index: usize,
    chunk_surface_level_index: usize,
    final_density_index: usize,
    noise_min_y: i32,
    noise_height: u32,
    sea_level: i32,
    default_block_state: VoxelId,
    default_fluid_state: VoxelId,
    world_seed: u64,
    /// Beta beach octave noise (4 octaves, stream position 4 in seed_beta_terrain).
    /// None for the modern router. Used by apply_beta_surface to determine beach columns.
    beta_beach_noise: Option<Box<OctavePerlinNoise<f64>>>,
    /// Beta surface octave noise (4 octaves, stream position 5 in seed_beta_terrain).
    /// None for the modern router. Used by apply_beta_surface to determine surface depth.
    beta_surface_noise: Option<Box<OctavePerlinNoise<f64>>>,
    /// f64-precision Beta terrain density noises for exact Java parity.
    /// None for the modern router. Replaces the f32 density-function tree for the Beta path.
    beta_terrain_f64: Option<Box<beta_terrain_f64::BetaTerrainF64>>,
    /// per_block[i] == true means entry i depends on Y and must be recomputed per block.
    /// per_block[i] == false means entry i is column-only (cached across Y changes).
    per_block: Box<[bool]>,
    /// Terms of `final_density` at or above every `interpolated` wrapper, ascending.
    outer_terms: Box<[usize]>,
    /// The `interpolated` wrappers `final_density` reads, ascending.
    outer_wrappers: Box<[usize]>,
    /// Stack index of each outer wrapper's input, in the same order.
    outer_wrapper_inputs: Box<[usize]>,
    /// First index of Zone B (per-Y entries for final_density).
    /// Zone A [0..column_boundary): column-only entries reachable from final_density.
    column_boundary: usize,
    /// First index of Zone C (entries not reachable from final_density).
    /// Zone B [column_boundary..fd_boundary): per-Y entries for final_density.
    fd_boundary: usize,
    /// The one cell size in blocks (typically 4 x 8 x 4) every `interpolated`
    /// wrapper `final_density` reads agrees on, or `None` when they disagree.
    cell_size: Option<IVec3>,
    stack: Box<[DensityFunctionComponent]>,
    node_labels: Box<[String]>,
    zone_b_schedule: BranchSchedule,
    zone_b_roots: Box<[usize]>,
}

impl NoiseRouter {
    /// Per stack entry, the mask of `AXIS_X` / `AXIS_Y` / `AXIS_Z` its value may
    /// vary along.
    pub fn domain_axes(&self) -> Vec<u8> {
        compile::compute_domain_axes(&self.stack)
    }

    /// All noise router entries as (name, index) pairs.
    pub fn roots(&self) -> Vec<(&'static str, usize)> {
        vec![
            ("temperature", self.temperature_index),
            ("vegetation", self.vegetation_index),
            ("continents", self.continents_index),
            ("erosion", self.erosion_index),
            ("depth", self.depth_index),
            ("ridges", self.ridges_index),
            ("chunk_surface_level", self.chunk_surface_level_index),
            ("final_density", self.final_density_index),
        ]
    }

    pub fn world_seed(&self) -> u64 {
        self.world_seed
    }

    pub fn temperature_index(&self) -> usize {
        self.temperature_index
    }

    pub fn vegetation_index(&self) -> usize {
        self.vegetation_index
    }

    pub fn final_density_index(&self) -> usize {
        self.final_density_index
    }

    pub fn noise_min_y(&self) -> i32 {
        self.noise_min_y
    }

    pub fn noise_height(&self) -> u32 {
        self.noise_height
    }

    pub fn sea_level(&self) -> i32 {
        self.sea_level
    }

    pub fn default_block_state(&self) -> VoxelId {
        self.default_block_state
    }

    pub fn default_fluid_state(&self) -> VoxelId {
        self.default_fluid_state
    }

    /// Return the Beta beach octave noise sampler (4 octaves, stream position 4).
    /// None for the modern router. Used by apply_beta_surface for beach/sand conditions.
    pub fn beta_beach_noise(&self) -> Option<&OctavePerlinNoise<f64>> {
        self.beta_beach_noise.as_deref()
    }

    /// Return the Beta surface octave noise sampler (4 octaves, stream position 5).
    /// None for the modern router. Used by apply_beta_surface for surface depth calculation.
    pub fn beta_surface_noise(&self) -> Option<&OctavePerlinNoise<f64>> {
        self.beta_surface_noise.as_deref()
    }

    /// Return the f64 Beta terrain noises, if this is a Beta router.
    /// None for the modern overworld router.
    pub fn beta_terrain_f64(&self) -> Option<&beta_terrain_f64::BetaTerrainF64> {
        self.beta_terrain_f64.as_deref()
    }

    /// Sample a 16×16 temperature grid (index = x*16+z) and a 16×16 rain/vegetation grid
    /// for the Beta `computeDensity` call. Both grids are in block coordinates starting at
    /// `(block_x, block_z)`.
    pub fn sample_beta_climate_grids(
        &self,
        block_x: i32,
        block_z: i32,
    ) -> ([f32; 256], [f32; 256]) {
        let mut temp_grid = [0.0f32; 256];
        let mut rain_grid = [0.0f32; 256];
        let volume = Volume::new(
            IVec3::new(16, 1, 16),
            IVec3::new(block_x, 0, block_z),
            IVec3::ONE,
        );
        let mut values = vec![0.0f32; 2 * volume.len()];
        self.sample_volume_roots(
            &[self.temperature_index, self.vegetation_index],
            &volume,
            &mut values,
            &mut FillScratch::new(),
        );
        for x in 0..16i32 {
            for z in 0..16i32 {
                let source = volume.index_unchecked(x, 0, z);
                temp_grid[(x * 16 + z) as usize] = values[source];
                rain_grid[(x * 16 + z) as usize] = values[volume.len() + source];
            }
        }
        (temp_grid, rain_grid)
    }

    /// Evaluate temperature and vegetation at `(block_x, block_z)`.
    pub fn sample_beta_climate(&self, block_x: i32, block_z: i32) -> (f32, f32) {
        let pos = IVec3::new(block_x, 0, block_z);
        let mut scratch = FillScratch::new();
        let temperature = self.sample_value(self.temperature_index, pos, &mut scratch);
        let humidity = self.sample_value(self.vegetation_index, pos, &mut scratch);
        (temperature, humidity)
    }

    /// Roots to fill over a cell-corner volume to feed `final_density_cell_bounds`,
    /// in the order that call expects them.
    #[inline]
    pub fn cell_value_roots(&self) -> &[usize] {
        &self.outer_wrapper_inputs
    }

    /// The cell lattice `final_density` interpolates on, or `None` when its
    /// wrappers disagree and there is no single one.
    #[inline]
    pub fn cell_size(&self) -> Option<IVec3> {
        self.cell_size
    }
}

#[derive(Clone, PartialEq)]
struct BlendedNoise {
    xz_scale: f64,
    y_scale: f64,
    xz_factor: f64,
    y_factor: f64,
    smear_scale_multiplier: f32,
    xz_multiplier: f64,
    y_multiplier: f64,
    max_value: f32,
    limit_smear: f64,
    main_smear: f64,
    /// The fBm value factor folded into each main-noise layer, narrowed to f32
    /// per layer. Applying it once after the sum instead rounds differently.
    main_amplitudes: [f32; MAIN_OCTAVES],
    /// Trailing divisor applied after the combine. Modern path passes 128.0; Beta path
    /// passes 1.0 (no division) — verified against ChunkProviderGenerate.java:280-297
    /// which has NO /128 vs BlendedNoise.java:159 which does.
    final_divisor: f32,
    lower_interpolated_noise: OctavePerlinNoise<f32>,
    upper_interpolated_noise: OctavePerlinNoise<f32>,
    interpolated_noise: OctavePerlinNoise<f32>,
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
struct Noise {
    noise_name: String,
    sampler: NoiseSampler,
    xz_scale: f64,
    y_scale: f64,
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
struct ShiftA {
    noise_name: String,
    sampler: NoiseSampler,
}

impl Debug for ShiftA {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShiftA")
            .field("noise_name", &self.noise_name)
            .field("min_value", &self.min_value())
            .field("max_value", &self.max_value())
            .finish()
    }
}

impl RangeFunction for ShiftA {
    #[inline]
    fn min_value(&self) -> f32 {
        -self.max_value()
    }

    #[inline]
    fn max_value(&self) -> f32 {
        (self.sampler.max_value() * 4.0) as f32
    }
}

impl DensityFunction for ShiftA {
    fn sample(&self, pos: IVec3) -> f32 {
        self.sampler
            .get(pos.x as f64 * 0.25, 0.0, pos.z as f64 * 0.25)
            * 4.0
    }
}

#[derive(Clone, PartialEq)]
struct ShiftB {
    noise_name: String,
    sampler: NoiseSampler,
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

#[derive(Clone, Debug, PartialEq)]
struct Shift {
    noise_name: String,
    sampler: NoiseSampler,
}

impl RangeFunction for Shift {
    #[inline]
    fn min_value(&self) -> f32 {
        -self.max_value()
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.sampler.max_value() * 4.0
    }
}

impl DensityFunction for Shift {
    fn sample(&self, pos: IVec3) -> f32 {
        self.sampler.get(
            pos.x as f64 * 0.25,
            pos.y as f64 * 0.25,
            pos.z as f64 * 0.25,
        ) * 4.0
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Interpolated {
    input_index: usize,
    input_members: Box<[u32]>,
    cell_size_xz: u32,
    cell_size_y: u32,
    cell_size_xz_inv: f32,
    cell_size_y_inv: f32,
    min_value: f32,
    max_value: f32,
}

impl RangeFunction for Interpolated {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ClampedYGradient {
    from_y: f32,
    to_y: f32,
    from_value: f32,
    to_value: f32,
}
impl RangeFunction for ClampedYGradient {
    fn min_value(&self) -> f32 {
        self.from_value.min(self.to_value)
    }

    fn max_value(&self) -> f32 {
        self.from_value.max(self.to_value)
    }
}
#[derive(Clone, Debug, PartialEq)]
struct Gradient {
    axis: Axis,
    tiling: TilingMode,
    from_coordinate: i32,
    to_coordinate: i32,
    from_value: f32,
    to_value: f32,
}

impl RangeFunction for Gradient {
    fn min_value(&self) -> f32 {
        self.from_value.min(self.to_value)
    }

    fn max_value(&self) -> f32 {
        self.from_value.max(self.to_value)
    }
}

impl DensityFunction for Gradient {
    fn sample(&self, pos: IVec3) -> f32 {
        let coordinate = match self.axis {
            Axis::X => pos.x,
            Axis::Y => pos.y,
            Axis::Z => pos.z,
        };
        let coordinate_range = self.to_coordinate - self.from_coordinate;
        let relative = coordinate - self.from_coordinate;
        let factor = match self.tiling {
            TilingMode::ClampToEdge => relative,
            TilingMode::Repeat => relative.rem_euclid(coordinate_range),
            TilingMode::MirroredRepeat => {
                let tile = relative.div_euclid(coordinate_range);
                let local = relative - tile * coordinate_range;
                if tile & 1 == 0 {
                    local
                } else {
                    coordinate_range - local
                }
            }
        };
        let t = (factor as f32 / coordinate_range as f32).clamp(0.0, 1.0);
        self.from_value + t * (self.to_value - self.from_value)
    }
}

impl DensityFunction for ClampedYGradient {
    fn sample(&self, pos: IVec3) -> f32 {
        let y = pos.y as f32;
        let from_y = self.from_y;
        if y < from_y {
            self.from_value
        } else if y > self.to_y {
            self.to_value
        } else {
            let from_value = self.from_value;
            from_value + (self.to_value - from_value) * (y - from_y) / (self.to_y - from_y)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct EndIslands {
    noise: Arc<SimplexNoise>,
}

impl EndIslands {
    fn new(world_seed: u64) -> Self {
        let mut random = LegacyRandom::new(world_seed);
        for _ in 0..17292 {
            random.next_i32();
        }
        Self {
            noise: Arc::new(SimplexNoise::from_random_at_origin(&mut random)),
        }
    }

    fn height(&self, section_x: i32, section_z: i32) -> f32 {
        let chunk_x = section_x / 2;
        let chunk_z = section_z / 2;
        let sub_x = (section_x % 2) as f32;
        let sub_z = (section_z % 2) as f32;
        let mut height = -100.0f32;
        for offset_x in -12..=12 {
            for offset_z in -12..=12 {
                let cell_x = (chunk_x + offset_x) as i64;
                let cell_z = (chunk_z + offset_z) as i64;
                if cell_x * cell_x + cell_z * cell_z <= 4096 {
                    continue;
                }
                // vanilla narrows the simplex value to f32 before the threshold test
                if self.noise.sample(cell_x as f64, cell_z as f64, 1.0, 1.0) as f32 >= -0.9 {
                    continue;
                }
                let island_size =
                    ((cell_x as f32).abs() * 3439.0 + (cell_z as f32).abs() * 147.0) % 13.0 + 9.0;
                let dx = sub_x - (offset_x * 2) as f32;
                let dz = sub_z - (offset_z * 2) as f32;
                let candidate =
                    (100.0 - (dx * dx + dz * dz).sqrt() * island_size).clamp(-100.0, 80.0);
                height = height.max(candidate);
            }
        }
        height
    }
}

impl RangeFunction for EndIslands {
    fn min_value(&self) -> f32 {
        -0.84375
    }

    fn max_value(&self) -> f32 {
        0.5625
    }
}

impl DensityFunction for EndIslands {
    fn sample(&self, pos: IVec3) -> f32 {
        (self.height(pos.x / 8, pos.z / 8) - 8.0) / 128.0
    }
}

#[derive(Clone, Debug, PartialEq)]
enum IndependentDensityFunction {
    Constant(f32),
    OldBlendedNoise(BlendedNoise),
    Noise(Noise),
    ShiftA(ShiftA),
    ShiftB(ShiftB),
    Shift(Shift),
    ClampedYGradient(ClampedYGradient),
    Gradient(Gradient),
    DistanceToPoint(DistanceToPoint),
    EndOuterIslands(EndIslands),
}

impl IndependentDensityFunction {
    /// Fill `out` a column at a time, so each octave hoists its lattice hashes
    /// across the run. Returns false when the caller must sample per position.
    pub(super) fn fill_columns(
        &self,
        volume: &Volume,
        positions: &[IVec3],
        out: &mut [f32],
    ) -> bool {
        let Self::Noise(noise) = self else {
            return false;
        };
        let height = volume.size().y as usize;
        if height < 2 {
            return false;
        }
        let mut ys = vec![0.0f64; height];
        let mut scratch = ColumnScratch::default();
        for (column, slots) in out.chunks_mut(height).enumerate() {
            let run = &positions[column * height..column * height + height];
            for (slot, pos) in ys.iter_mut().zip(run) {
                *slot = pos.y as f64 * noise.y_scale;
            }
            noise.sampler.get_column(
                run[0].x as f64 * noise.xz_scale,
                run[0].z as f64 * noise.xz_scale,
                &ys,
                slots,
                &mut scratch,
            );
        }
        true
    }
}

impl RangeFunction for IndependentDensityFunction {
    fn min_value(&self) -> f32 {
        match self {
            IndependentDensityFunction::Constant(x) => *x,
            IndependentDensityFunction::OldBlendedNoise(x) => x.min_value(),
            IndependentDensityFunction::Noise(x) => x.min_value(),
            IndependentDensityFunction::ShiftA(x) => x.min_value(),
            IndependentDensityFunction::ShiftB(x) => x.min_value(),
            IndependentDensityFunction::Shift(x) => x.min_value(),
            IndependentDensityFunction::ClampedYGradient(x) => x.min_value(),
            IndependentDensityFunction::Gradient(x) => x.min_value(),
            IndependentDensityFunction::DistanceToPoint(x) => x.min_value(),
            IndependentDensityFunction::EndOuterIslands(x) => x.min_value(),
        }
    }

    fn max_value(&self) -> f32 {
        match self {
            IndependentDensityFunction::Constant(x) => *x,
            IndependentDensityFunction::OldBlendedNoise(x) => x.max_value(),
            IndependentDensityFunction::Noise(x) => x.max_value(),
            IndependentDensityFunction::ShiftA(x) => x.max_value(),
            IndependentDensityFunction::ShiftB(x) => x.max_value(),
            IndependentDensityFunction::Shift(x) => x.max_value(),
            IndependentDensityFunction::ClampedYGradient(x) => x.max_value(),
            IndependentDensityFunction::Gradient(x) => x.max_value(),
            IndependentDensityFunction::DistanceToPoint(x) => x.max_value(),
            IndependentDensityFunction::EndOuterIslands(x) => x.max_value(),
        }
    }
}

impl DensityFunction for IndependentDensityFunction {
    fn sample(&self, pos: IVec3) -> f32 {
        match self {
            IndependentDensityFunction::Constant(x) => *x,
            IndependentDensityFunction::OldBlendedNoise(x) => x.sample(pos),
            IndependentDensityFunction::Noise(x) => x.sample(pos),
            IndependentDensityFunction::ShiftA(x) => x.sample(pos),
            IndependentDensityFunction::ShiftB(x) => x.sample(pos),
            IndependentDensityFunction::Shift(x) => x.sample(pos),
            IndependentDensityFunction::ClampedYGradient(x) => x.sample(pos),
            IndependentDensityFunction::Gradient(x) => x.sample(pos),
            IndependentDensityFunction::DistanceToPoint(x) => x.sample(pos),
            IndependentDensityFunction::EndOuterIslands(x) => x.sample(pos),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum DependentDensityFunction {
    Linear(Linear),
    Affine(Affine),
    PiecewiseAffine(PiecewiseAffine),
    Slide(Slide),
    Unary(Unary),
    Binary(Binary),
    ShiftedNoise(ShiftedNoise),
    Clamp(Clamp),
    RangeChoice(RangeChoice),
    Spline(Spline),
    FindTopSurface(FindTopSurface),
    Lerp(Lerp),
    Slice(Slice),
}

impl RangeFunction for DependentDensityFunction {
    fn min_value(&self) -> f32 {
        match self {
            DependentDensityFunction::Linear(x) => x.min_value(),
            DependentDensityFunction::Affine(x) => x.min_value(),
            DependentDensityFunction::PiecewiseAffine(x) => x.min_value(),
            DependentDensityFunction::Slide(x) => x.min_value(),
            DependentDensityFunction::Unary(x) => x.min_value(),
            DependentDensityFunction::Binary(x) => x.min_value(),
            DependentDensityFunction::ShiftedNoise(x) => x.min_value(),
            DependentDensityFunction::Clamp(x) => x.min_value(),
            DependentDensityFunction::RangeChoice(x) => x.min_value(),
            DependentDensityFunction::Spline(x) => x.min_value(),
            DependentDensityFunction::FindTopSurface(x) => x.min_value(),
            DependentDensityFunction::Lerp(x) => x.min_value(),
            DependentDensityFunction::Slice(x) => x.min_value(),
        }
    }

    fn max_value(&self) -> f32 {
        match self {
            DependentDensityFunction::Linear(x) => x.max_value(),
            DependentDensityFunction::Affine(x) => x.max_value(),
            DependentDensityFunction::PiecewiseAffine(x) => x.max_value(),
            DependentDensityFunction::Slide(x) => x.max_value(),
            DependentDensityFunction::Unary(x) => x.max_value(),
            DependentDensityFunction::Binary(x) => x.max_value(),
            DependentDensityFunction::ShiftedNoise(x) => x.max_value(),
            DependentDensityFunction::Clamp(x) => x.max_value(),
            DependentDensityFunction::RangeChoice(x) => x.max_value(),
            DependentDensityFunction::Spline(x) => x.max_value(),
            DependentDensityFunction::FindTopSurface(x) => x.max_value(),
            DependentDensityFunction::Lerp(x) => x.max_value(),
            DependentDensityFunction::Slice(x) => x.max_value(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Linear {
    input_index: usize,
    min_value: f32,
    max_value: f32,
    argument: f32,
    operation: LinearOperation,
}

#[derive(Clone, Debug, PartialEq)]
struct Affine {
    input_index: usize,
    scale: f32,
    offset: f32,
    min_value: f32,
    max_value: f32,
}

#[derive(Clone, Debug, PartialEq, Copy, Eq)]
enum LinearOperation {
    Add,
    Multiply,
}

impl Affine {
    fn compute_range(input_min: f32, input_max: f32, scale: f32, offset: f32) -> (f32, f32) {
        if scale >= 0.0 {
            (
                input_min.mul_add(scale, offset),
                input_max.mul_add(scale, offset),
            )
        } else {
            (
                input_max.mul_add(scale, offset),
                input_min.mul_add(scale, offset),
            )
        }
    }
}

impl RangeFunction for Affine {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

/// Piecewise-linear affine: different scales for negative vs non-negative input.
///
/// Replaces patterns like `Affine(Unary::QuarterNegative(x))` or
/// `Affine(Unary::HalfNegative(x))` where the unary damps the negative side.
///
/// Computes: `if x < 0 { x * neg_scale + offset } else { x * pos_scale + offset }`
#[derive(Clone, Debug, PartialEq)]
struct PiecewiseAffine {
    input_index: usize,
    neg_scale: f32,
    pos_scale: f32,
    offset: f32,
    min_value: f32,
    max_value: f32,
}

impl PiecewiseAffine {
    fn compute_range(
        input_min: f32,
        input_max: f32,
        neg_scale: f32,
        pos_scale: f32,
        offset: f32,
    ) -> (f32, f32) {
        // Two monotone pieces meeting at zero: the extremes can only sit at an
        // endpoint or at the breakpoint, and the breakpoint only counts when the
        // input interval actually straddles it.
        let apply = |x: f32| {
            if x < 0.0 {
                x * neg_scale + offset
            } else {
                x * pos_scale + offset
            }
        };
        let a = apply(input_min);
        let b = apply(input_max);
        let mut lo = a.min(b);
        let mut hi = a.max(b);
        if input_min < 0.0 && input_max >= 0.0 {
            lo = lo.min(offset);
            hi = hi.max(offset);
        }
        (lo, hi)
    }
}

impl RangeFunction for PiecewiseAffine {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

/// Fused world-boundary "slide" operation.
///
/// Replaces the 5-node chain:
///   `Affine(+a) → Mul(y_grad1) → Affine(+b) → Mul(y_grad2) → Affine(+c)`
///
/// Full computation:
///   `grad2(y) * (grad1(y) * (input + offset_a) + offset_b) + offset_c`
///
/// Fast path: when both gradients saturate to 1.0 (y in the interior range),
/// the three offsets cancel out and the result equals `input + combined_offset`
/// (which is typically ~0, i.e. identity).
#[derive(Clone, Debug, PartialEq)]
struct Slide {
    input_index: usize,

    // First Y-gradient applied (typically "top": 240..256 → 1.0..0.0)
    grad1: ClampedYGradient,
    // Second Y-gradient applied (typically "bottom": -64..-40 → 0.0..1.0)
    grad2: ClampedYGradient,

    // Three affine offsets (all original affines had scale=1.0)
    offset_a: f32, // pre-grad1
    offset_b: f32, // between grad1 and grad2
    offset_c: f32, // post-grad2

    // Pre-computed: offset_a + offset_b + offset_c
    combined_offset: f32,

    // Y range where both gradients saturate to 1.0 (fast path)
    fast_path_min_y: f32,
    fast_path_max_y: f32,

    min_value: f32,
    max_value: f32,
}

impl Slide {
    #[inline]
    fn eval_gradient(g: &ClampedYGradient, y: f32) -> f32 {
        if y < g.from_y {
            g.from_value
        } else if y > g.to_y {
            g.to_value
        } else {
            g.from_value + (g.to_value - g.from_value) * (y - g.from_y) / (g.to_y - g.from_y)
        }
    }

    #[inline]
    fn compute(&self, input: f32, y: f32) -> f32 {
        if y > self.fast_path_min_y && y < self.fast_path_max_y {
            input + self.combined_offset
        } else {
            let g1 = Self::eval_gradient(&self.grad1, y);
            let g2 = Self::eval_gradient(&self.grad2, y);
            (g1 * (input + self.offset_a) + self.offset_b).mul_add(g2, self.offset_c)
        }
    }

    /// Compute the Y range where a gradient saturates to exactly 1.0.
    /// Returns (min_y, max_y) or None if the gradient never equals 1.0.
    fn saturate_one_range(g: &ClampedYGradient) -> Option<(f32, f32)> {
        let below = g.from_value == 1.0; // y <= from_y → 1.0
        let above = g.to_value == 1.0; // y >= to_y → 1.0
        match (below, above) {
            (true, true) => Some((f32::NEG_INFINITY, f32::INFINITY)),
            (true, false) => Some((f32::NEG_INFINITY, g.from_y)),
            (false, true) => Some((g.to_y, f32::INFINITY)),
            (false, false) => None,
        }
    }
}

impl RangeFunction for Slide {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

impl RangeFunction for Linear {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Unary {
    input_index: usize,
    min_value: f32,
    max_value: f32,
    operation: UnaryOperation,
}

#[derive(Clone, Debug, PartialEq, Copy, Eq)]
enum UnaryOperation {
    Abs,
    Square,
    Cube,
    HalfNegative,
    QuarterNegative,
    Reciprocal,
    Squeeze,
    Sqrt,
    Log,
    Sign,
}

impl UnaryOperation {
    #[inline]
    pub fn apply(&self, value: f32) -> f32 {
        match self {
            UnaryOperation::Abs => value.abs(),
            UnaryOperation::Square => value.powi(2),
            UnaryOperation::Cube => value.powi(3),
            UnaryOperation::HalfNegative => {
                if value > 0.0 {
                    value
                } else {
                    value * 0.5
                }
            }
            UnaryOperation::QuarterNegative => {
                if value > 0.0 {
                    value
                } else {
                    value * 0.25
                }
            }
            UnaryOperation::Reciprocal => 1.0 / value,
            UnaryOperation::Squeeze => {
                let clamped = value.clamp(-1.0, 1.0);
                clamped / 2.0 - clamped.powi(3) / 24.0
            }
            UnaryOperation::Sqrt => value.sqrt(),
            UnaryOperation::Log => (value as f64).ln() as f32,
            // Unlike f32::signum, zero and NaN come back unchanged.
            UnaryOperation::Sign => {
                if value == 0.0 || value.is_nan() {
                    value
                } else if value > 0.0 {
                    1.0
                } else {
                    -1.0
                }
            }
        }
    }
}

impl RangeFunction for Unary {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

#[derive(Clone, PartialEq)]
struct ShiftedNoise {
    noise_name: String,
    input_x_index: usize,
    input_y_index: usize,
    input_z_index: usize,
    xz_scale: f64,
    y_scale: f64,
    sampler: NoiseSampler,
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

#[derive(Clone, Debug, PartialEq)]
struct Clamp {
    input_index: usize,
    /// The datapack bounds already narrowed to the input's own range. Sampling clamps
    /// to these rather than the raw bounds: for any value the input can produce the two
    /// agree, so the narrower pair serves as both the operation and the declared range.
    min_value: f32,
    max_value: f32,
}

impl RangeFunction for Clamp {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

#[derive(Clone, Debug, PartialEq)]
struct RangeChoice {
    input_index: usize,
    when_in_index: usize,
    when_out_index: usize,
    min_inclusion_value: f32,
    max_exclusion_value: f32,
    min_value: f32,
    max_value: f32,
}

impl RangeFunction for RangeChoice {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

#[derive(Clone, Debug, PartialEq)]
enum SplineValue {
    Spline(Spline),
    Constant(f32),
}

impl RangeFunction for SplineValue {
    fn min_value(&self) -> f32 {
        match self {
            SplineValue::Spline(x) => x.min_value(),
            SplineValue::Constant(x) => *x,
        }
    }

    fn max_value(&self) -> f32 {
        match self {
            SplineValue::Spline(x) => x.max_value(),
            SplineValue::Constant(x) => *x,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Segment {
    left: f32,
    dist: f32,             // x[i+1] - x[i]
    lower_deriv_dist: f32, // d[i]   * dist
    upper_deriv_dist: f32, // d[i+1] * dist
}

#[derive(Clone, Debug, PartialEq)]
struct Spline {
    input_index: usize,
    min_value: f32,
    max_value: f32,
    locations: Box<[f32]>,
    derivatives: Box<[f32]>,
    values: Box<[SplineValue]>,
    segments: Box<[Segment]>, // len = locations.len() - 1
}

impl Spline {
    pub fn new(
        input_index: usize,
        coordinate_min: f32,
        coordinate_max: f32,
        locations: Vec<f32>,
        derivatives: Vec<f32>,
        values: Vec<SplineValue>,
    ) -> Self {
        let n = locations.len() - 1;

        let mut min_value = f32::INFINITY;
        let mut max_value = f32::NEG_INFINITY;

        if coordinate_min < locations[0] {
            let extend_min = Self::linear_extend(
                coordinate_min,
                &locations,
                values[0].min_value(),
                &derivatives,
                0,
            );
            let extend_max = Self::linear_extend(
                coordinate_min,
                &locations,
                values[0].max_value(),
                &derivatives,
                0,
            );
            min_value = min_value.min(extend_min.min(extend_max));
            max_value = max_value.max(extend_min.max(extend_max));
        }

        if coordinate_max > locations[n] {
            let extend_min = Self::linear_extend(
                coordinate_max,
                &locations,
                values[n].min_value(),
                &derivatives,
                n,
            );
            let extend_max = Self::linear_extend(
                coordinate_max,
                &locations,
                values[n].max_value(),
                &derivatives,
                n,
            );
            min_value = min_value.min(extend_min.min(extend_max));
            max_value = max_value.max(extend_min.max(extend_max));
        }

        values.iter().for_each(|v| {
            min_value = min_value.min(v.min_value());
            max_value = max_value.max(v.max_value());
        });

        for i in 0..n {
            let location_left = locations[i];
            let location_right = locations[i + 1];
            let location_delta = location_right - location_left;

            let min_left = values[i].min_value();
            let max_left = values[i].max_value();
            let min_right = values[i + 1].min_value();
            let max_right = values[i + 1].max_value();

            let derivative_left = derivatives[i];
            let derivative_right = derivatives[i + 1];

            if derivative_left != 0.0 || derivative_right != 0.0 {
                let max_value_delta_left = derivative_left * location_delta;
                let max_value_delta_right = derivative_right * location_delta;

                let mut local_min = min_left.min(min_right);
                let mut local_max = max_left.max(max_right);

                let min_delta_left = max_value_delta_left - max_right + min_left;
                let max_delta_left = max_value_delta_left - min_right + max_left;

                let min_delta_right = -max_value_delta_right + min_right - max_left;
                let max_delta_right = -max_value_delta_right + max_right - min_left;

                let min_delta = min_delta_left.min(min_delta_right);
                let max_delta = max_delta_left.max(max_delta_right);

                local_min = local_min.min(local_min + 0.25 * min_delta);
                local_max = local_max.max(local_max + 0.25 * max_delta);

                min_value = min_value.min(local_min);
                max_value = max_value.max(local_max);
            }
        }

        let mut segs = Vec::with_capacity(n);
        for i in 0..n {
            let left = locations[i];
            let dist = locations[i + 1] - left;
            debug_assert!(dist > 0.0, "locations must be strictly increasing");
            segs.push(Segment {
                left,
                dist,
                lower_deriv_dist: derivatives[i] * dist,
                upper_deriv_dist: derivatives[i + 1] * dist,
            });
        }

        Self {
            input_index,
            min_value,
            max_value,
            locations: locations.into_boxed_slice(),
            derivatives: derivatives.into_boxed_slice(),
            values: values.into_boxed_slice(),
            segments: segs.into_boxed_slice(),
        }
    }

    #[inline]
    fn linear_extend(
        point: f32,
        locations: &[f32],
        value: f32,
        derivatives: &[f32],
        i: usize,
    ) -> f32 {
        let f = derivatives[i];
        if f == 0.0 {
            value
        } else {
            value + f * (point - locations[i])
        }
    }

    #[inline(always)]
    fn upper_bound(xs: &[f32], x: f32) -> usize {
        // index of first element > x  (upper_bound)
        match xs.binary_search_by(|v| v.total_cmp(&x)) {
            Ok(i) => i + 1,
            Err(i) => i,
        }
    }

    #[inline(always)]
    fn lerp(a: f32, b: f32, t: f32) -> f32 {
        a + t * (b - a)
    }
}

impl RangeFunction for Spline {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

impl SplineValue {
    #[inline]
    fn sample(&self, cache: &[f32]) -> f32 {
        match self {
            SplineValue::Spline(x) => x.sample(cache),
            SplineValue::Constant(x) => *x,
        }
    }
}

impl Spline {
    fn sample(&self, cache: &[f32]) -> f32 {
        let location = cache[self.input_index];

        let locs = &self.locations;
        let idx_gt = Self::upper_bound(locs, location);
        let n_points = locs.len();

        if idx_gt == 0 {
            let v0 = self.values[0].sample(cache);
            let d0 = self.derivatives[0];
            return if d0 == 0.0 {
                v0
            } else {
                v0 + d0 * (location - locs[0])
            };
        }

        if idx_gt == n_points {
            let i = n_points - 1;
            let v = self.values[i].sample(cache);
            let d = self.derivatives[i];
            return if d == 0.0 {
                v
            } else {
                v + d * (location - locs[i])
            };
        }

        let i0 = idx_gt - 1;
        let i1 = idx_gt;

        let v0 = self.values[i0].sample(cache);
        let v1 = self.values[i1].sample(cache);

        let seg = self.segments[i0];
        let x = (location - seg.left) / seg.dist;

        let delta = v1 - v0;

        let e0 = seg.lower_deriv_dist - delta;
        let e1 = -seg.upper_deriv_dist + delta;

        let cubic = (x * (1.0 - x)) * Self::lerp(e0, e1, x);
        let linear = Self::lerp(v0, v1, x);

        cubic + linear
    }
}

#[derive(Clone, Debug, PartialEq)]
struct FindTopSurface {
    density_index: usize,
    density_members: Box<[u32]>,
    upper_bound_index: usize,
    lower_bound: f32,
    cell_height: f32,
    max_value: f32,
}

#[derive(Clone, Debug, PartialEq)]
struct Lerp {
    alpha_index: usize,
    first_index: usize,
    second_index: usize,
    min_value: f32,
    max_value: f32,
}

impl RangeFunction for Lerp {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Slice {
    axis: Axis,
    coordinate: i32,
    input_index: usize,
    input_members: Box<[u32]>,
    min_value: f32,
    max_value: f32,
}

impl RangeFunction for Slice {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

#[derive(Clone, Debug, PartialEq)]
struct DistanceToPoint {
    point: IVec3,
    metric: DistanceMetric,
}

impl RangeFunction for DistanceToPoint {
    #[inline]
    fn min_value(&self) -> f32 {
        0.0
    }

    #[inline]
    fn max_value(&self) -> f32 {
        f32::INFINITY
    }
}

impl DensityFunction for DistanceToPoint {
    fn sample(&self, pos: IVec3) -> f32 {
        let d = (self.point - pos).as_vec3();
        match self.metric {
            DistanceMetric::Euclidean => d.length(),
            DistanceMetric::EuclideanSquared => d.length_squared(),
            DistanceMetric::Manhattan => d.x.abs() + d.y.abs() + d.z.abs(),
            DistanceMetric::Chebyshev => d.x.abs().max(d.y.abs()).max(d.z.abs()),
        }
    }
}

impl RangeFunction for FindTopSurface {
    #[inline]
    fn min_value(&self) -> f32 {
        self.lower_bound
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Binary {
    input1_index: usize,
    input2_index: usize,
    min_value: f32,
    max_value: f32,
    operation: BinaryOperation,
}

impl RangeFunction for Binary {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

#[derive(Clone, Debug, PartialEq, Copy, Eq)]
enum BinaryOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Min,
    Max,
    Pow,
    Round(RoundingMode),
}

impl BinaryOperation {
    #[inline]
    fn apply(self, a: f32, b: f32) -> f32 {
        match self {
            BinaryOperation::Add => a + b,
            BinaryOperation::Subtract => a - b,
            BinaryOperation::Multiply => a * b,
            BinaryOperation::Divide => {
                if a == 0.0 {
                    0.0
                } else {
                    a / b
                }
            }
            BinaryOperation::Min => a.min(b),
            BinaryOperation::Max => a.max(b),
            BinaryOperation::Pow => pow_narrowed(a, b),
            BinaryOperation::Round(mode) => {
                if b == 0.0 {
                    a
                } else {
                    round_to_integer(a / b, mode) * b
                }
            }
        }
    }
}

#[inline]
fn round_to_integer(value: f32, mode: RoundingMode) -> f32 {
    match mode {
        RoundingMode::Floor => value.floor(),
        // Java rounds halves up, not away from zero: round(-2.5) is -2.
        RoundingMode::Round => (value + 0.5).floor(),
        RoundingMode::Ceil => value.ceil(),
        RoundingMode::Truncate => {
            if value > 0.0 {
                value.floor()
            } else {
                value.ceil()
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum DensityFunctionComponent {
    Independent(IndependentDensityFunction),
    Dependent(DependentDensityFunction),
    Interpolated(Interpolated),
}

impl DensityFunctionComponent {
    fn as_constant(&self) -> Option<f32> {
        match self {
            DensityFunctionComponent::Independent(x) => match x {
                IndependentDensityFunction::Constant(v) => Some(*v),
                _ => None,
            },
            _ => None,
        }
    }
}

impl SplineValue {
    fn rewrite_indices(&mut self, redirect: &[usize]) {
        if let SplineValue::Spline(spline) = self {
            spline.rewrite_indices(redirect);
        }
    }
}

impl Spline {
    fn rewrite_indices(&mut self, redirect: &[usize]) {
        self.input_index = redirect[self.input_index];
        for value in self.values.iter_mut() {
            value.rewrite_indices(redirect);
        }
    }

    fn visit_input_indices(&self, f: &mut impl FnMut(usize)) {
        f(self.input_index);
        for value in self.values.iter() {
            if let SplineValue::Spline(nested) = value {
                nested.visit_input_indices(f);
            }
        }
    }
}

impl DensityFunctionComponent {
    fn rewrite_indices(&mut self, redirect: &[usize]) {
        match self {
            DensityFunctionComponent::Independent(_) => {}
            DensityFunctionComponent::Dependent(dep) => match dep {
                DependentDensityFunction::Linear(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::Affine(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::PiecewiseAffine(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::Slide(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::Unary(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::Binary(x) => {
                    x.input1_index = redirect[x.input1_index];
                    x.input2_index = redirect[x.input2_index];
                }
                DependentDensityFunction::ShiftedNoise(x) => {
                    x.input_x_index = redirect[x.input_x_index];
                    x.input_y_index = redirect[x.input_y_index];
                    x.input_z_index = redirect[x.input_z_index];
                }
                DependentDensityFunction::Clamp(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::RangeChoice(x) => {
                    x.input_index = redirect[x.input_index];
                    x.when_in_index = redirect[x.when_in_index];
                    x.when_out_index = redirect[x.when_out_index];
                }
                DependentDensityFunction::Spline(x) => {
                    x.rewrite_indices(redirect);
                }
                DependentDensityFunction::FindTopSurface(x) => {
                    x.density_index = redirect[x.density_index];
                    x.upper_bound_index = redirect[x.upper_bound_index];
                }
                DependentDensityFunction::Lerp(x) => {
                    x.alpha_index = redirect[x.alpha_index];
                    x.first_index = redirect[x.first_index];
                    x.second_index = redirect[x.second_index];
                }
                DependentDensityFunction::Slice(x) => {
                    x.input_index = redirect[x.input_index];
                }
            },
            DensityFunctionComponent::Interpolated(x) => {
                x.input_index = redirect[x.input_index];
            }
        }
    }

    fn visit_input_indices(&self, f: &mut impl FnMut(usize)) {
        match self {
            DensityFunctionComponent::Independent(_) => {}
            DensityFunctionComponent::Dependent(dep) => match dep {
                DependentDensityFunction::Linear(x) => f(x.input_index),
                DependentDensityFunction::Affine(x) => f(x.input_index),
                DependentDensityFunction::PiecewiseAffine(x) => f(x.input_index),
                DependentDensityFunction::Slide(x) => f(x.input_index),
                DependentDensityFunction::Unary(x) => f(x.input_index),
                DependentDensityFunction::Binary(x) => {
                    f(x.input1_index);
                    f(x.input2_index);
                }
                DependentDensityFunction::ShiftedNoise(x) => {
                    f(x.input_x_index);
                    f(x.input_y_index);
                    f(x.input_z_index);
                }
                DependentDensityFunction::Clamp(x) => f(x.input_index),
                DependentDensityFunction::RangeChoice(x) => {
                    f(x.input_index);
                    f(x.when_in_index);
                    f(x.when_out_index);
                }
                DependentDensityFunction::Spline(x) => x.visit_input_indices(f),
                DependentDensityFunction::FindTopSurface(x) => {
                    f(x.density_index);
                    f(x.upper_bound_index);
                }
                DependentDensityFunction::Lerp(x) => {
                    f(x.alpha_index);
                    f(x.first_index);
                    f(x.second_index);
                }
                DependentDensityFunction::Slice(x) => f(x.input_index),
            },
            DensityFunctionComponent::Interpolated(x) => f(x.input_index),
        }
    }
}

impl RangeFunction for DensityFunctionComponent {
    fn min_value(&self) -> f32 {
        match self {
            DensityFunctionComponent::Independent(func) => func.min_value(),
            DensityFunctionComponent::Dependent(func) => func.min_value(),
            DensityFunctionComponent::Interpolated(func) => func.min_value(),
        }
    }

    fn max_value(&self) -> f32 {
        match self {
            DensityFunctionComponent::Independent(func) => func.max_value(),
            DensityFunctionComponent::Dependent(func) => func.max_value(),
            DensityFunctionComponent::Interpolated(func) => func.max_value(),
        }
    }
}

#[inline]
pub fn lerp(delta: f32, start: f32, end: f32) -> f32 {
    start + delta * (end - start)
}
