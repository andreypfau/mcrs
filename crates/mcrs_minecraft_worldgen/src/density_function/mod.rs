use crate::density_function::branch_schedule::{BranchSchedule, Step};
use crate::density_function::proto::{
    ALL_AXES, AXIS_X, AXIS_Y, AXIS_Z, Axis, DensityFunctionHolder, DistanceMetric, HashableF64,
    InlineReference, NoiseHolder, NoiseParam, PowFunctionArguments, ProtoDensityFunction,
    RewriteRule, RoundFunctionArguments, RoundingMode, SingleArgumentFunction, SliceUniformAxes,
    SplineHolder, TilingMode, TwoArgumentFunction, Visitor, noise_scale_axes,
};
use crate::noise::normal_noise::NoiseSampler;
use crate::noise::octave_perlin_noise::OctavePerlinNoise;
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
use tracing::info;

pub mod beta_seed;
pub mod beta_terrain_f64;
mod branch_schedule;
mod compile;
#[cfg(test)]
mod interval_prune;
pub mod proto;

pub use compile::build_functions;

/// Maximum number of positions that can be batched in a single plane fill.
/// 5 Z-columns * 3 Y-positions = 15, rounded up to 16 for alignment.
#[cfg(feature = "batch-noise")]
pub(crate) const MAX_BATCH: usize = 128;

#[cfg(feature = "batch-noise")]
const _: () = assert!(MAX_BATCH <= u8::MAX as usize + 1);

trait DensityFunction: RangeFunction {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32;
}

fn mul_range(min1: f32, max1: f32, min2: f32, max2: f32) -> (f32, f32) {
    let min = if min1 > 0.0 && min2 > 0.0 {
        min1 * min2
    } else if max1 < 0.0 && max2 < 0.0 {
        max1 * max2
    } else {
        (min1 * max2).min(max1 * min2)
    };

    let max = if min1 > 0.0 && min2 > 0.0 {
        max1 * max2
    } else if max1 < 0.0 && max2 < 0.0 {
        min1 * min2
    } else {
        (min1 * min2).max(max1 * max2)
    };

    (min, max)
}

// A negative base with a non-constant exponent falls back to the trivial
// interval rather than analysing sign alternation. Ranges are only ever used to
// prove work redundant, so a wider interval costs speed, never parity.
#[inline]
fn pow_narrowed(base: f32, exponent: f32) -> f32 {
    (base as f64).powf(exponent as f64) as f32
}

fn pow_range(base: (f32, f32), exponent: (f32, f32)) -> (f32, f32) {
    let is_constant = base.0 == base.1 && exponent.0 == exponent.1;
    if base.0 < 0.0 && !is_constant {
        return (f32::NEG_INFINITY, f32::INFINITY);
    }
    let corners = [
        pow_narrowed(base.0, exponent.0),
        pow_narrowed(base.0, exponent.1),
        pow_narrowed(base.1, exponent.0),
        pow_narrowed(base.1, exponent.1),
    ];
    if corners.iter().any(|v| v.is_nan()) {
        return (f32::NEG_INFINITY, f32::INFINITY);
    }
    corners
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), &value| {
            (min.min(value), max.max(value))
        })
}

fn binary_range(
    operation: BinaryOperation,
    (min1, max1): (f32, f32),
    (min2, max2): (f32, f32),
) -> (f32, f32) {
    match operation {
        BinaryOperation::Add => (min1 + min2, max1 + max2),
        BinaryOperation::Multiply => mul_range(min1, max1, min2, max2),
        BinaryOperation::Subtract => (min1 - max2, max1 - min2),
        BinaryOperation::Divide => {
            let (rmin, rmax) = reciprocal_range(min2, max2);
            mul_range(min1, max1, rmin, rmax)
        }
        BinaryOperation::Min => (min1.min(min2), max1.min(max2)),
        BinaryOperation::Max => (min1.max(min2), max1.max(max2)),
        BinaryOperation::Pow => pow_range((min1, max1), (min2, max2)),
        BinaryOperation::Round(mode) => {
            let (rmin, rmax) = reciprocal_range(min2, max2);
            let (dmin, dmax) = mul_range(min1, max1, rmin, rmax);
            let lo = round_to_integer(dmin, mode);
            let hi = round_to_integer(dmax, mode);
            mul_range(lo.min(hi), lo.max(hi), min2, max2)
        }
    }
}

fn unary_range(operation: UnaryOperation, min: f32, max: f32) -> (f32, f32) {
    let min_image = operation.apply(min);
    let max_image = operation.apply(max);
    match operation {
        UnaryOperation::Reciprocal => reciprocal_range(min, max),
        UnaryOperation::Abs | UnaryOperation::Square => {
            if min >= 0.0 {
                (min_image, max_image)
            } else if max <= 0.0 {
                (max_image, min_image)
            } else {
                (0.0, min_image.max(max_image))
            }
        }
        UnaryOperation::Sqrt | UnaryOperation::Log => {
            (operation.apply(min.max(0.0)), operation.apply(max.max(0.0)))
        }
        UnaryOperation::Sign => {
            if min > 0.0 {
                (1.0, 1.0)
            } else if max < 0.0 {
                (-1.0, -1.0)
            } else {
                (
                    if min == 0.0 { 0.0 } else { -1.0 },
                    if max == 0.0 { 0.0 } else { 1.0 },
                )
            }
        }
        _ => (min_image, max_image),
    }
}

fn reciprocal_range(min: f32, max: f32) -> (f32, f32) {
    if min == 0.0 && max == 0.0 {
        (f32::NEG_INFINITY, f32::INFINITY)
    } else if min > 0.0 || max < 0.0 {
        (1.0 / max, 1.0 / min)
    } else if max == 0.0 {
        (f32::NEG_INFINITY, 1.0 / min)
    } else if min == 0.0 {
        (1.0 / max, f32::INFINITY)
    } else {
        (f32::NEG_INFINITY, f32::INFINITY)
    }
}

/// Persistent cache for density function evaluation.
/// Reuse across calls to `final_density` within the same chunk generation
/// to cache column-only values.
pub struct DensityCache {
    scratch: Vec<f32>,
    last_x: i32,
    last_z: i32,
    /// Entries `[0..column_valid_upto)` hold values for `(last_x, last_z)` at y=0.
    /// A root reaching further needs the whole prefix recomputed, never extended:
    /// the per-Y pass has since overwritten the per-block entries inside it.
    column_valid_upto: usize,
}

/// Pre-populated cache holding Zone A (column-only) results for all 289 (17x17) XZ positions
/// within a chunk column plus the +16 boundary. Eliminates the `column_changed` branch from
/// the per-block hot path when evaluating `final_density`.
///
/// The 17x17 grid covers local coordinates 0..=16 in both X and Z, which is needed because
/// plane fills sample corner positions at block_x+0, block_x+4, ..., block_x+16.
pub struct ColumnCache {
    /// `column_data[xz_idx * zone_a_count + entry_idx]`
    /// where `xz_idx = local_x * GRID_SIDE + local_z` (row-major).
    column_data: Vec<f32>,
    zone_a_count: usize,
    pub(crate) base_block_x: i32,
    pub(crate) base_block_z: i32,
    /// Spacing of the populated positions; only multiples of it hold real values.
    step: i32,
    /// Scratch buffer (len == stack.len()), reused per `final_density_from_column_cache` call.
    pub scratch: Vec<f32>,
    /// Interval scratch (len == stack.len()) for `final_density_cell_bounds`.
    bounds_scratch: Vec<(f32, f32)>,
    /// Scratch buffer for batch evaluation: MAX_BATCH * stack_len flat layout.
    /// Pre-allocated at construction to avoid repeated allocation.
    #[cfg(feature = "batch-noise")]
    batch_scratch: Vec<f32>,
    /// Temp buffer for Spline evaluation during batch path (len == stack.len()).
    #[cfg(feature = "batch-noise")]
    spline_temp: Vec<f32>,
    /// Fixed buffer for batch noise evaluation results.
    #[cfg(feature = "batch-noise")]
    batch_noise_results: [f32; MAX_BATCH],
    /// Fixed buffer for batch noise positions (scaled coordinates).
    #[cfg(feature = "batch-noise")]
    batch_noise_positions: [(f64, f64, f64); MAX_BATCH],
}

impl ColumnCache {
    const GRID_SIDE: i32 = 17;

    #[inline]
    fn offset_of(&self, local_x: i32, local_z: i32) -> Option<usize> {
        let on_lattice =
            |v: i32| v >= 0 && v < Self::GRID_SIDE && v.rem_euclid(self.step) == 0;
        (on_lattice(local_x) && on_lattice(local_z))
            .then(|| (local_x * Self::GRID_SIDE + local_z) as usize * self.zone_a_count)
    }

    /// All Zone A values at (local_x, local_z), or `None` for a position the
    /// column pass never visited.
    #[inline]
    pub fn column_values(&self, local_x: i32, local_z: i32) -> Option<&[f32]> {
        self.offset_of(local_x, local_z)
            .map(|off| &self.column_data[off..off + self.zone_a_count])
    }

    /// One column-invariant value at (local_x, local_z) without loading the whole
    /// column. Only entries below the column boundary are in the grid; screening
    /// the index with `NoiseRouter::is_column_entry` is the caller's job, because
    /// doing it here measured at 7% of the Beta chunk fill.
    #[inline]
    pub fn read_za_value(&self, local_x: i32, local_z: i32, za_index: usize) -> Option<f32> {
        debug_assert!(
            za_index < self.zone_a_count,
            "entry {za_index} is not in the column grid"
        );
        self.column_values(local_x, local_z)
            .map(|column| column[za_index])
    }

    /// Load pre-computed Zone A values for the given local (x, z) into scratch[0..zone_a_count).
    #[inline]
    pub fn load_column(&mut self, local_x: i32, local_z: i32) {
        let off = self
            .offset_of(local_x, local_z)
            .expect("column position is not on the populated lattice");
        self.scratch[..self.zone_a_count]
            .copy_from_slice(&self.column_data[off..off + self.zone_a_count]);
    }
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
    /// Horizontal cell size in blocks (typically 4).
    h_cell_blocks: usize,
    /// Vertical cell size in blocks (typically 8).
    v_cell_blocks: usize,
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

    pub fn column_boundary(&self) -> usize {
        self.column_boundary
    }

    /// Whether an entry is one the column pass computes, and so one a
    /// `ColumnCache` actually carries.
    #[inline]
    pub fn is_column_entry(&self, index: usize) -> bool {
        index < self.column_boundary
    }

    pub fn world_seed(&self) -> u64 {
        self.world_seed
    }

    pub fn final_density_idx(&self) -> usize {
        self.final_density_index
    }

    /// Evaluate a single stack entry using pre-computed cache values.
    #[inline]
    pub fn sample_entry(&self, index: usize, cache: &[f32], pos: IVec3) -> f32 {
        self.stack[index].sample_cached(cache, &self.stack, pos)
    }

    /// Create a new DensityCache for use with `final_density`.
    /// Reuse across calls within the same chunk generation.
    pub fn new_cache(&self) -> DensityCache {
        DensityCache {
            scratch: vec![0.0f32; self.stack.len()],
            last_x: i32::MIN,
            last_z: i32::MIN,
            column_valid_upto: 0,
        }
    }

    pub fn temperature_index(&self) -> usize {
        self.temperature_index
    }

    pub fn vegetation_index(&self) -> usize {
        self.vegetation_index
    }

    pub fn continents_index(&self) -> usize {
        self.continents_index
    }

    pub fn erosion_index(&self) -> usize {
        self.erosion_index
    }

    pub fn depth_index(&self) -> usize {
        self.depth_index
    }

    pub fn ridges_index(&self) -> usize {
        self.ridges_index
    }

    pub fn chunk_surface_level_index(&self) -> usize {
        self.chunk_surface_level_index
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

    pub fn sample_root(&self, root: usize, pos: IVec3, cache: &mut DensityCache) -> f32 {
        self.evaluate_forward(root, pos, cache)
    }

    /// Evaluate final_density without caching (recursive, for validation/comparison).
    pub fn final_density_uncached(&self, pos: IVec3) -> f32 {
        DensityFunctionComponent::sample_from_stack(&self.stack[..=self.final_density_index], pos)
    }

    /// Create a new `ColumnCache` for a 17x17 chunk column grid starting at block (base_block_x, base_block_z).
    /// The 17x17 grid covers local coordinates 0..=16 to include boundary corner positions.
    pub fn new_column_cache(&self, base_block_x: i32, base_block_z: i32) -> ColumnCache {
        let grid_positions = (ColumnCache::GRID_SIDE * ColumnCache::GRID_SIDE) as usize;
        ColumnCache {
            column_data: vec![0.0f32; grid_positions * self.column_boundary],
            zone_a_count: self.column_boundary,
            base_block_x,
            base_block_z,
            step: self.h_cell_blocks as i32,
            scratch: vec![0.0f32; self.stack.len()],
            bounds_scratch: vec![(0.0f32, 0.0f32); self.stack.len()],
            #[cfg(feature = "batch-noise")]
            batch_scratch: vec![0.0f32; MAX_BATCH * (self.final_density_index + 1)],
            #[cfg(feature = "batch-noise")]
            spline_temp: vec![0.0f32; self.stack.len()],
            #[cfg(feature = "batch-noise")]
            batch_noise_results: [0.0f32; MAX_BATCH],
            #[cfg(feature = "batch-noise")]
            batch_noise_positions: [(0.0f64, 0.0f64, 0.0f64); MAX_BATCH],
        }
    }

    /// Pre-populate Zone A values at cell corner positions in the chunk column grid.
    /// Only evaluates the (h_cells+1)^2 = 25 corner positions (step by h_cell_blocks),
    /// not every block position. This matches exactly the positions sampled by the plane fills.
    pub fn populate_columns(&self, cache: &mut ColumnCache) {
        let zone_a_count = cache.zone_a_count;
        let grid_side = ColumnCache::GRID_SIDE;
        let step = cache.step;
        let corners = (16 / step) + 1; // h_cells + 1
        for cx in 0..corners {
            let local_x = cx * step;
            for cz in 0..corners {
                let local_z = cz * step;
                let y0_pos = IVec3::new(
                    cache.base_block_x + local_x,
                    0,
                    cache.base_block_z + local_z,
                );
                for i in 0..zone_a_count {
                    cache.scratch[i] =
                        self.stack[i].sample_cached(&cache.scratch, &self.stack, y0_pos);
                }
                let xz_idx = (local_x * grid_side + local_z) as usize;
                let off = xz_idx * zone_a_count;
                cache.column_data[off..off + zone_a_count]
                    .copy_from_slice(&cache.scratch[..zone_a_count]);
            }
        }
    }

    /// Read post-processed (temperature, humidity) for a column position from Zone A cache.
    ///
    /// Returns (0.0, 0.0) for a position the column pass never visited. Only a
    /// router whose climate roots are column entries can answer at all — the
    /// modern router's are not, and it reaches its climate through
    /// `evaluate_forward` instead.
    pub fn sample_climate_at(&self, cache: &ColumnCache, block_x: i32, block_z: i32) -> (f32, f32) {
        let local_x = block_x - cache.base_block_x;
        let local_z = block_z - cache.base_block_z;
        let Some(column) = cache.column_values(local_x, local_z) else {
            return (0.0, 0.0);
        };
        debug_assert!(
            self.is_column_entry(self.temperature_index)
                && self.is_column_entry(self.vegetation_index),
            "this router's climate roots are not in the column grid"
        );
        (
            column[self.temperature_index],
            column[self.vegetation_index],
        )
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
        for x in 0..16i32 {
            for z in 0..16i32 {
                let pos = IVec3::new(block_x + x, 0, block_z + z);
                let temp = DensityFunctionComponent::sample_from_stack(
                    &self.stack[..=self.temperature_index],
                    pos,
                );
                let rain = DensityFunctionComponent::sample_from_stack(
                    &self.stack[..=self.vegetation_index],
                    pos,
                );
                temp_grid[(x * 16 + z) as usize] = temp;
                rain_grid[(x * 16 + z) as usize] = rain;
            }
        }
        (temp_grid, rain_grid)
    }

    /// Evaluate temperature and vegetation at `(block_x, block_z)` without a
    /// pre-populated column cache. Used by apply_beta_surface which runs after
    /// generate_column has already discarded the cache.
    pub fn sample_beta_climate(&self, block_x: i32, block_z: i32) -> (f32, f32) {
        let pos = IVec3::new(block_x, 0, block_z);
        let temperature = DensityFunctionComponent::sample_from_stack(
            &self.stack[..=self.temperature_index],
            pos,
        );
        let humidity =
            DensityFunctionComponent::sample_from_stack(&self.stack[..=self.vegetation_index], pos);
        (temperature, humidity)
    }

    /// Evaluate Zone B at `pos` into `scratch`, jumping over every arm-exclusive
    /// run whose selection cannot reach it.
    fn run_zone_b(&self, pos: IVec3, scratch: &mut [f32]) {
        let sched = &self.zone_b_schedule;
        let mut s = 0usize;
        while s < sched.steps.len() {
            match sched.steps[s] {
                Step::Eval { start, end } => {
                    for &i in &sched.order[start as usize..end as usize] {
                        let value = self.stack[i].sample_cached(scratch, &self.stack, pos);
                        scratch[i] = value;
                    }
                    s += 1;
                }
                Step::Guard {
                    input,
                    min_inclusive,
                    max_exclusive,
                    want_in,
                    unguard,
                } => {
                    let v = scratch[input as usize];
                    let in_range = v >= min_inclusive && v < max_exclusive;
                    s = if in_range == want_in {
                        s + 1
                    } else {
                        unguard as usize
                    };
                }
                Step::Unguard => s += 1,
            }
        }
        #[cfg(debug_assertions)]
        self.verify_zone_b(pos, scratch);
    }

    /// Re-evaluate the skipped runs and check nothing the caller reads moved.
    #[cfg(debug_assertions)]
    fn verify_zone_b(&self, pos: IVec3, scratch: &[f32]) {
        let mut full = scratch.to_vec();
        for i in self.column_boundary..=self.final_density_index {
            let value = self.stack[i].sample_cached(&full, &self.stack, pos);
            full[i] = value;
        }
        for &root in self.zone_b_roots.iter() {
            assert_eq!(
                full[root].to_bits(),
                scratch[root].to_bits(),
                "branch skip changed node {root} at {pos:?}"
            );
        }
    }

    /// Evaluate final_density using a pre-populated column cache.
    /// Zone A values must already be loaded into `cache.scratch` via `load_column`.
    #[inline]
    pub fn final_density_from_column_cache(&self, pos: IVec3, cache: &mut ColumnCache) -> f32 {
        self.run_zone_b(pos, &mut cache.scratch);
        cache.scratch[self.final_density_index]
    }

    /// Number of `interpolated` wrapper inputs carried per cell corner.
    #[inline]
    pub fn cell_value_count(&self) -> usize {
        self.outer_wrappers.len()
    }

    /// `final_density` at `pos` from this block's already-interpolated wrapper
    /// values. `scratch` must be at least `stack.len()` long; nothing in it is
    /// read before it is written.
    pub fn final_density_from_cell_values(
        &self,
        pos: IVec3,
        cell_values: &[f32],
        scratch: &mut [f32],
    ) -> f32 {
        for (k, &idx) in self.outer_wrappers.iter().enumerate() {
            scratch[idx] = cell_values[k];
        }
        for &i in self.outer_terms.iter() {
            let value = self.stack[i].sample_cached(scratch, &self.stack, pos);
            scratch[i] = value;
        }
        scratch[self.final_density_index]
    }

    /// Bounds on `final_density` across a whole cell, given each `interpolated`
    /// wrapper's own bounds over the cell's eight corners. Trilinear
    /// interpolation is a convex combination, so it never leaves the corner
    /// hull; interval arithmetic over the outer terms carries that up.
    ///
    /// `None` when an outer term has a kind this cannot bound, which simply
    /// means the caller must evaluate the cell block by block.
    pub fn final_density_cell_bounds(
        &self,
        wrapper_bounds: &[(f32, f32)],
        cache: &mut ColumnCache,
    ) -> Option<(f32, f32)> {
        let iv = &mut cache.bounds_scratch;
        for (k, &idx) in self.outer_wrappers.iter().enumerate() {
            iv[idx] = wrapper_bounds[k];
        }
        for &i in self.outer_terms.iter() {
            iv[i] = match &self.stack[i] {
                DensityFunctionComponent::Independent(IndependentDensityFunction::Constant(v)) => {
                    (*v, *v)
                }
                DensityFunctionComponent::Dependent(f) => match f {
                    DependentDensityFunction::Linear(x) => {
                        let (lo, hi) = iv[x.input_index];
                        match x.operation {
                            LinearOperation::Add => (lo + x.argument, hi + x.argument),
                            LinearOperation::Multiply => mul_range(lo, hi, x.argument, x.argument),
                        }
                    }
                    DependentDensityFunction::Affine(x) => {
                        let (lo, hi) = iv[x.input_index];
                        Affine::compute_range(lo, hi, x.scale, x.offset)
                    }
                    DependentDensityFunction::Unary(x) => {
                        let (lo, hi) = iv[x.input_index];
                        unary_range(x.operation, lo, hi)
                    }
                    DependentDensityFunction::Binary(x) => {
                        binary_range(x.operation, iv[x.input1_index], iv[x.input2_index])
                    }
                    DependentDensityFunction::Clamp(x) => {
                        let (lo, hi) = iv[x.input_index];
                        (
                            lo.clamp(x.min_value, x.max_value),
                            hi.clamp(x.min_value, x.max_value),
                        )
                    }
                    DependentDensityFunction::RangeChoice(x) => {
                        let (lo, hi) = iv[x.input_index];
                        if lo >= x.min_inclusion_value && hi < x.max_exclusion_value {
                            iv[x.when_in_index]
                        } else if hi < x.min_inclusion_value || lo >= x.max_exclusion_value {
                            iv[x.when_out_index]
                        } else {
                            let a = iv[x.when_in_index];
                            let b = iv[x.when_out_index];
                            (a.0.min(b.0), a.1.max(b.1))
                        }
                    }
                    _ => return None,
                },
                DensityFunctionComponent::Wrapper(WrapperDensityFunction::Cache(x)) => {
                    iv[x.input_index]
                }
                _ => return None,
            };
        }
        Some(iv[self.final_density_index])
    }

    /// The inputs of `final_density`'s `interpolated` wrappers at one lattice corner.
    /// Zone A must already be loaded into `cache.scratch` via `load_column`.
    pub fn cell_values_from_column_cache(
        &self,
        pos: IVec3,
        cache: &mut ColumnCache,
        out: &mut [f32],
    ) {
        self.run_zone_b(pos, &mut cache.scratch);
        for (k, &idx) in self.outer_wrapper_inputs.iter().enumerate() {
            out[k] = cache.scratch[idx];
        }
    }

    /// Batch-evaluate Zone B of final_density across multiple positions.
    ///
    /// **Preconditions**: `cache.batch_scratch` must be pre-populated with Zone A data for
    /// each position at `[p * stack_len .. p * stack_len + column_boundary)`.
    /// The caller is responsible for setting up Zone A (possibly from different columns).
    ///
    /// Results are written to `results[0..n]` and also left in batch_scratch at
    /// `[p * stack_len + final_density_index]`.
    #[cfg(feature = "batch-noise")]
    fn evaluate_zone_b_batch(
        &self,
        positions: &[IVec3],
        cache: &mut ColumnCache,
        results: &mut [f32],
    ) {
        let n = positions.len();
        debug_assert_eq!(n * self.cell_value_count(), results.len());
        debug_assert!(n <= MAX_BATCH);
        let stack_len = self.final_density_index + 1;
        let sched = &self.zone_b_schedule;

        let mut active = [[0u8; MAX_BATCH]; branch_schedule::MAX_GUARD_DEPTH + 1];
        let mut live = [0usize; branch_schedule::MAX_GUARD_DEPTH + 1];
        for p in 0..n {
            active[0][p] = p as u8;
        }
        live[0] = n;
        let mut depth = 0usize;

        let mut s = 0usize;
        while s < sched.steps.len() {
            match sched.steps[s] {
                Step::Eval { start, end } => {
                    let nodes = &sched.order[start as usize..end as usize];
                    if live[depth] == n {
                        for &i in nodes {
                            self.eval_node_batch(i, positions, 0..n, stack_len, cache);
                        }
                    } else {
                        let act = &active[depth][..live[depth]];
                        for &i in nodes {
                            self.eval_node_batch(
                                i,
                                positions,
                                act.iter().map(|&p| p as usize),
                                stack_len,
                                cache,
                            );
                        }
                    }
                    s += 1;
                }
                Step::Guard {
                    input,
                    min_inclusive,
                    max_exclusive,
                    want_in,
                    unguard,
                } => {
                    let mut kept = 0usize;
                    for idx in 0..live[depth] {
                        let p = active[depth][idx] as usize;
                        let v = cache.batch_scratch[p * stack_len + input as usize];
                        if ((v >= min_inclusive) & (v < max_exclusive)) == want_in {
                            active[depth + 1][kept] = p as u8;
                            kept += 1;
                        }
                    }
                    live[depth + 1] = kept;
                    depth += 1;
                    s = if kept == 0 { unguard as usize } else { s + 1 };
                }
                Step::Unguard => {
                    depth -= 1;
                    s += 1;
                }
            }
        }

        let w = self.outer_wrapper_inputs.len();
        for p in 0..n {
            let base = p * stack_len;
            for (k, &idx) in self.outer_wrapper_inputs.iter().enumerate() {
                results[p * w + k] = cache.batch_scratch[base + idx];
            }
        }

        // Zone A is untouched by the pass above, so re-running every node for
        // every position reproduces what the guards jumped over.
        #[cfg(debug_assertions)]
        {
            let mut guarded = Vec::with_capacity(n * self.zone_b_roots.len());
            for p in 0..n {
                for &idx in self.zone_b_roots.iter() {
                    guarded.push(cache.batch_scratch[p * stack_len + idx]);
                }
            }
            for i in self.column_boundary..=self.final_density_index {
                self.eval_node_batch(i, positions, 0..n, stack_len, cache);
            }
            let r = self.zone_b_roots.len();
            for p in 0..n {
                let base = p * stack_len;
                for (k, &idx) in self.zone_b_roots.iter().enumerate() {
                    assert_eq!(
                        guarded[p * r + k].to_bits(),
                        cache.batch_scratch[base + idx].to_bits(),
                        "branch skip changed node {idx} at {:?}",
                        positions[p]
                    );
                }
            }
        }
    }

    /// Evaluate one Zone B node across the batch positions `ps` selects.
    ///
    /// Generic over the position iterator so an unguarded run keeps its dense
    /// `0..n` loop: routing every node through a gathered index list costs more
    /// than the guards save on a stack that barely branches.
    #[cfg(feature = "batch-noise")]
    #[inline]
    fn eval_node_batch<I>(
        &self,
        i: usize,
        positions: &[IVec3],
        ps: I,
        stack_len: usize,
        cache: &mut ColumnCache,
    ) where
        I: ExactSizeIterator<Item = usize> + Clone,
    {
        match &self.stack[i] {
            DensityFunctionComponent::Independent(f) => match f {
                IndependentDensityFunction::OldBlendedNoise(noise) => {
                    let k = ps.len();
                    if k == positions.len() {
                        noise.sample_batch(positions, &mut cache.batch_noise_results[..k]);
                    } else {
                        let mut gathered = [IVec3::ZERO; MAX_BATCH];
                        for (j, p) in ps.clone().enumerate() {
                            gathered[j] = positions[p];
                        }
                        noise.sample_batch(&gathered[..k], &mut cache.batch_noise_results[..k]);
                    }
                    for (j, p) in ps.clone().enumerate() {
                        cache.batch_scratch[p * stack_len + i] = cache.batch_noise_results[j];
                    }
                }
                IndependentDensityFunction::Noise(noise) => {
                    let k = ps.len();
                    for (j, p) in ps.clone().enumerate() {
                        let pos = positions[p];
                        cache.batch_noise_positions[j] = (
                            pos.x as f64 * noise.xz_scale,
                            pos.y as f64 * noise.y_scale,
                            pos.z as f64 * noise.xz_scale,
                        );
                    }
                    noise.sampler.get_batch(
                        &cache.batch_noise_positions[..k],
                        &mut cache.batch_noise_results[..k],
                    );
                    for (j, p) in ps.clone().enumerate() {
                        cache.batch_scratch[p * stack_len + i] = cache.batch_noise_results[j];
                    }
                }
                _ => {
                    for p in ps.clone() {
                        cache.batch_scratch[p * stack_len + i] = f.sample(&[], positions[p]);
                    }
                }
            },
            DensityFunctionComponent::Dependent(f) => match f {
                DependentDensityFunction::Linear(x) => {
                    for p in ps.clone() {
                        let base = p * stack_len;
                        let input = cache.batch_scratch[base + x.input_index];
                        cache.batch_scratch[base + i] = match x.operation {
                            LinearOperation::Add => input + x.argument,
                            LinearOperation::Multiply => input * x.argument,
                        };
                    }
                }
                DependentDensityFunction::Affine(x) => {
                    for p in ps.clone() {
                        let base = p * stack_len;
                        let input = cache.batch_scratch[base + x.input_index];
                        cache.batch_scratch[base + i] = input.mul_add(x.scale, x.offset);
                    }
                }
                DependentDensityFunction::PiecewiseAffine(x) => {
                    for p in ps.clone() {
                        let base = p * stack_len;
                        let input = cache.batch_scratch[base + x.input_index];
                        let scale = if input < 0.0 {
                            x.neg_scale
                        } else {
                            x.pos_scale
                        };
                        cache.batch_scratch[base + i] = input.mul_add(scale, x.offset);
                    }
                }
                DependentDensityFunction::Slide(x) => {
                    for p in ps.clone() {
                        let base = p * stack_len;
                        let input = cache.batch_scratch[base + x.input_index];
                        cache.batch_scratch[base + i] = x.compute(input, positions[p].y as f32);
                    }
                }
                DependentDensityFunction::Unary(x) => {
                    for p in ps.clone() {
                        let base = p * stack_len;
                        let input = cache.batch_scratch[base + x.input_index];
                        cache.batch_scratch[base + i] = x.operation.apply(input);
                    }
                }
                DependentDensityFunction::Binary(x) => {
                    for p in ps.clone() {
                        let base = p * stack_len;
                        let a = cache.batch_scratch[base + x.input1_index];
                        let b = cache.batch_scratch[base + x.input2_index];
                        cache.batch_scratch[base + i] = x.operation.apply(a, b);
                    }
                }
                DependentDensityFunction::ShiftedNoise(x) => {
                    for p in ps.clone() {
                        let base = p * stack_len;
                        cache.batch_scratch[base + i] = x.sampler.get(
                            positions[p].x as f64 * x.xz_scale
                                + cache.batch_scratch[base + x.input_x_index] as f64,
                            positions[p].y as f64 * x.y_scale
                                + cache.batch_scratch[base + x.input_y_index] as f64,
                            positions[p].z as f64 * x.xz_scale
                                + cache.batch_scratch[base + x.input_z_index] as f64,
                        );
                    }
                }
                DependentDensityFunction::Clamp(x) => {
                    for p in ps.clone() {
                        let base = p * stack_len;
                        let input = cache.batch_scratch[base + x.input_index];
                        cache.batch_scratch[base + i] = input.clamp(x.min_value, x.max_value);
                    }
                }
                DependentDensityFunction::RangeChoice(x) => {
                    for p in ps.clone() {
                        let base = p * stack_len;
                        let input = cache.batch_scratch[base + x.input_index];
                        cache.batch_scratch[base + i] =
                            if input >= x.min_inclusion_value && input < x.max_exclusion_value {
                                cache.batch_scratch[base + x.when_in_index]
                            } else {
                                cache.batch_scratch[base + x.when_out_index]
                            };
                    }
                }
                DependentDensityFunction::Spline(x) => {
                    for p in ps.clone() {
                        let base = p * stack_len;
                        // Copy this position's scratch into temp for Spline's &[f32] API
                        cache.spline_temp[..stack_len]
                            .copy_from_slice(&cache.batch_scratch[base..base + stack_len]);
                        cache.batch_scratch[base + i] =
                            x.sample_cached(&cache.spline_temp, &self.stack, positions[p]);
                    }
                }
                DependentDensityFunction::Lerp(x) => {
                    for p in ps.clone() {
                        let base = p * stack_len;
                        let alpha = cache.batch_scratch[base + x.alpha_index];
                        let first = cache.batch_scratch[base + x.first_index];
                        let second = cache.batch_scratch[base + x.second_index];
                        cache.batch_scratch[base + i] = if alpha == 0.0 {
                            first
                        } else if alpha == 1.0 {
                            second
                        } else {
                            first + alpha * (second - first)
                        };
                    }
                }
                DependentDensityFunction::Slice(x) => {
                    for p in ps.clone() {
                        cache.batch_scratch[p * stack_len + i] =
                            x.sample(&self.stack[..=i], positions[p]);
                    }
                }
                DependentDensityFunction::FindTopSurface(x) => {
                    for p in ps.clone() {
                        let base = p * stack_len;
                        let top_y = (cache.batch_scratch[base + x.upper_bound_index]
                            / x.cell_height)
                            .floor()
                            * x.cell_height;
                        cache.batch_scratch[base + i] = if top_y <= x.lower_bound {
                            x.lower_bound
                        } else {
                            let mut current_y = top_y;
                            loop {
                                let sample_pos =
                                    IVec3::new(positions[p].x, current_y as i32, positions[p].z);
                                let density = DensityFunctionComponent::sample_from_stack(
                                    &self.stack[..=x.density_index],
                                    sample_pos,
                                );
                                if density > 0.0 || current_y <= x.lower_bound {
                                    break current_y;
                                }
                                current_y -= x.cell_height;
                            }
                        };
                    }
                }
            },
            DensityFunctionComponent::Wrapper(f) => match f {
                WrapperDensityFunction::Interpolated(x) => {
                    for p in ps.clone() {
                        let base = p * stack_len;
                        cache.batch_scratch[base + i] = if x.is_cell_corner(positions[p]) {
                            cache.batch_scratch[base + x.input_index]
                        } else {
                            x.sample(&self.stack, positions[p])
                        };
                    }
                }
                WrapperDensityFunction::Cache(x) => {
                    for p in ps.clone() {
                        let base = p * stack_len;
                        cache.batch_scratch[base + i] = cache.batch_scratch[base + x.input_index];
                    }
                }
            },
        }
    }

    /// Batch-evaluate final_density across multiple Z columns in one call.
    ///
    /// `positions` is laid out as `[col0_y0, col0_y1, ..., col1_y0, col1_y1, ...]`
    /// with `per_col` positions per column. `column_local_xz[c]` gives the
    /// `(local_x, local_z)` used to load Zone A for column `c`.
    ///
    /// This batches all positions across all columns through the Zone B evaluation,
    /// keeping noise permutation tables cache-hot and enabling SIMD across positions.
    #[cfg(feature = "batch-noise")]
    pub fn evaluate_plane_batch(
        &self,
        positions: &[IVec3],
        column_local_xz: &[(i32, i32)],
        per_col: usize,
        cache: &mut ColumnCache,
        results: &mut [f32],
    ) {
        let n = positions.len();
        debug_assert_eq!(n * self.cell_value_count(), results.len());
        debug_assert_eq!(n, column_local_xz.len() * per_col);
        debug_assert!(n <= MAX_BATCH);

        let stack_len = self.final_density_index + 1;
        let za = self.column_boundary;
        debug_assert!(cache.batch_scratch.len() >= n * stack_len);

        // Load Zone A for each column and copy to each position in that column
        for (c, &(local_x, local_z)) in column_local_xz.iter().enumerate() {
            cache.load_column(local_x, local_z);
            for j in 0..per_col {
                let p = c * per_col + j;
                let off = p * stack_len;
                cache.batch_scratch[off..off + za].copy_from_slice(&cache.scratch[..za]);
            }
        }

        self.evaluate_zone_b_batch(positions, cache, results);
    }

    /// Forward evaluation with column caching and zone-based dispatch.
    ///
    /// The stack is reordered into three zones:
    ///   Zone A `[0..column_boundary)`:  column-only entries for final_density
    ///   Zone B `[column_boundary..fd_boundary)`: per-Y entries for final_density
    ///   Zone C `[fd_boundary..n)`:               other roots (aquifer, veins, etc.)
    ///
    /// For Zone A roots (continents, erosion, ridges, etc.):
    ///   Only the column pass runs; the per-Y loop is empty.
    ///
    /// For Zone B roots (final_density and its per-Y dependencies):
    ///   Column pass evaluates Zone A at Y=0; per-Y pass sweeps Zone B branchlessly.
    ///
    /// For Zone C roots (temperature, chunk_surface_level, veins, etc.):
    ///   Falls back to the general per_block-checking approach.
    fn evaluate_forward(&self, root: usize, pos: IVec3, cache: &mut DensityCache) -> f32 {
        // Zone A and Zone B roots read column values only below the boundary;
        // a Zone C root also reads the column-only entries interleaved with its
        // own per-Y ones, so its prefix runs all the way to the root.
        let column_needed = if root < self.fd_boundary {
            self.column_boundary
        } else {
            root + 1
        };

        if pos.x != cache.last_x
            || pos.z != cache.last_z
            || cache.column_valid_upto < column_needed
        {
            cache.last_x = pos.x;
            cache.last_z = pos.z;
            cache.column_valid_upto = column_needed;
            let y0_pos = IVec3::new(pos.x, 0, pos.z);
            for i in 0..column_needed {
                cache.scratch[i] = self.stack[i].sample_cached(&cache.scratch, &self.stack, y0_pos);
            }
        }

        if root >= self.fd_boundary {
            // Zone C root: fallback for aquifer, veins, temperature, etc.
            for i in 0..=root {
                if self.per_block[i] {
                    cache.scratch[i] =
                        self.stack[i].sample_cached(&cache.scratch, &self.stack, pos);
                }
            }
        } else if root >= self.column_boundary {
            // Zone B root: every entry in this range is per_block by construction.
            for i in self.column_boundary..=root {
                cache.scratch[i] = self.stack[i].sample_cached(&cache.scratch, &self.stack, pos);
            }
        }

        cache.scratch[root]
    }

    /// Create a new `NoiseCellInterpolator` matching this router's cell dimensions.
    pub fn new_noise_cell_interpolator(&self) -> NoiseCellInterpolator {
        NoiseCellInterpolator::new(
            self.h_cell_blocks,
            self.v_cell_blocks,
            self.cell_value_count(),
        )
    }
}

/// Trilinear interpolator for the chunk fill.
///
/// Carries the input of every `interpolated` wrapper `final_density` reads —
/// `values_per_corner()` floats per lattice corner. Those are the only values
/// vanilla interpolates; everything above the wrappers is applied per block by
/// `NoiseRouter::final_density_from_cell_values`.
pub struct NoiseCellInterpolator {
    h_cell_blocks: usize,
    v_cell_blocks: usize,
    h_cells: usize,
    v_cells: usize,
    values: usize,

    /// Y-Z plane of corner values at the current X plane start.
    /// Indexed: `buf[((z_corner * (v_cells + 1)) + y_corner) * values + k]`
    start_buf: Vec<f32>,
    /// Y-Z plane of corner values at the current X plane end.
    end_buf: Vec<f32>,

    /// 8 cell corners after `on_sampled_cell_corners`, corner-major.
    corners: Vec<f32>,
    /// 4 corner-pairs after Y interpolation
    after_y: Vec<f32>,
    /// 2 values after X interpolation
    after_x: Vec<f32>,
    /// Final interpolated cell values
    val: Vec<f32>,
    /// Per-value (min, max) over the 8 corners, refreshed by `on_sampled_cell_corners`.
    bounds: Vec<(f32, f32)>,

    /// Saved top-Y corner values for section-boundary reuse.
    /// Adjacent Y sections share cell corners at their boundary (the top Y-row
    /// of section s equals the bottom Y-row of section s+1). This buffer saves
    /// those values to avoid recomputing them.
    ///
    /// Indexed by `[(plane_seq * z_count + z_corner) * values + k]` where
    /// plane_seq is the sequential plane fill index within a section
    /// (0..=h_cells) and z_count = h_cells + 1.
    saved_top_y: Vec<f32>,
    /// True after the first section has been fully processed, meaning
    /// `saved_top_y` contains valid data for the next section.
    section_boundary_valid: bool,

    /// Precomputed corner values for the whole column:
    /// `grid[((plane_x * (h_cells + 1) + z_corner) * grid_rows + y_row) * values + k]`.
    /// Filled by `precompute_column_grid`; plane fills copy from it instead
    /// of evaluating the density stack per section.
    #[cfg(feature = "batch-noise")]
    grid: Vec<f32>,
    #[cfg(feature = "batch-noise")]
    grid_base_y: i32,
    #[cfg(feature = "batch-noise")]
    grid_rows: usize,
    #[cfg(feature = "batch-noise")]
    grid_valid: bool,
    /// Reusable batch output, `MAX_BATCH * values` long.
    #[cfg(feature = "batch-noise")]
    batch_results: Vec<f32>,
}

impl NoiseCellInterpolator {
    pub fn new(h_cell_blocks: usize, v_cell_blocks: usize, values: usize) -> Self {
        let h_cells = 16 / h_cell_blocks;
        let v_cells = 16 / v_cell_blocks;
        let plane_size = (h_cells + 1) * (v_cells + 1) * values;
        let z_count = h_cells + 1;
        let num_planes = h_cells + 1; // start plane + h_cells end planes
        Self {
            h_cell_blocks,
            v_cell_blocks,
            h_cells,
            v_cells,
            values,
            start_buf: vec![0.0f32; plane_size],
            end_buf: vec![0.0f32; plane_size],
            corners: vec![0.0f32; 8 * values],
            after_y: vec![0.0f32; 4 * values],
            after_x: vec![0.0f32; 2 * values],
            val: vec![0.0f32; values],
            bounds: vec![(0.0f32, 0.0f32); values],
            saved_top_y: vec![0.0f32; num_planes * z_count * values],
            section_boundary_valid: false,
            #[cfg(feature = "batch-noise")]
            grid: Vec::new(),
            #[cfg(feature = "batch-noise")]
            grid_base_y: 0,
            #[cfg(feature = "batch-noise")]
            grid_rows: 0,
            #[cfg(feature = "batch-noise")]
            grid_valid: false,
            #[cfg(feature = "batch-noise")]
            batch_results: vec![0.0f32; MAX_BATCH * values],
        }
    }

    /// Evaluate the cell-lattice values for every corner in the column in large
    /// multi-column batches and store them in the interpolator grid.
    ///
    /// Corner rows span `grid_base_y + r * v_cell_blocks` for `r in 0..rows`.
    /// Subsequent `fill_plane_cached_reuse` calls whose corners fall inside the
    /// grid copy values instead of evaluating the density stack, amortizing the
    /// per-batch octave-loop overhead over up to `MAX_BATCH` positions.
    #[cfg(feature = "batch-noise")]
    pub fn precompute_column_grid(
        &mut self,
        router: &NoiseRouter,
        cache: &mut ColumnCache,
        grid_base_y: i32,
        rows: usize,
    ) {
        let side = self.h_cells + 1;
        let w = self.values;
        if rows == 0 || rows > MAX_BATCH {
            self.grid_valid = false;
            return;
        }
        self.grid.resize(side * side * rows * w, 0.0);
        self.grid_base_y = grid_base_y;
        self.grid_rows = rows;

        let cols_per_batch = MAX_BATCH / rows;
        let mut positions = [IVec3::ZERO; MAX_BATCH];
        let mut col_xz = [(0i32, 0i32); MAX_BATCH];
        let mut col_grid_idx = [0usize; MAX_BATCH];
        let mut batch_cols = 0usize;
        let mut idx = 0usize;

        for px in 0..side {
            let local_x = (px * self.h_cell_blocks) as i32;
            let x = cache.base_block_x + local_x;
            for cz in 0..side {
                let local_z = (cz * self.h_cell_blocks) as i32;
                let z = cache.base_block_z + local_z;
                col_xz[batch_cols] = (local_x, local_z);
                col_grid_idx[batch_cols] = px * side + cz;
                for r in 0..rows {
                    positions[idx] =
                        IVec3::new(x, grid_base_y + (r * self.v_cell_blocks) as i32, z);
                    idx += 1;
                }
                batch_cols += 1;

                if batch_cols == cols_per_batch {
                    router.evaluate_plane_batch(
                        &positions[..idx],
                        &col_xz[..batch_cols],
                        rows,
                        cache,
                        &mut self.batch_results[..idx * w],
                    );
                    for c in 0..batch_cols {
                        let g0 = col_grid_idx[c] * rows * w;
                        self.grid[g0..g0 + rows * w]
                            .copy_from_slice(&self.batch_results[c * rows * w..(c + 1) * rows * w]);
                    }
                    batch_cols = 0;
                    idx = 0;
                }
            }
        }
        if batch_cols > 0 {
            router.evaluate_plane_batch(
                &positions[..idx],
                &col_xz[..batch_cols],
                rows,
                cache,
                &mut self.batch_results[..idx * w],
            );
            for c in 0..batch_cols {
                let g0 = col_grid_idx[c] * rows * w;
                self.grid[g0..g0 + rows * w]
                    .copy_from_slice(&self.batch_results[c * rows * w..(c + 1) * rows * w]);
            }
        }
        self.grid_valid = true;
    }

    #[inline]
    pub fn h_cells(&self) -> usize {
        self.h_cells
    }

    #[inline]
    pub fn v_cells(&self) -> usize {
        self.v_cells
    }

    #[inline]
    pub fn h_cell_blocks(&self) -> usize {
        self.h_cell_blocks
    }

    #[inline]
    pub fn v_cell_blocks(&self) -> usize {
        self.v_cell_blocks
    }

    #[inline]
    pub fn values_per_corner(&self) -> usize {
        self.values
    }

    /// No-op without the `batch-noise` feature; plane fills evaluate lazily.
    #[cfg(not(feature = "batch-noise"))]
    pub fn precompute_column_grid(
        &mut self,
        _router: &NoiseRouter,
        _cache: &mut ColumnCache,
        _grid_base_y: i32,
        _rows: usize,
    ) {
    }

    /// Evaluate the cell-lattice values at every corner on a Y-Z plane for a
    /// given X, storing results into `start_buf` or `end_buf`. Zone A comes from
    /// the pre-populated `ColumnCache`, so only Zone B is evaluated per corner.
    ///
    /// The top-Y row from the previous section is reused as this section's
    /// bottom-Y row when `section_boundary_valid` is true.
    ///
    /// `plane_seq` identifies which X-plane is being filled (0 = start plane,
    /// 1..=h_cells = successive end planes). This index is used to look up the
    /// correct saved top-Y row from the previous section.
    ///
    /// After filling, the top-Y row (cy = v_cells) is saved for the next section.
    pub fn fill_plane_cached_reuse(
        &mut self,
        plane_seq: usize,
        is_start: bool,
        x: i32,
        base_y: i32,
        base_z: i32,
        router: &NoiseRouter,
        column_cache: &mut ColumnCache,
    ) {
        let w = self.values;
        let v_stride = (self.v_cells + 1) * w;
        let local_x = x - column_cache.base_block_x;
        let z_count = self.h_cells + 1;
        let reuse = self.section_boundary_valid;

        // Fast path: copy the plane from the precomputed column grid.
        #[cfg(feature = "batch-noise")]
        if self.grid_valid {
            let dy = base_y - self.grid_base_y;
            let v_step = self.v_cell_blocks as i32;
            let row0 = dy / v_step;
            if dy >= 0
                && dy % v_step == 0
                && (row0 as usize) + self.v_cells < self.grid_rows
                && local_x >= 0
                && local_x % self.h_cell_blocks as i32 == 0
            {
                let px = (local_x as usize) / self.h_cell_blocks;
                if px < z_count {
                    let row0 = row0 as usize;
                    let buf = if is_start {
                        &mut self.start_buf
                    } else {
                        &mut self.end_buf
                    };
                    for cz in 0..z_count {
                        let g0 = ((px * z_count + cz) * self.grid_rows + row0) * w;
                        buf[cz * v_stride..cz * v_stride + v_stride]
                            .copy_from_slice(&self.grid[g0..g0 + v_stride]);
                        let top = cz * v_stride + self.v_cells * w;
                        self.saved_top_y[(plane_seq * z_count + cz) * w..][..w]
                            .copy_from_slice(&buf[top..top + w]);
                    }
                    return;
                }
            }
        }

        #[cfg(feature = "batch-noise")]
        {
            use crate::density_function::MAX_BATCH;

            let cy_start = if reuse { 1usize } else { 0 };
            let per_col = self.v_cells + 1 - cy_start;
            let total = z_count * per_col;
            debug_assert!(total <= MAX_BATCH);

            let mut positions = [IVec3::ZERO; MAX_BATCH];
            let mut column_xz = [(0i32, 0i32); MAX_BATCH];
            let mut idx = 0;

            {
                let buf = if is_start {
                    &mut self.start_buf
                } else {
                    &mut self.end_buf
                };
                for cz in 0..z_count {
                    if reuse {
                        buf[cz * v_stride..cz * v_stride + w].copy_from_slice(
                            &self.saved_top_y[(plane_seq * z_count + cz) * w..][..w],
                        );
                    }
                    let z = base_z + (cz * self.h_cell_blocks) as i32;
                    let local_z = z - column_cache.base_block_z;
                    column_xz[cz] = (local_x, local_z);

                    for cy in cy_start..=self.v_cells {
                        let y = base_y + (cy * self.v_cell_blocks) as i32;
                        positions[idx] = IVec3::new(x, y, z);
                        idx += 1;
                    }
                }
            }
            debug_assert_eq!(idx, total);

            router.evaluate_plane_batch(
                &positions[..total],
                &column_xz[..z_count],
                per_col,
                column_cache,
                &mut self.batch_results[..total * w],
            );

            let buf = if is_start {
                &mut self.start_buf
            } else {
                &mut self.end_buf
            };
            let mut idx = 0;
            for cz in 0..z_count {
                for cy in cy_start..=self.v_cells {
                    let dst = cz * v_stride + cy * w;
                    buf[dst..dst + w].copy_from_slice(&self.batch_results[idx * w..(idx + 1) * w]);
                    idx += 1;
                }
                let top = cz * v_stride + self.v_cells * w;
                self.saved_top_y[(plane_seq * z_count + cz) * w..][..w]
                    .copy_from_slice(&buf[top..top + w]);
            }
        }

        #[cfg(not(feature = "batch-noise"))]
        {
            let mut corner = vec![0.0f32; w];
            for cz in 0..z_count {
                if reuse {
                    let src = (plane_seq * z_count + cz) * w;
                    let dst = cz * v_stride;
                    let saved: Vec<f32> = self.saved_top_y[src..src + w].to_vec();
                    let buf = if is_start {
                        &mut self.start_buf
                    } else {
                        &mut self.end_buf
                    };
                    buf[dst..dst + w].copy_from_slice(&saved);
                }

                let z = base_z + (cz * self.h_cell_blocks) as i32;
                let local_z = z - column_cache.base_block_z;

                let cy_start = if reuse { 1 } else { 0 };
                for cy in cy_start..=self.v_cells {
                    column_cache.load_column(local_x, local_z);
                    let y = base_y + (cy * self.v_cell_blocks) as i32;
                    let pos = IVec3::new(x, y, z);
                    router.cell_values_from_column_cache(pos, column_cache, &mut corner);
                    let dst = cz * v_stride + cy * w;
                    let buf = if is_start {
                        &mut self.start_buf
                    } else {
                        &mut self.end_buf
                    };
                    buf[dst..dst + w].copy_from_slice(&corner);
                }

                let top = cz * v_stride + self.v_cells * w;
                let buf = if is_start {
                    &self.start_buf
                } else {
                    &self.end_buf
                };
                let saved: Vec<f32> = buf[top..top + w].to_vec();
                self.saved_top_y[(plane_seq * z_count + cz) * w..][..w].copy_from_slice(&saved);
            }
        }
    }

    /// Mark the current section as complete, enabling Y-boundary reuse for the
    /// next section. Call this after all plane fills and interpolation for
    /// a section are done.
    #[inline]
    pub fn end_section(&mut self) {
        self.section_boundary_valid = true;
    }

    /// Invalidate the Y-boundary cache, forcing the next section to compute
    /// all corner values from scratch. Must be called when the next section
    /// is not adjacent to the current one (i.e. there is a gap in Y sections).
    #[inline]
    pub fn reset_section_boundary(&mut self) {
        self.section_boundary_valid = false;
    }

    /// Load the 8 cell corners into `corners`, corner-major:
    ///   corner 0 = (x0, y0, z0)   corner 1 = (x0, y1, z0)
    ///   corner 2 = (x0, y0, z1)   corner 3 = (x0, y1, z1)
    ///   corner 4 = (x1, y0, z0)   corner 5 = (x1, y1, z0)
    ///   corner 6 = (x1, y0, z1)   corner 7 = (x1, y1, z1)
    #[inline]
    pub fn on_sampled_cell_corners(&mut self, cell_y: usize, cell_z: usize) {
        let w = self.values;
        let v_stride = (self.v_cells + 1) * w;
        let z0 = cell_z * v_stride + cell_y * w;
        let z1 = (cell_z + 1) * v_stride + cell_y * w;
        for (slot, (buf, off)) in [
            (0usize, (false, z0)),
            (1, (false, z0 + w)),
            (2, (false, z1)),
            (3, (false, z1 + w)),
            (4, (true, z0)),
            (5, (true, z0 + w)),
            (6, (true, z1)),
            (7, (true, z1 + w)),
        ] {
            let src = if buf { &self.end_buf } else { &self.start_buf };
            self.corners[slot * w..slot * w + w].copy_from_slice(&src[off..off + w]);
        }
        for k in 0..w {
            let mut lo = self.corners[k];
            let mut hi = lo;
            for slot in 1..8 {
                let v = self.corners[slot * w + k];
                lo = lo.min(v);
                hi = hi.max(v);
            }
            self.bounds[k] = (lo, hi);
        }
    }

    /// Per-value (min, max) over the current cell's eight corners.
    #[inline]
    pub fn corner_bounds(&self) -> &[(f32, f32)] {
        &self.bounds
    }

    /// The 8 cell corners, corner-major, `values_per_corner()` floats each.
    #[inline]
    pub fn corners(&self) -> &[f32] {
        &self.corners
    }

    /// Interpolate along Y: 8 corners → 4 values.
    /// `delta` = local_y / v_cell_blocks (0.0 at bottom of cell, 1.0 at top).
    #[inline]
    pub fn interpolate_y(&mut self, delta: f32) {
        let w = self.values;
        for pair in 0..4 {
            let lo = pair * 2 * w;
            let hi = lo + w;
            for k in 0..w {
                self.after_y[pair * w + k] = self.corners[lo + k].lerp(self.corners[hi + k], delta);
            }
        }
    }

    /// Interpolate along X: 4 values → 2 values.
    /// `delta` = local_x / h_cell_blocks.
    #[inline]
    pub fn interpolate_x(&mut self, delta: f32) {
        let w = self.values;
        for k in 0..w {
            self.after_x[k] = self.after_y[k].lerp(self.after_y[2 * w + k], delta);
            self.after_x[w + k] = self.after_y[w + k].lerp(self.after_y[3 * w + k], delta);
        }
    }

    /// Interpolate along Z: 2 values → 1 value.
    /// `delta` = local_z / h_cell_blocks.
    #[inline]
    pub fn interpolate_z(&mut self, delta: f32) {
        let w = self.values;
        for k in 0..w {
            self.val[k] = self.after_x[k].lerp(self.after_x[w + k], delta);
        }
    }

    /// Swap start and end buffers (the current end becomes the next start).
    #[inline]
    pub fn swap_buffers(&mut self) {
        swap(&mut self.start_buf, &mut self.end_buf);
    }

    /// The interpolated cell values for the current block, ready to feed
    /// `NoiseRouter::final_density_from_cell_values`.
    #[inline]
    pub fn result(&self) -> &[f32] {
        &self.val
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
    /// Trailing divisor applied after the combine. Modern path passes 128.0; Beta path
    /// passes 1.0 (no division) — verified against ChunkProviderGenerate.java:280-297
    /// which has NO /128 vs BlendedNoise.java:159 which does.
    final_divisor: f32,
    lower_interpolated_noise: OctavePerlinNoise<f32>,
    upper_interpolated_noise: OctavePerlinNoise<f32>,
    interpolated_noise: OctavePerlinNoise<f32>,
}

type OldBlendedNoise = BlendedNoise;

impl Debug for BlendedNoise {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlendedNoise")
            .field("xz_scale", &self.xz_scale)
            .field("y_scale", &self.y_scale)
            .field("xz_factor", &self.xz_scale)
            .field("y_factor", &self.y_scale)
            .field("smear_scale_multiplier", &self.smear_scale_multiplier)
            .field("xz_multiplier", &self.xz_multiplier)
            .field("y_multiplier", &self.y_multiplier)
            .field("max_value", &self.max_value)
            .field("final_divisor", &self.final_divisor)
            .finish()
    }
}

const LIMIT_OCTAVES: u32 = 16;

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

    /// Batch-evaluate OldBlendedNoise at multiple positions simultaneously (zero heap allocation).
    /// Evaluates all octaves for all positions together, keeping permutation
    /// tables cache-warm. Evaluates both lower and upper noise unconditionally
    /// (the branch savings from skipping are offset by batch SIMD gains).
    #[cfg(feature = "batch-noise")]
    pub fn sample_batch(&self, positions: &[IVec3], results: &mut [f32]) {
        let n = positions.len();
        debug_assert_eq!(n, results.len());
        debug_assert!(n <= MAX_BATCH);

        // Pre-compute scaled coordinates on stack
        let mut scaled = [(0.0f64, 0.0f64, 0.0f64); MAX_BATCH];
        for j in 0..n {
            scaled[j] = (
                positions[j].x as f64 * self.xz_multiplier,
                positions[j].y as f64 * self.y_multiplier,
                positions[j].z as f64 * self.xz_multiplier,
            );
        }

        // Reusable stack buffers
        let mut octave_results = [0.0f32; MAX_BATCH];
        let mut positions_buf = [(0.0f64, 0.0f64, 0.0f64); MAX_BATCH];
        let mut y_maxes = [0.0f64; MAX_BATCH];

        // ---- Interpolated noise: 8 octaves ----
        let mut interp_fxs = [0.0f64; MAX_BATCH];
        let mut interp_fys = [0.0f64; MAX_BATCH];
        let mut interp_fzs = [0.0f64; MAX_BATCH];
        for j in 0..n {
            interp_fxs[j] = scaled[j].0 / self.xz_factor;
            interp_fys[j] = scaled[j].1 / self.y_factor;
            interp_fzs[j] = scaled[j].2 / self.xz_factor;
        }
        let mut interp_values = [0.0f32; MAX_BATCH];
        let mut main_smear = self.main_smear;
        let mut amplitude = 1.0f32;

        for i in 0..8 {
            for j in 0..n {
                positions_buf[j] = (
                    OctavePerlinNoise::maintain_precission(interp_fxs[j]),
                    OctavePerlinNoise::maintain_precission(interp_fys[j]),
                    OctavePerlinNoise::maintain_precission(interp_fzs[j]),
                );
                y_maxes[j] = interp_fys[j];
            }

            self.interpolated_noise.sample_octave_batch(
                i,
                &positions_buf[..n],
                main_smear,
                &y_maxes[..n],
                &mut octave_results[..n],
            );

            for j in 0..n {
                interp_values[j] += octave_results[j] * amplitude;
                interp_fxs[j] *= 0.5;
                interp_fys[j] *= 0.5;
                interp_fzs[j] *= 0.5;
            }
            main_smear *= 0.5;
            amplitude *= 2.0;
        }

        // Mirror the scalar path's laziness: only evaluate the lower (resp. upper)
        // 16-octave noise for positions whose main-noise value actually selects it.
        // Each position's math is unchanged, so results stay bit-identical.
        let mut blend_values = [0.0f32; MAX_BATCH];
        let mut lower_idx = [0usize; MAX_BATCH];
        let mut upper_idx = [0usize; MAX_BATCH];
        let mut n_lower = 0;
        let mut n_upper = 0;
        for j in 0..n {
            let value = (interp_values[j] / 10.0 + 1.0) / 2.0;
            blend_values[j] = value;
            if value < 1.0 {
                lower_idx[n_lower] = j;
                n_lower += 1;
            }
            if value > 0.0 {
                upper_idx[n_upper] = j;
                n_upper += 1;
            }
        }

        let mut sxs = [0.0f64; MAX_BATCH];
        let mut sys = [0.0f64; MAX_BATCH];
        let mut szs = [0.0f64; MAX_BATCH];

        // ---- Lower noise: 16 octaves (only positions with value < 1.0) ----
        for (k, &j) in lower_idx[..n_lower].iter().enumerate() {
            sxs[k] = scaled[j].0;
            sys[k] = scaled[j].1;
            szs[k] = scaled[j].2;
        }
        let mut lower_values = [0.0f32; MAX_BATCH];
        let mut sm: f64 = self.limit_smear;
        amplitude = 1.0;

        for i in 0..16 {
            for k in 0..n_lower {
                positions_buf[k] = (
                    OctavePerlinNoise::maintain_precission(sxs[k]),
                    OctavePerlinNoise::maintain_precission(sys[k]),
                    OctavePerlinNoise::maintain_precission(szs[k]),
                );
                y_maxes[k] = sys[k];
            }

            self.lower_interpolated_noise.sample_octave_batch(
                i,
                &positions_buf[..n_lower],
                sm,
                &y_maxes[..n_lower],
                &mut octave_results[..n_lower],
            );

            for k in 0..n_lower {
                lower_values[k] += octave_results[k] * amplitude;
                sxs[k] *= 0.5;
                sys[k] *= 0.5;
                szs[k] *= 0.5;
            }
            sm *= 0.5;
            amplitude *= 2.0;
        }

        // ---- Upper noise: 16 octaves (only positions with value > 0.0) ----
        for (k, &j) in upper_idx[..n_upper].iter().enumerate() {
            sxs[k] = scaled[j].0;
            sys[k] = scaled[j].1;
            szs[k] = scaled[j].2;
        }
        let mut upper_values = [0.0f32; MAX_BATCH];
        sm = self.limit_smear;
        amplitude = 1.0;

        for i in 0..16 {
            for k in 0..n_upper {
                positions_buf[k] = (
                    OctavePerlinNoise::maintain_precission(sxs[k]),
                    OctavePerlinNoise::maintain_precission(sys[k]),
                    OctavePerlinNoise::maintain_precission(szs[k]),
                );
                y_maxes[k] = sys[k];
            }

            self.upper_interpolated_noise.sample_octave_batch(
                i,
                &positions_buf[..n_upper],
                sm,
                &y_maxes[..n_upper],
                &mut octave_results[..n_upper],
            );

            for k in 0..n_upper {
                upper_values[k] += octave_results[k] * amplitude;
                sxs[k] *= 0.5;
                sys[k] *= 0.5;
                szs[k] *= 0.5;
            }
            sm *= 0.5;
            amplitude *= 2.0;
        }

        // ---- Scatter lower/upper back to per-position slots ----
        let mut starts = [0.0f32; MAX_BATCH];
        let mut ends = [0.0f32; MAX_BATCH];
        for (k, &j) in lower_idx[..n_lower].iter().enumerate() {
            starts[j] = lower_values[k] / 512.0;
        }
        for (k, &j) in upper_idx[..n_upper].iter().enumerate() {
            ends[j] = upper_values[k] / 512.0;
        }

        // ---- Combine results ----
        for j in 0..n {
            let value = blend_values[j];
            results[j] = if value < 0.0 {
                starts[j]
            } else if value > 1.0 {
                ends[j]
            } else {
                value.mul_add(ends[j] - starts[j], starts[j])
            } / self.final_divisor;
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
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
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
        let mut amplitude = 1.0f32;
        for i in 0..8 {
            let s = self.interpolated_noise.sample_octave(
                i,
                OctavePerlinNoise::maintain_precission(fx),
                OctavePerlinNoise::maintain_precission(fy),
                OctavePerlinNoise::maintain_precission(fz),
                main_smear,
                fy,
            );
            value += s * amplitude;
            fx *= 0.5;
            fy *= 0.5;
            fz *= 0.5;
            main_smear *= 0.5;
            amplitude *= 2.0;
        }

        value = (value / 10.0 + 1.0) / 2.0;
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
            value.mul_add(end - start, start)
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
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
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
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
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
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
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
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        self.sampler.get(
            pos.z as f64 * 0.25,
            pos.x as f64 * 0.25,
            pos.z as f64 * 0.25,
        ) * 4.0
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Interpolated {
    input_index: usize,
    cell_size_xz: u32,
    cell_size_y: u32,
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

impl Interpolated {
    #[inline]
    fn sample_input(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        DensityFunctionComponent::sample_from_stack(&stack[..=self.input_index], pos)
    }

    #[inline]
    fn is_cell_corner(&self, pos: IVec3) -> bool {
        pos.x.rem_euclid(self.cell_size_xz as i32) == 0
            && pos.y.rem_euclid(self.cell_size_y as i32) == 0
            && pos.z.rem_euclid(self.cell_size_xz as i32) == 0
    }

    #[inline]
    fn sample_cached(&self, cache: &[f32], stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        if self.is_cell_corner(pos) {
            cache[self.input_index]
        } else {
            self.sample(stack, pos)
        }
    }
}

impl DensityFunction for Interpolated {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let size_xz = self.cell_size_xz as i32;
        let size_y = self.cell_size_y as i32;
        let x_in_cell = pos.x.rem_euclid(size_xz);
        let y_in_cell = pos.y.rem_euclid(size_y);
        let z_in_cell = pos.z.rem_euclid(size_xz);
        if x_in_cell == 0 && y_in_cell == 0 && z_in_cell == 0 {
            return self.sample_input(stack, pos);
        }

        let x0 = pos.x - x_in_cell;
        let y0 = pos.y - y_in_cell;
        let z0 = pos.z - z_in_cell;
        let alpha_x = x_in_cell as f32 / size_xz as f32;
        let alpha_y = y_in_cell as f32 / size_y as f32;
        let alpha_z = z_in_cell as f32 / size_xz as f32;

        // lerp(0, a, b) is exactly a, so a zero weight lets the far corner go
        // unevaluated instead of costing another walk of the input sub-tree.
        let along_x = |y: i32, z: i32| {
            let low = self.sample_input(stack, IVec3::new(x0, y, z));
            if alpha_x == 0.0 {
                low
            } else {
                low + alpha_x * (self.sample_input(stack, IVec3::new(x0 + size_xz, y, z)) - low)
            }
        };
        let along_xy = |z: i32| {
            let low = along_x(y0, z);
            if alpha_y == 0.0 {
                low
            } else {
                low + alpha_y * (along_x(y0 + size_y, z) - low)
            }
        };
        let low = along_xy(z0);
        if alpha_z == 0.0 {
            low
        } else {
            low + alpha_z * (along_xy(z0 + size_xz) - low)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Cache {
    input_index: usize,
    min_value: f32,
    max_value: f32,
}

impl RangeFunction for Cache {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

impl DensityFunction for Cache {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        DensityFunctionComponent::sample_from_stack(&stack[..=self.input_index], pos)
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
    fn sample(&self, _stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
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
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
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
enum IndependentDensityFunction {
    Constant(f32),
    OldBlendedNoise(OldBlendedNoise),
    Noise(Noise),
    ShiftA(ShiftA),
    ShiftB(ShiftB),
    Shift(Shift),
    ClampedYGradient(ClampedYGradient),
    Gradient(Gradient),
    DistanceToPoint(DistanceToPoint),
    EndOuterIslands,
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
            IndependentDensityFunction::EndOuterIslands => -0.84375,
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
            IndependentDensityFunction::EndOuterIslands => 0.5625,
        }
    }
}

impl DensityFunction for IndependentDensityFunction {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        match self {
            IndependentDensityFunction::Constant(x) => *x,
            IndependentDensityFunction::OldBlendedNoise(x) => x.sample(stack, pos),
            IndependentDensityFunction::Noise(x) => x.sample(stack, pos),
            IndependentDensityFunction::ShiftA(x) => x.sample(stack, pos),
            IndependentDensityFunction::ShiftB(x) => x.sample(stack, pos),
            IndependentDensityFunction::Shift(x) => x.sample(stack, pos),
            IndependentDensityFunction::ClampedYGradient(x) => x.sample(stack, pos),
            IndependentDensityFunction::Gradient(x) => x.sample(stack, pos),
            IndependentDensityFunction::DistanceToPoint(x) => x.sample(stack, pos),
            IndependentDensityFunction::EndOuterIslands => {
                // TODO: implement proper end islands noise sampling
                0.0
            }
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

#[derive(Clone, Debug, PartialEq)]
enum WrapperDensityFunction {
    Interpolated(Interpolated),
    Cache(Cache),
}

impl RangeFunction for WrapperDensityFunction {
    fn min_value(&self) -> f32 {
        match self {
            WrapperDensityFunction::Interpolated(x) => x.min_value(),
            WrapperDensityFunction::Cache(x) => x.min_value(),
        }
    }

    fn max_value(&self) -> f32 {
        match self {
            WrapperDensityFunction::Interpolated(x) => x.max_value(),
            WrapperDensityFunction::Cache(x) => x.max_value(),
        }
    }
}

impl DensityFunction for WrapperDensityFunction {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        match self {
            WrapperDensityFunction::Interpolated(x) => x.sample(stack, pos),
            WrapperDensityFunction::Cache(x) => x.sample(stack, pos),
        }
    }
}

impl DensityFunction for DependentDensityFunction {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        match self {
            DependentDensityFunction::Linear(x) => x.sample(stack, pos),
            DependentDensityFunction::Affine(x) => x.sample(stack, pos),
            DependentDensityFunction::PiecewiseAffine(x) => x.sample(stack, pos),
            DependentDensityFunction::Slide(x) => x.sample(stack, pos),
            DependentDensityFunction::Unary(x) => x.sample(stack, pos),
            DependentDensityFunction::Binary(x) => x.sample(stack, pos),
            DependentDensityFunction::ShiftedNoise(x) => x.sample(stack, pos),
            DependentDensityFunction::Clamp(x) => x.sample(stack, pos),
            DependentDensityFunction::RangeChoice(x) => x.sample(stack, pos),
            DependentDensityFunction::Spline(x) => x.sample(stack, pos),
            DependentDensityFunction::FindTopSurface(x) => x.sample(stack, pos),
            DependentDensityFunction::Lerp(x) => x.sample(stack, pos),
            DependentDensityFunction::Slice(x) => x.sample(stack, pos),
        }
    }
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

impl DensityFunction for Affine {
    #[inline]
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let density = DensityFunctionComponent::sample_from_stack(&stack[..=self.input_index], pos);
        density.mul_add(self.scale, self.offset)
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

impl DensityFunction for PiecewiseAffine {
    #[inline]
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let x = DensityFunctionComponent::sample_from_stack(&stack[..=self.input_index], pos);
        let scale = if x < 0.0 {
            self.neg_scale
        } else {
            self.pos_scale
        };
        x.mul_add(scale, self.offset)
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

impl DensityFunction for Slide {
    #[inline]
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let input = DensityFunctionComponent::sample_from_stack(&stack[..=self.input_index], pos);
        self.compute(input, pos.y as f32)
    }
}

impl DensityFunction for Linear {
    #[inline]
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let density = DensityFunctionComponent::sample_from_stack(&stack[..=self.input_index], pos);
        match self.operation {
            LinearOperation::Add => density + self.argument,
            LinearOperation::Multiply => density * self.argument,
        }
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

impl DensityFunction for Unary {
    #[inline]
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let density = DensityFunctionComponent::sample_from_stack(&stack[..=self.input_index], pos);
        self.operation.apply(density)
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

impl DensityFunction for ShiftedNoise {
    #[inline]
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let shifted_x =
            DensityFunctionComponent::sample_from_stack(&stack[..=self.input_x_index], pos);
        let shifted_y =
            DensityFunctionComponent::sample_from_stack(&stack[..=self.input_y_index], pos);
        let shifted_z =
            DensityFunctionComponent::sample_from_stack(&stack[..=self.input_z_index], pos);

        self.sampler.get(
            pos.x as f64 * self.xz_scale + shifted_x as f64,
            pos.y as f64 * self.y_scale + shifted_y as f64,
            pos.z as f64 * self.xz_scale + shifted_z as f64,
        )
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

impl DensityFunction for Clamp {
    #[inline]
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let density = DensityFunctionComponent::sample_from_stack(&stack[..=self.input_index], pos);
        density.clamp(self.min_value, self.max_value)
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

impl DensityFunction for RangeChoice {
    #[inline]
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let input_density =
            DensityFunctionComponent::sample_from_stack(&stack[..=self.input_index], pos);

        let idx = if input_density >= self.min_inclusion_value
            && input_density < self.max_exclusion_value
        {
            self.when_in_index
        } else {
            self.when_out_index
        };
        DensityFunctionComponent::sample_from_stack(&stack[..=idx], pos)
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

impl DensityFunction for SplineValue {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        match self {
            SplineValue::Spline(x) => x.sample(stack, pos),
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

impl DensityFunction for Spline {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let location =
            DensityFunctionComponent::sample_from_stack(&stack[..=self.input_index], pos);

        let locs = &self.locations;
        let idx_gt = Self::upper_bound(locs, location);
        let n_points = locs.len();

        if idx_gt == 0 {
            let v0 = self.values[0].sample(stack, pos);
            let d0 = self.derivatives[0];
            return if d0 == 0.0 {
                v0
            } else {
                v0 + d0 * (location - locs[0])
            };
        }

        if idx_gt == n_points {
            let i = n_points - 1;
            let v = self.values[i].sample(stack, pos);
            let d = self.derivatives[i];
            return if d == 0.0 {
                v
            } else {
                v + d * (location - locs[i])
            };
        }

        let i0 = idx_gt - 1;
        let i1 = idx_gt;

        let v0 = self.values[i0].sample(stack, pos);
        let v1 = self.values[i1].sample(stack, pos);

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

impl SplineValue {
    #[inline]
    fn sample_cached(&self, cache: &[f32], stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        match self {
            SplineValue::Spline(x) => x.sample_cached(cache, stack, pos),
            SplineValue::Constant(x) => *x,
        }
    }
}

impl Spline {
    fn sample_cached(&self, cache: &[f32], stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let location = cache[self.input_index];

        let locs = &self.locations;
        let idx_gt = Self::upper_bound(locs, location);
        let n_points = locs.len();

        if idx_gt == 0 {
            let v0 = self.values[0].sample_cached(cache, stack, pos);
            let d0 = self.derivatives[0];
            return if d0 == 0.0 {
                v0
            } else {
                v0 + d0 * (location - locs[0])
            };
        }

        if idx_gt == n_points {
            let i = n_points - 1;
            let v = self.values[i].sample_cached(cache, stack, pos);
            let d = self.derivatives[i];
            return if d == 0.0 {
                v
            } else {
                v + d * (location - locs[i])
            };
        }

        let i0 = idx_gt - 1;
        let i1 = idx_gt;

        let v0 = self.values[i0].sample_cached(cache, stack, pos);
        let v1 = self.values[i1].sample_cached(cache, stack, pos);

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

impl DensityFunction for Lerp {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let alpha = DensityFunctionComponent::sample_from_stack(&stack[..=self.alpha_index], pos);
        if alpha == 0.0 {
            return DensityFunctionComponent::sample_from_stack(&stack[..=self.first_index], pos);
        }
        let second = DensityFunctionComponent::sample_from_stack(&stack[..=self.second_index], pos);
        if alpha == 1.0 {
            return second;
        }
        let first = DensityFunctionComponent::sample_from_stack(&stack[..=self.first_index], pos);
        first + alpha * (second - first)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Slice {
    axis: Axis,
    coordinate: i32,
    input_index: usize,
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

impl DensityFunction for Slice {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let pinned = match self.axis {
            Axis::X => IVec3::new(self.coordinate, pos.y, pos.z),
            Axis::Y => IVec3::new(pos.x, self.coordinate, pos.z),
            Axis::Z => IVec3::new(pos.x, pos.y, self.coordinate),
        };
        DensityFunctionComponent::sample_from_stack(&stack[..=self.input_index], pinned)
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
    fn sample(&self, _stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
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

impl DensityFunction for FindTopSurface {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let top_y =
            (DensityFunctionComponent::sample_from_stack(&stack[..=self.upper_bound_index], pos)
                / self.cell_height)
                .floor()
                * self.cell_height;
        if top_y <= self.lower_bound {
            self.lower_bound
        } else {
            let mut current_y = top_y;
            loop {
                let sample_pos = IVec3::new(pos.x, current_y as i32, pos.z);
                let density = DensityFunctionComponent::sample_from_stack(
                    &stack[..=self.density_index],
                    sample_pos,
                );
                if density > 0.0 || current_y <= self.lower_bound {
                    return current_y;
                }
                current_y -= self.cell_height;
            }
        }
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

impl DensityFunction for Binary {
    #[inline]
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let input1_density =
            DensityFunctionComponent::sample_from_stack(&stack[..=self.input1_index], pos);
        // let input2_density =
        //     DensityFunctionComponent::sample_from_stack(&stack[..=self.input2_index], pos);
        // println!("binary {:?} d1={} d2={}", self.operation, input1_density, input2_density);
        // println!(
        //     "binary {:?} d={:?} arg1.min={:?} arg1.max={:?}",
        //     self.operation,
        //     input1_density,
        //     stack[self.input1_index].min_value(),
        //     stack[self.input1_index].max_value()
        // );
        match self.operation {
            BinaryOperation::Add => {
                let input2_density =
                    DensityFunctionComponent::sample_from_stack(&stack[..=self.input2_index], pos);
                input1_density + input2_density
            }
            BinaryOperation::Subtract => {
                let input2_density =
                    DensityFunctionComponent::sample_from_stack(&stack[..=self.input2_index], pos);
                input1_density - input2_density
            }
            BinaryOperation::Multiply => {
                if input1_density == 0.0 {
                    0.0
                } else {
                    let input2_density = DensityFunctionComponent::sample_from_stack(
                        &stack[..=self.input2_index],
                        pos,
                    );
                    input1_density * input2_density
                }
            }
            BinaryOperation::Divide => {
                if input1_density == 0.0 {
                    0.0
                } else {
                    let input2_density = DensityFunctionComponent::sample_from_stack(
                        &stack[..=self.input2_index],
                        pos,
                    );
                    input1_density / input2_density
                }
            }
            BinaryOperation::Min => {
                let input2_min = stack[self.input2_index].min_value();
                if input1_density < input2_min {
                    input1_density
                } else {
                    let input2_density = DensityFunctionComponent::sample_from_stack(
                        &stack[..=self.input2_index],
                        pos,
                    );
                    input1_density.min(input2_density)
                }
            }
            BinaryOperation::Max => {
                let input2_max = stack[self.input2_index].max_value();
                if input1_density > input2_max {
                    input1_density
                } else {
                    let input2_density = DensityFunctionComponent::sample_from_stack(
                        &stack[..=self.input2_index],
                        pos,
                    );
                    input1_density.max(input2_density)
                }
            }
            BinaryOperation::Pow | BinaryOperation::Round(_) => {
                let input2_density =
                    DensityFunctionComponent::sample_from_stack(&stack[..=self.input2_index], pos);
                self.operation.apply(input1_density, input2_density)
            }
        }
    }
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
    Wrapper(WrapperDensityFunction),
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

impl TryFrom<DensityFunctionComponent> for f32 {
    type Error = ();

    fn try_from(value: DensityFunctionComponent) -> Result<Self, Self::Error> {
        if let Some(v) = value.as_constant() {
            Ok(v)
        } else {
            Err(())
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
            DensityFunctionComponent::Wrapper(wrapper) => match wrapper {
                WrapperDensityFunction::Interpolated(x) => {
                    x.input_index = redirect[x.input_index];
                }
                WrapperDensityFunction::Cache(x) => {
                    x.input_index = redirect[x.input_index];
                }
            },
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
            DensityFunctionComponent::Wrapper(wrapper) => match wrapper {
                WrapperDensityFunction::Interpolated(x) => f(x.input_index),
                WrapperDensityFunction::Cache(x) => f(x.input_index),
            },
        }
    }
}

impl DensityFunctionComponent {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        match self {
            DensityFunctionComponent::Independent(func) => func.sample(stack, pos),
            DensityFunctionComponent::Dependent(func) => func.sample(stack, pos),
            DensityFunctionComponent::Wrapper(func) => func.sample(stack, pos),
        }
    }

    fn sample_from_stack(stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let (top_component, component_stack) = stack.split_last().unwrap();
        top_component.sample(component_stack, pos)
    }

    /// Evaluate using pre-computed cache (forward evaluation).
    /// All entries at indices < this entry's position are already computed in `cache`.
    /// No tracing spans — this is the optimized hot path.
    #[inline]
    fn sample_cached(&self, cache: &[f32], stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        match self {
            DensityFunctionComponent::Independent(f) => match f {
                IndependentDensityFunction::Constant(x) => *x,
                IndependentDensityFunction::OldBlendedNoise(x) => x.sample(&[], pos),
                IndependentDensityFunction::Noise(x) => x.sample(&[], pos),
                IndependentDensityFunction::ShiftA(x) => x.sample(&[], pos),
                IndependentDensityFunction::ShiftB(x) => x.sample(&[], pos),
                IndependentDensityFunction::Shift(x) => x.sample(&[], pos),
                IndependentDensityFunction::ClampedYGradient(x) => x.sample(&[], pos),
                IndependentDensityFunction::Gradient(x) => x.sample(&[], pos),
                IndependentDensityFunction::DistanceToPoint(x) => x.sample(&[], pos),
                IndependentDensityFunction::EndOuterIslands => 0.0,
            },
            DensityFunctionComponent::Dependent(f) => match f {
                DependentDensityFunction::Linear(x) => {
                    let input = cache[x.input_index];
                    match x.operation {
                        LinearOperation::Add => input + x.argument,
                        LinearOperation::Multiply => input * x.argument,
                    }
                }
                DependentDensityFunction::Affine(x) => {
                    cache[x.input_index].mul_add(x.scale, x.offset)
                }
                DependentDensityFunction::PiecewiseAffine(x) => {
                    let input = cache[x.input_index];
                    let scale = if input < 0.0 {
                        x.neg_scale
                    } else {
                        x.pos_scale
                    };
                    input.mul_add(scale, x.offset)
                }
                DependentDensityFunction::Slide(x) => x.compute(cache[x.input_index], pos.y as f32),
                DependentDensityFunction::Unary(x) => x.operation.apply(cache[x.input_index]),
                DependentDensityFunction::Binary(x) => x
                    .operation
                    .apply(cache[x.input1_index], cache[x.input2_index]),
                DependentDensityFunction::ShiftedNoise(x) => x.sampler.get(
                    pos.x as f64 * x.xz_scale + cache[x.input_x_index] as f64,
                    pos.y as f64 * x.y_scale + cache[x.input_y_index] as f64,
                    pos.z as f64 * x.xz_scale + cache[x.input_z_index] as f64,
                ),
                DependentDensityFunction::Clamp(x) => {
                    cache[x.input_index].clamp(x.min_value, x.max_value)
                }
                DependentDensityFunction::RangeChoice(x) => {
                    let input = cache[x.input_index];
                    if input >= x.min_inclusion_value && input < x.max_exclusion_value {
                        cache[x.when_in_index]
                    } else {
                        cache[x.when_out_index]
                    }
                }
                DependentDensityFunction::Spline(x) => x.sample_cached(cache, stack, pos),
                DependentDensityFunction::Lerp(x) => {
                    let alpha = cache[x.alpha_index];
                    if alpha == 0.0 {
                        cache[x.first_index]
                    } else if alpha == 1.0 {
                        cache[x.second_index]
                    } else {
                        let first = cache[x.first_index];
                        first + alpha * (cache[x.second_index] - first)
                    }
                }
                DependentDensityFunction::Slice(x) => x.sample(stack, pos),
                DependentDensityFunction::FindTopSurface(x) => {
                    let top_y =
                        (cache[x.upper_bound_index] / x.cell_height).floor() * x.cell_height;
                    if top_y <= x.lower_bound {
                        x.lower_bound
                    } else {
                        // Must evaluate density at different Y positions — fall back to recursive
                        let mut current_y = top_y;
                        loop {
                            let sample_pos = IVec3::new(pos.x, current_y as i32, pos.z);
                            let density = DensityFunctionComponent::sample_from_stack(
                                &stack[..=x.density_index],
                                sample_pos,
                            );
                            if density > 0.0 || current_y <= x.lower_bound {
                                return current_y;
                            }
                            current_y -= x.cell_height;
                        }
                    }
                }
            },
            DensityFunctionComponent::Wrapper(f) => match f {
                WrapperDensityFunction::Interpolated(x) => x.sample_cached(cache, stack, pos),
                WrapperDensityFunction::Cache(x) => cache[x.input_index],
            },
        }
    }
}

impl RangeFunction for DensityFunctionComponent {
    fn min_value(&self) -> f32 {
        match self {
            DensityFunctionComponent::Independent(func) => func.min_value(),
            DensityFunctionComponent::Dependent(func) => func.min_value(),
            DensityFunctionComponent::Wrapper(func) => func.min_value(),
        }
    }

    fn max_value(&self) -> f32 {
        match self {
            DensityFunctionComponent::Independent(func) => func.max_value(),
            DensityFunctionComponent::Dependent(func) => func.max_value(),
            DensityFunctionComponent::Wrapper(func) => func.max_value(),
        }
    }
}

#[inline]
pub fn lerp(delta: f32, start: f32, end: f32) -> f32 {
    start + delta * (end - start)
}

#[cfg(test)]
mod tests {
    use super::{BlendedNoise, OldBlendedNoise, RangeFunction};
    use crate::density_function::DensityFunction;
    use crate::density_function::beta_seed::seed_beta_terrain;
    use crate::proto::NoiseGeneratorSettings;
    use mcrs_minecraft_random::RandomSource;

    #[test]
    fn modern_blended_noise_unchanged() {
        let mut random = RandomSource::new(0, true);
        let noise = OldBlendedNoise::new(&mut random, 1.0, 1.0, 80.0, 160.0, 8.0, 128.0);
        for (pos, expected) in [
            ((0, 0, 0), 1050715813u32),
            ((4, 8, 4), 1044906416),
            ((8, 16, 8), 1054301785),
        ] {
            let sample = noise.sample(&[], bevy_math::IVec3::new(pos.0, pos.1, pos.2));
            assert_eq!(sample.to_bits(), expected, "blended noise moved at {pos:?}");
        }
    }

    #[test]
    fn blended_noise_never_leaves_its_declared_range() {
        let mut rng = 0x9e3779b97f4a7c15u64;
        let mut next = move || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        for (xz_scale, y_scale, xz_factor, y_factor, smear, divisor) in [
            (1.0, 1.0, 80.0, 160.0, 8.0, 128.0),
            (1.0, 1.0, 80.0, 160.0, 8.0, 1.0),
            (0.25, 0.125, 80.0, 160.0, 8.0, 128.0),
            (0.25, 0.125, 80.0, 160.0, 8.0, 1.0),
            (0.25, 0.125, 20.0, 40.0, 1.0, 128.0),
            (1000.0, 0.001, 0.5, 1000.0, 8.0, 128.0),
        ] {
            let mut random = RandomSource::new(next() as i64 as u64, true);
            let noise = BlendedNoise::new(
                &mut random,
                xz_scale,
                y_scale,
                xz_factor,
                y_factor,
                smear,
                divisor,
            );
            let bound = noise.max_value();
            let mut peak = 0.0f32;
            assert_eq!(bound, -noise.min_value());
            for _ in 0..200_000 {
                let pos = bevy_math::IVec3::new(
                    (next() % 4_000_001) as i32 - 2_000_000,
                    (next() % 2049) as i32 - 1024,
                    (next() % 4_000_001) as i32 - 2_000_000,
                );
                let value = noise.sample(&[], pos);
                peak = peak.max(value.abs());
                assert!(
                    value.abs() <= bound,
                    "{pos:?} sampled {value} outside +/-{bound} (divisor {divisor})"
                );
            }
            assert!(
                peak > bound * 0.25,
                "peak {peak} is so far under {bound} that the bound proves nothing"
            );
        }
    }

    /// Verify that disabling the /128 divisor yields exactly 128x the enabled-divisor output.
    #[test]
    fn blended_noise_no_128_divisor() {
        let mut r1 = RandomSource::new(12345, true);
        let noise_with_div = BlendedNoise::new(&mut r1, 1.0, 1.0, 80.0, 160.0, 8.0, 128.0);
        let mut r2 = RandomSource::new(12345, true);
        let noise_no_div = BlendedNoise::new(&mut r2, 1.0, 1.0, 80.0, 160.0, 8.0, 1.0);

        let pos = bevy_math::IVec3::new(4, 8, 4);
        let v_div = noise_with_div.sample(&[], pos);
        let v_nodiv = noise_no_div.sample(&[], pos);
        let ratio = v_nodiv / v_div;
        assert!(
            (ratio - 128.0).abs() < 1e-3,
            "disabling divisor should yield 128x output, got ratio {}",
            ratio
        );
    }

    #[test]
    fn beta_scale_depth_2d_finite() {
        use crate::noise::normal_noise::NoiseSampler;
        let (_, _, _, _, _, scale_noise, depth_noise) = seed_beta_terrain(12345);
        let scale_node = NoiseSampler::beta_octave_2d(scale_noise.clone(), 1.121, 2048.0);
        let depth_node = NoiseSampler::beta_octave_2d(depth_noise.clone(), 200.0, 131072.0);

        let sv_a = scale_node.get(0.0, 0.0, 0.0);
        let sv_y = scale_node.get(0.0, 100.0, 0.0);
        let dv_a = depth_node.get(0.0, 0.0, 0.0);
        let dv_y = depth_node.get(0.0, 100.0, 0.0);

        assert!(sv_a.is_finite(), "scale at origin must be finite");
        assert!(dv_a.is_finite(), "depth at origin must be finite");
        assert!(
            scale_node.get(64.0, 0.0, 64.0).is_finite(),
            "scale at (64,0,64) must be finite"
        );
        assert!(
            depth_node.get(64.0, 0.0, 64.0).is_finite(),
            "depth at (64,0,64) must be finite"
        );
        assert_eq!(sv_a, sv_y, "beta scale sampler must ignore y");
        assert_eq!(dv_a, dv_y, "beta depth sampler must ignore y");

        // Noise-cell semantics: blocks within the same 4-block cell sample identically,
        // matching the deleted BetaScale2d/BetaDepth2d (pos.x >> 2) convention.
        assert_eq!(
            scale_node.get(5.0, 0.0, 7.0),
            scale_node.get(4.0, 0.0, 4.0),
            "scale sampler must quantize to noise cells (block >> 2)"
        );
        assert_eq!(
            depth_node.get(-1.0, 0.0, -4.0),
            depth_node.get(-4.0, 0.0, -1.0),
            "depth sampler must floor-quantize negative coords to noise cells"
        );
        // (pos.x >> 2) sampled directly through sample_xz must agree with the sampler.
        assert_eq!(
            scale_node.get(13.0, 0.0, -9.0),
            scale_noise.sample_xz((13 >> 2) as f32, (-9 >> 2) as f32, 1.121, 1.121),
            "sampler must match raw sample_xz at (block >> 2) coords"
        );
        assert_eq!(
            depth_node.get(13.0, 0.0, -9.0),
            depth_noise.sample_xz((13 >> 2) as f32, (-9 >> 2) as f32, 200.0, 200.0),
            "sampler must match raw sample_xz at (block >> 2) coords"
        );
    }

    #[test]
    fn beta_blended_noise_samples_finite() {
        use std::collections::BTreeMap;
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/minecraft/worldgen/noise_settings/beta.json"
        );
        let json = std::fs::read_to_string(path).expect("beta.json should exist");
        let settings: NoiseGeneratorSettings =
            serde_json::from_str(&json).expect("beta.json should deserialize");
        let functions = load_density_functions_from_disk();
        let noises = BTreeMap::new();
        let router = super::build_functions(
            &functions,
            &noises,
            &settings,
            12345,
            mcrs_voxel_storage::VoxelId(1),
            mcrs_voxel_storage::VoxelId(86),
        );

        // Sample a column at multiple Y values to find a sign flip
        let mut all_densities = vec![];
        for y in (0..128).step_by(8) {
            let v = router.final_density_uncached(bevy_math::IVec3::new(0, y, 0));
            assert!(v.is_finite(), "density at y={y} must be finite");
            all_densities.push(v);
        }

        // Must have a sign flip somewhere in 0..128 (real terrain surface)
        let has_positive = all_densities.iter().any(|&v| v > 0.0);
        let has_negative = all_densities.iter().any(|&v| v < 0.0);
        assert!(
            has_positive && has_negative,
            "beta terrain must have both positive and negative densities across 0..128 (surface exists), got: {:?}",
            all_densities
        );
    }

    #[test]
    fn beta_build_functions_wires_final_density() {
        use std::collections::BTreeMap;
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/minecraft/worldgen/noise_settings/beta.json"
        );
        let json = std::fs::read_to_string(path).expect("beta.json should exist");
        let settings: NoiseGeneratorSettings =
            serde_json::from_str(&json).expect("beta.json should deserialize");
        let functions = load_density_functions_from_disk();
        let noises = BTreeMap::new();
        let router = super::build_functions(
            &functions,
            &noises,
            &settings,
            12345,
            mcrs_voxel_storage::VoxelId(1),
            mcrs_voxel_storage::VoxelId(86),
        );
        // Zone A must contain the two cached 2D nodes (scale/depth).
        assert!(
            router.column_boundary() > 0,
            "Zone A must be non-empty (cached 2D scale/depth nodes)"
        );
        // final_density must be wired into Zone B.
        assert!(
            router.final_density_idx() >= router.column_boundary(),
            "final_density must be in Zone B"
        );
    }

    /// Java ground truth: full 5x17x5 noise field q for chunk (0,0), seed 845,
    /// computed by replicating ChunkProviderGenerate.a / NoiseGeneratorOctaves /
    /// NoiseGeneratorPerlin / WorldChunkManager from Beta 1.7.3 (f64) byte-exactly.
    /// Order matches Java iteration: x outer, z mid, y inner (17 rows per column).
    #[test]
    fn beta_density_matches_java_ground_truth_seed845() {
        use std::collections::BTreeMap;
        const JAVA_Q: [f32; 425] = [
            666.401823,
            574.970280,
            478.681110,
            380.819986,
            284.135296,
            185.571644,
            89.311232,
            -6.778209,
            -55.011039,
            -76.112579,
            -99.501081,
            -121.413514,
            -146.573790,
            -171.385002,
            -132.712962,
            -77.591012,
            -10.000000,
            669.342833,
            574.713830,
            478.024659,
            383.470108,
            290.005486,
            188.965738,
            90.407789,
            -5.468320,
            -52.866059,
            -47.876501,
            -92.087417,
            -120.040676,
            -144.508432,
            -166.961468,
            -132.321389,
            -77.502219,
            -10.000000,
            672.734144,
            575.044405,
            480.980701,
            386.531316,
            289.862162,
            191.067845,
            92.867618,
            -6.419450,
            -51.188966,
            -76.268906,
            -102.445166,
            -118.400780,
            -140.788081,
            -161.827064,
            -131.322503,
            -77.179950,
            -10.000000,
            674.904594,
            576.065926,
            483.954049,
            392.399859,
            297.335288,
            196.118769,
            97.615984,
            0.451100,
            -49.337367,
            -75.968985,
            -102.672964,
            -115.084212,
            -137.307103,
            -158.665919,
            -127.474046,
            -75.037052,
            -10.000000,
            674.059567,
            576.376996,
            482.646532,
            390.885434,
            296.215616,
            198.251638,
            101.281242,
            6.233780,
            -47.346325,
            -76.445555,
            -103.679837,
            -113.507576,
            -134.853416,
            -157.808348,
            -124.986103,
            -73.264414,
            -10.000000,
            666.321338,
            573.754282,
            475.123270,
            377.912645,
            282.822846,
            184.034556,
            88.036727,
            -7.113503,
            -56.272692,
            -74.245514,
            -99.871382,
            -119.567582,
            -147.106212,
            -174.233256,
            -135.453875,
            -77.604082,
            -10.000000,
            667.064067,
            571.139682,
            476.550205,
            381.060976,
            286.908059,
            186.484358,
            89.676142,
            -4.531380,
            -54.241825,
            -74.624135,
            -100.887252,
            -118.632211,
            -146.377092,
            -169.212050,
            -134.893946,
            -77.778141,
            -10.000000,
            668.267770,
            571.907047,
            476.760097,
            382.324817,
            287.710318,
            187.011360,
            89.151144,
            -10.808942,
            -53.341635,
            -75.849727,
            -103.924576,
            -116.553822,
            -141.364431,
            -164.141531,
            -132.996807,
            -76.958444,
            -10.000000,
            669.074471,
            572.633310,
            478.862617,
            387.063419,
            292.075235,
            189.687961,
            92.529843,
            -8.852508,
            -53.687553,
            -74.794100,
            -101.081951,
            -114.399523,
            -137.428277,
            -159.724123,
            -128.698493,
            -75.314063,
            -10.000000,
            668.468517,
            570.640666,
            477.322101,
            385.537356,
            290.977160,
            190.682416,
            94.084826,
            -4.863159,
            -52.207734,
            -75.857369,
            -100.683690,
            -113.618091,
            -135.918690,
            -158.129795,
            -126.738469,
            -74.330833,
            -10.000000,
            660.251630,
            568.153617,
            470.324742,
            371.830594,
            277.114690,
            181.309854,
            84.001524,
            -13.314334,
            -59.489066,
            -73.863309,
            -99.237947,
            -118.479167,
            -148.881309,
            -175.314348,
            -137.261292,
            -78.901549,
            -10.000000,
            663.346902,
            565.708005,
            471.639085,
            374.919340,
            280.333148,
            183.333784,
            84.682601,
            -10.590852,
            -57.967263,
            -74.414654,
            -101.757907,
            -117.033592,
            -145.698244,
            -170.846144,
            -135.201435,
            -78.212604,
            -10.000000,
            664.982287,
            567.420291,
            473.498656,
            379.476121,
            285.950015,
            185.920028,
            87.153552,
            -10.297192,
            -56.471469,
            -75.109933,
            -102.838374,
            -115.232962,
            -141.504711,
            -165.216749,
            -132.713858,
            -77.356849,
            -10.000000,
            665.661169,
            568.743053,
            473.145448,
            382.455458,
            287.768767,
            186.227355,
            87.063511,
            -12.634489,
            -55.150207,
            -74.916734,
            -100.325923,
            -112.962969,
            -138.951443,
            -162.951349,
            -130.388150,
            -75.838621,
            -10.000000,
            663.752590,
            566.332611,
            470.935959,
            381.429498,
            286.673279,
            186.621242,
            89.461488,
            -11.957783,
            -55.266499,
            -75.807301,
            -102.301229,
            -113.103323,
            -138.112630,
            -162.233767,
            -128.716157,
            -74.759912,
            -10.000000,
            660.902565,
            569.469006,
            471.478709,
            371.553758,
            274.323295,
            181.006891,
            81.752000,
            -17.011096,
            -58.514686,
            -73.142533,
            -97.777174,
            -118.769499,
            -146.873997,
            -171.634882,
            -135.824081,
            -78.765859,
            -10.000000,
            662.076495,
            566.027445,
            471.393663,
            374.700455,
            278.211565,
            183.551246,
            83.185912,
            -14.907925,
            -58.845890,
            -73.436010,
            -98.670127,
            -115.418750,
            -143.681340,
            -168.945794,
            -134.432819,
            -78.418499,
            -10.000000,
            664.217491,
            567.223105,
            473.289568,
            379.379958,
            284.119886,
            185.970268,
            87.016266,
            -12.227263,
            -57.498530,
            -75.121935,
            -102.664991,
            -113.955838,
            -139.988759,
            -166.138875,
            -132.869586,
            -77.181576,
            -10.000000,
            664.704731,
            568.040216,
            473.040241,
            380.722090,
            287.176577,
            184.422623,
            87.538162,
            -9.589195,
            -54.660401,
            -74.029853,
            -100.387492,
            -113.290830,
            -139.539897,
            -164.076101,
            -130.401158,
            -76.018545,
            -10.000000,
            662.375413,
            565.716182,
            470.751240,
            380.620118,
            284.853525,
            185.194922,
            88.335615,
            -8.028071,
            -54.532626,
            -76.207332,
            -101.731390,
            -114.635772,
            -139.706055,
            -163.914887,
            -129.332874,
            -75.098150,
            -10.000000,
            664.370917,
            571.604140,
            472.797294,
            373.908726,
            276.933247,
            182.794375,
            84.345789,
            -14.746582,
            -58.100517,
            -73.880614,
            -95.543194,
            -117.777348,
            -143.937817,
            -166.800452,
            -131.611208,
            -77.183241,
            -10.000000,
            663.679301,
            570.768397,
            472.329104,
            376.523007,
            282.759980,
            185.809434,
            86.750410,
            -13.329684,
            -57.060859,
            -74.584002,
            -96.202405,
            -114.786064,
            -141.694237,
            -167.277362,
            -131.808050,
            -77.443974,
            -10.000000,
            664.869151,
            570.160451,
            475.823317,
            381.108105,
            288.270719,
            187.560799,
            89.310895,
            -10.293879,
            -55.514385,
            -74.190969,
            -101.284476,
            -115.137061,
            -140.664055,
            -166.131633,
            -132.072757,
            -76.846243,
            -10.000000,
            665.854372,
            569.408691,
            477.073104,
            384.448890,
            291.124860,
            188.113546,
            90.342023,
            -4.526532,
            -52.118861,
            -74.466064,
            -103.903487,
            -114.401566,
            -137.642271,
            -162.648556,
            -129.420618,
            -76.169560,
            -10.000000,
            663.084255,
            564.940685,
            474.790421,
            381.058727,
            286.722097,
            187.061383,
            89.863657,
            7.550697,
            -23.185904,
            -60.880314,
            -86.694535,
            -113.897573,
            -139.597863,
            -163.739809,
            -129.224495,
            -76.082980,
            -10.000000,
        ];
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/minecraft/worldgen/noise_settings/beta.json"
        );
        let json = std::fs::read_to_string(path).expect("beta.json should exist");
        let settings: NoiseGeneratorSettings =
            serde_json::from_str(&json).expect("beta.json should deserialize");
        let functions = load_density_functions_from_disk();
        let noises = BTreeMap::new();
        let router = super::build_functions(
            &functions,
            &noises,
            &settings,
            845,
            mcrs_voxel_storage::VoxelId(1),
            mcrs_voxel_storage::VoxelId(86),
        );
        let mut i = 0;
        let mut max_diff = 0.0_f32;
        for cx in 0..5i32 {
            for cz in 0..5i32 {
                for cy in 0..17i32 {
                    let pos = bevy_math::IVec3::new(cx * 4, cy * 8, cz * 4);
                    let rust_v = router.final_density_uncached(pos);
                    let java_v = JAVA_Q[i];
                    let diff = (rust_v - java_v).abs();
                    max_diff = max_diff.max(diff);
                    // Residual tolerance covers f32 noise accumulation and the
                    // climate sample-point offset (quart origins vs Java's
                    // per-chunk cell*3+1 stride, which is seam-inconsistent in
                    // Java itself). All structural divergence is far above this.
                    assert!(
                        diff < 8.0,
                        "q({},{},{}): java={} rust={} diff={}",
                        cx,
                        cz,
                        cy,
                        java_v,
                        rust_v,
                        diff
                    );
                    if java_v.abs() > 20.0 {
                        assert_eq!(
                            java_v > 0.0,
                            rust_v > 0.0,
                            "sign mismatch at q({},{},{}): java={} rust={}",
                            cx,
                            cz,
                            cy,
                            java_v,
                            rust_v
                        );
                    }
                    i += 1;
                }
            }
        }
        assert!(max_diff < 8.0, "max diff {}", max_diff);
    }
    #[test]
    #[ignore]
    fn beta_dump_density_chunk00_seed845() {
        use std::collections::BTreeMap;
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/minecraft/worldgen/noise_settings/beta.json"
        );
        let json = std::fs::read_to_string(path).expect("beta.json should exist");
        let settings: NoiseGeneratorSettings =
            serde_json::from_str(&json).expect("beta.json should deserialize");
        let functions = load_density_functions_from_disk();
        let noises = BTreeMap::new();
        let router = super::build_functions(
            &functions,
            &noises,
            &settings,
            845,
            mcrs_voxel_storage::VoxelId(1),
            mcrs_voxel_storage::VoxelId(86),
        );
        for cx in 0..5i32 {
            for cz in 0..5i32 {
                for cy in 0..17i32 {
                    let pos = bevy_math::IVec3::new(cx * 4, cy * 8, cz * 4);
                    let d = router.final_density_uncached(pos);
                    println!("q {} {} {} {:.6}", cx, cz, cy, d);
                }
            }
        }
    }

    #[test]
    fn beta_json_deserializes() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/minecraft/worldgen/noise_settings/beta.json"
        );
        let json = std::fs::read_to_string(path).expect("beta.json should exist at assets path");
        let settings: NoiseGeneratorSettings =
            serde_json::from_str(&json).expect("beta.json should deserialize without error");
        assert_eq!(settings.sea_level, 64);
        assert_eq!(settings.noise.height, 128);
        assert!(settings.legacy_random_source);
    }

    fn recurse_density_functions(
        dir: &std::path::Path,
        prefix: &str,
        map: &mut std::collections::BTreeMap<
            mcrs_minecraft_core::ResourceLocation,
            crate::density_function::ProtoDensityFunction,
        >,
    ) {
        use crate::density_function::proto::DensityFunctionHolder;
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let subdir = entry.file_name().to_string_lossy().to_string();
                let new_prefix = if prefix.is_empty() {
                    subdir
                } else {
                    format!("{}/{}", prefix, subdir)
                };
                recurse_density_functions(&path, &new_prefix, map);
            } else if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let json = std::fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                let holder = serde_json::from_str::<DensityFunctionHolder>(&json)
                    .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                let function = match holder {
                    DensityFunctionHolder::Owned(pdf) => *pdf,
                    DensityFunctionHolder::Value(value) => {
                        crate::density_function::ProtoDensityFunction::Constant(value)
                    }
                    DensityFunctionHolder::Reference(target) => {
                        panic!(
                            "{}: a density function file must not be a bare reference to {target}",
                            path.display()
                        )
                    }
                };
                let stem = path.file_stem().unwrap().to_string_lossy();
                let key = if prefix.is_empty() {
                    format!("minecraft:{}", stem)
                } else {
                    format!("minecraft:{}/{}", prefix, stem)
                };
                let ident = key
                    .parse::<mcrs_minecraft_core::ResourceLocation>()
                    .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                map.insert(ident, function);
            }
        }
    }

    /// Load all density_function JSON assets recursively into a `ProtoDensityFunction` map.
    fn load_density_functions_from_disk() -> std::collections::BTreeMap<
        mcrs_minecraft_core::ResourceLocation,
        crate::density_function::ProtoDensityFunction,
    > {
        let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/minecraft/worldgen/density_function");
        let mut map = std::collections::BTreeMap::new();
        recurse_density_functions(&base, "", &mut map);
        map
    }

    /// Load all noise JSON assets into a `NoiseParam` map.
    fn load_noises_from_disk() -> std::collections::BTreeMap<
        mcrs_minecraft_core::ResourceLocation,
        crate::density_function::proto::NoiseParam,
    > {
        let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/minecraft/worldgen/noise");
        let mut map = std::collections::BTreeMap::new();
        recurse_noises(&base, "", &mut map);
        map
    }

    fn recurse_noises(
        dir: &std::path::Path,
        prefix: &str,
        map: &mut std::collections::BTreeMap<
            mcrs_minecraft_core::ResourceLocation,
            crate::density_function::proto::NoiseParam,
        >,
    ) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let subdir = entry.file_name().to_string_lossy().to_string();
                let new_prefix = if prefix.is_empty() {
                    subdir
                } else {
                    format!("{}/{}", prefix, subdir)
                };
                recurse_noises(&path, &new_prefix, map);
                continue;
            }
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let json = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let noise = serde_json::from_str::<crate::density_function::proto::NoiseParam>(&json)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let stem = path.file_stem().unwrap().to_string_lossy();
            let key = if prefix.is_empty() {
                format!("minecraft:{}", stem)
            } else {
                format!("minecraft:{}/{}", prefix, stem)
            };
            let ident = key
                .parse::<mcrs_minecraft_core::ResourceLocation>()
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            map.insert(ident, noise);
        }
    }

    fn count_json_files(dir: &std::path::Path) -> usize {
        let Ok(entries) = std::fs::read_dir(dir) else {
            panic!("{} must exist", dir.display());
        };
        entries
            .flatten()
            .map(|entry| {
                let path = entry.path();
                if path.is_dir() {
                    count_json_files(&path)
                } else {
                    usize::from(path.extension().and_then(|s| s.to_str()) == Some("json"))
                }
            })
            .sum()
    }

    fn settings_for(settings_file: &str) -> NoiseGeneratorSettings {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/minecraft/worldgen/noise_settings")
            .join(settings_file);
        let json =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        serde_json::from_str(&json).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    fn router_for(settings_file: &str) -> super::NoiseRouter {
        let settings = settings_for(settings_file);
        super::build_functions(
            &load_density_functions_from_disk(),
            &load_noises_from_disk(),
            &settings,
            2,
            mcrs_voxel_storage::VoxelId(1),
            mcrs_voxel_storage::VoxelId(86),
        )
    }

    /// A `DensityCache` is not one root's private scratch: every root queried
    /// through a shared cache, in any order, must answer exactly as a cache
    /// built for it alone does.
    #[test]
    fn a_shared_cache_answers_every_root() {
        let router = router_for("overworld.json");
        let mut roots = router.roots();
        roots.sort_by_key(|&(_, index)| index);

        for (x, y, z) in [(37, 55, -19), (37, 71, -19), (38, 55, -19)] {
            let pos = bevy_math::IVec3::new(x, y, z);
            let alone: Vec<f32> = roots
                .iter()
                .map(|&(_, index)| router.sample_root(index, pos, &mut router.new_cache()))
                .collect();

            for descending in [false, true] {
                let mut shared = router.new_cache();
                let mut order: Vec<usize> = (0..roots.len()).collect();
                if descending {
                    order.reverse();
                }
                for k in order {
                    let (name, index) = roots[k];
                    let got = router.sample_root(index, pos, &mut shared);
                    assert_eq!(
                        got.to_bits(),
                        alone[k].to_bits(),
                        "{name} at {pos:?} through a shared cache (descending={descending}) \
                         gave {got}, alone it gives {}",
                        alone[k]
                    );
                }
            }
        }
    }

    /// The slicing rewrite has to leave every parent-to-child edge either uniform
    /// in its axes or bridged by slices pinning exactly the axes the child drops,
    /// because that is what lets a consumer read the child's invariance off the
    /// child alone.
    #[test]
    fn slicing_pins_every_axis_a_child_drops() {
        use super::ALL_AXES;
        use super::proto::{
            DensityFunctionHolder, InlineReference, ProtoDensityFunction, RewriteRule,
            SliceUniformAxes, is_uniform_axis_slice_leaf, sliced_axes,
        };

        let functions = load_density_functions_from_disk();
        let inline = InlineReference(&functions);
        let mut inserted = 0usize;
        for settings_file in ["overworld.json", "beta.json"] {
            let settings = settings_for(settings_file);
            let nr = &settings.noise_router;
            for holder in [
                &nr.temperature,
                &nr.vegetation,
                &nr.continents,
                &nr.erosion,
                &nr.depth,
                &nr.ridges,
                &nr.chunk_surface_level,
                &nr.final_density,
            ] {
                let original = inline.rewrite(holder);
                let rewritten = SliceUniformAxes::new(ALL_AXES).rewrite(&original);
                assert_eq!(rewritten.domain_axes(), original.domain_axes());

                let mut pending = vec![(ALL_AXES, rewritten.clone())];
                while let Some((parent_axes, holder)) = pending.pop() {
                    if is_uniform_axis_slice_leaf(&holder) {
                        continue;
                    }
                    let axes = holder.domain_axes();
                    let pinned = sliced_axes(&holder);
                    inserted += pinned.count_ones() as usize;
                    assert_eq!(
                        pinned,
                        parent_axes & !axes,
                        "{settings_file}: a child with axes {axes:#05b} under a parent with \
                         {parent_axes:#05b} pins {pinned:#05b}"
                    );
                    let mut inner = &holder;
                    while let DensityFunctionHolder::Owned(f) = inner {
                        let ProtoDensityFunction::Slice { input, .. } = &**f else {
                            break;
                        };
                        inner = input;
                    }
                    if let DensityFunctionHolder::Owned(f) = inner {
                        f.visit_children(&mut |child| pending.push((axes, child.clone())));
                    }
                }
            }
        }
        assert!(inserted > 0, "the rewrite never fired");
        println!("{inserted} axes pinned across both presets");
    }

    /// The rewrite rules run on the AST, so their axis masks have to be at least
    /// as wide as the ones the compiled stack is checked against by sampling.
    #[test]
    fn ast_domain_axes_cover_the_compiled_ones() {
        use super::proto::{InlineReference, RewriteRule};

        let functions = load_density_functions_from_disk();
        let inline = InlineReference(&functions);
        for settings_file in ["overworld.json", "beta.json"] {
            let settings = settings_for(settings_file);
            let router = router_for(settings_file);
            let axes = router.domain_axes();
            let nr = &settings.noise_router;
            let roots = [
                ("temperature", &nr.temperature),
                ("vegetation", &nr.vegetation),
                ("continents", &nr.continents),
                ("erosion", &nr.erosion),
                ("depth", &nr.depth),
                ("ridges", &nr.ridges),
                ("chunk_surface_level", &nr.chunk_surface_level),
                ("final_density", &nr.final_density),
            ];
            for (name, holder) in roots {
                let ast_axes = inline.rewrite(holder).domain_axes();
                let index = router
                    .roots()
                    .into_iter()
                    .find(|(root, _)| *root == name)
                    .unwrap()
                    .1;
                assert_eq!(
                    axes[index] & !ast_axes,
                    0,
                    "{settings_file}: {name} compiled axes {:#05b} escape ast axes {ast_axes:#05b}",
                    axes[index]
                );
            }
        }
    }

    /// A too-narrow axis mask silently corrupts terrain once a consumer pins the
    /// axis it dropped, so every entry of every router is checked directly:
    /// zeroing the coordinates outside the mask must not move the value at all.
    #[test]
    fn domain_axes_are_sound() {
        use super::{AXIS_X, AXIS_Y, AXIS_Z};

        let mut positions = vec![
            bevy_math::IVec3::new(0, 64, 0),
            bevy_math::IVec3::new(1, 5, 2),
            bevy_math::IVec3::new(13, -37, 7),
            bevy_math::IVec3::new(-19, 123, 41),
            bevy_math::IVec3::new(-4, 0, -9),
            bevy_math::IVec3::new(1000, 200, -1000),
            bevy_math::IVec3::new(-333, -60, 333),
            bevy_math::IVec3::new(7, 319, 7),
        ];
        // The slide gradients and the cell lattice both branch on Y, so the
        // sweep must land on and between their boundaries, not only near them.
        for y in [
            -64, -63, -56, -48, -40, -39, 8, 232, 239, 240, 248, 255, 256, 257,
        ] {
            positions.push(bevy_math::IVec3::new(3, y, -5));
            positions.push(bevy_math::IVec3::new(-16, y, 32));
        }
        let mut state = 0x2545_f491_4f6c_dd1du64;
        for _ in 0..256 {
            let mut next = || {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state
            };
            positions.push(bevy_math::IVec3::new(
                (next() % 4096) as i32 - 2048,
                (next() % 512) as i32 - 128,
                (next() % 4096) as i32 - 2048,
            ));
        }

        for settings_file in ["overworld.json", "beta.json"] {
            let router = router_for(settings_file);
            let stack = &router.stack;
            let axes = router.domain_axes();

            for (i, entry) in stack.iter().enumerate() {
                entry.visit_input_indices(&mut |j| {
                    assert!(j < i, "{settings_file}: entry {i} reads later entry {j}");
                });
            }

            let mut checked = 0usize;
            let mut restricted_entries = 0usize;
            for i in 0..stack.len() {
                if axes[i] != super::ALL_AXES {
                    restricted_entries += 1;
                }
            }
            for &pos in &positions {
                for i in 0..stack.len() {
                    let mask = axes[i];
                    let pinned = bevy_math::IVec3::new(
                        if mask & AXIS_X != 0 { pos.x } else { 0 },
                        if mask & AXIS_Y != 0 { pos.y } else { 0 },
                        if mask & AXIS_Z != 0 { pos.z } else { 0 },
                    );
                    if pinned == pos {
                        continue;
                    }
                    checked += 1;
                    let full =
                        super::DensityFunctionComponent::sample_from_stack(&stack[..=i], pos);
                    let restricted =
                        super::DensityFunctionComponent::sample_from_stack(&stack[..=i], pinned);
                    assert_eq!(
                        full.to_bits(),
                        restricted.to_bits(),
                        "{settings_file}: entry {i} ({}) declares axes {mask:#05b} but {pos:?} -> {full} \
                         differs from {pinned:?} -> {restricted}",
                        router.node_labels[i],
                    );
                }
            }

            let mut conservative = Vec::new();
            for i in 0..stack.len() {
                if axes[i] & AXIS_Y != 0 {
                    assert!(
                        router.per_block[i],
                        "{settings_file}: entry {i} ({}) varies with Y but per_block says otherwise",
                        router.node_labels[i],
                    );
                } else if router.per_block[i] {
                    conservative.push((i, router.node_labels[i].clone()));
                }
            }
            assert!(restricted_entries > 0 && checked > 0);
            println!(
                "{settings_file}: {} entries, {restricted_entries} with a restricted domain, \
                 {checked} pinned-coordinate comparisons, {} per_block-only",
                stack.len(),
                conservative.len(),
            );
            for (i, label) in &conservative {
                println!("  per_block over-approximates entry {i} ({label})");
            }
        }
    }

    /// Regression gate: the modern NoiseRouter built from overworld.json via
    /// build_functions must remain structurally and numerically unchanged after
    /// the data-driven preset refactor.  Any perturbation to the modern path
    /// (wrong seed forwarding, different build_functions code path, changed
    /// stack ordering) will cause this test to fail.
    #[test]
    fn overworld_router_unchanged() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/minecraft/worldgen/noise_settings/overworld.json"
        );
        let json = std::fs::read_to_string(path).expect("overworld.json must exist");
        let settings: NoiseGeneratorSettings =
            serde_json::from_str(&json).expect("overworld.json must deserialize");

        let functions: std::collections::BTreeMap<
            mcrs_minecraft_core::ResourceLocation,
            crate::density_function::ProtoDensityFunction,
        > = load_density_functions_from_disk();
        let noises: std::collections::BTreeMap<
            mcrs_minecraft_core::ResourceLocation,
            crate::density_function::proto::NoiseParam,
        > = load_noises_from_disk();

        let router = super::build_functions(
            &functions,
            &noises,
            &settings,
            2,
            mcrs_voxel_storage::VoxelId(1),
            mcrs_voxel_storage::VoxelId(86),
        );

        assert!(
            router.final_density_idx() > 0,
            "modern router final_density_index must be non-zero (wired)"
        );
        assert!(
            router.column_boundary() > 0,
            "modern router must have Zone A column-only entries"
        );

        let sample = router.final_density_uncached(bevy_math::IVec3::new(0, 64, 0));
        assert!(
            sample.is_finite(),
            "modern router sample at (0,64,0) must be finite"
        );

        assert_eq!(
            sample.to_bits(),
            3168561611u32,
            "modern router sample must match baseline (seed=2, pos=(0,64,0))"
        );
    }

    /// Regression gate: the Beta router must produce a numerically distinct
    /// final_density sample from the modern overworld router, confirming the
    /// two `build_functions` codepaths diverge as expected.
    #[test]
    fn beta_router_differs_from_modern() {
        let beta_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/minecraft/worldgen/noise_settings/beta.json"
        );
        let beta_json = std::fs::read_to_string(beta_path).expect("beta.json must exist");
        let beta_settings: NoiseGeneratorSettings =
            serde_json::from_str(&beta_json).expect("beta.json must deserialize");

        let overworld_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/minecraft/worldgen/noise_settings/overworld.json"
        );
        let overworld_json =
            std::fs::read_to_string(overworld_path).expect("overworld.json must exist");
        let overworld_settings: NoiseGeneratorSettings =
            serde_json::from_str(&overworld_json).expect("overworld.json must deserialize");

        let functions = load_density_functions_from_disk();
        let noises = load_noises_from_disk();

        let modern_router = super::build_functions(
            &functions,
            &noises,
            &overworld_settings,
            2,
            mcrs_voxel_storage::VoxelId(1),
            mcrs_voxel_storage::VoxelId(86),
        );
        let beta_router = super::build_functions(
            &functions,
            &noises,
            &beta_settings,
            2,
            mcrs_voxel_storage::VoxelId(1),
            mcrs_voxel_storage::VoxelId(86),
        );

        let pos = bevy_math::IVec3::new(0, 64, 0);
        let modern_sample = modern_router.final_density_uncached(pos);
        let beta_sample = beta_router.final_density_uncached(pos);

        assert!(modern_sample.is_finite(), "modern sample must be finite");
        assert!(beta_sample.is_finite(), "beta sample must be finite");
        assert_ne!(
            modern_sample.to_bits(),
            beta_sample.to_bits(),
            "beta router must produce a different final_density than the modern router at (0,64,0)"
        );
    }

    /// Every shipped worldgen asset must deserialize. A loader that swallowed
    /// errors once hid 42 of 62 unparseable density functions, so this asserts
    /// the on-disk file count and the parsed count agree.
    #[test]
    fn whole_worldgen_corpus_parses() {
        let assets = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/minecraft/worldgen");

        let functions = load_density_functions_from_disk();
        assert_eq!(
            functions.len(),
            count_json_files(&assets.join("density_function")),
            "every density_function asset must parse"
        );

        for (ident, function) in &functions {
            // A bare constant ships as a naked number, never as a tagged object.
            if matches!(
                function,
                crate::density_function::ProtoDensityFunction::Constant(_)
            ) {
                continue;
            }
            let reencoded = serde_json::to_string(function).unwrap();
            let roundtripped =
                serde_json::from_str::<crate::density_function::ProtoDensityFunction>(&reencoded)
                    .unwrap_or_else(|e| panic!("{ident}: {e}\n{reencoded}"));
            assert_eq!(&roundtripped, function, "{ident} must round-trip");
        }

        let noises = load_noises_from_disk();
        assert_eq!(
            noises.len(),
            count_json_files(&assets.join("noise")),
            "every noise asset must parse"
        );

        let settings_dir = assets.join("noise_settings");
        let mut settings_count = 0;
        for entry in std::fs::read_dir(&settings_dir).unwrap().flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let json = std::fs::read_to_string(&path).unwrap();
            let settings: NoiseGeneratorSettings =
                serde_json::from_str(&json).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            super::build_functions(
                &functions,
                &noises,
                &settings,
                2,
                mcrs_voxel_storage::VoxelId(1),
                mcrs_voxel_storage::VoxelId(86),
            );
            settings_count += 1;
        }
        assert_eq!(settings_count, count_json_files(&settings_dir));
    }
}
