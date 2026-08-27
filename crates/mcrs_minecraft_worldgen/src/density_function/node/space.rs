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
use crate::spline::SplineFunction;
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

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Interpolated {
    pub(crate) input_index: usize,
    pub(crate) input_members: Box<[u32]>,
    pub(crate) cell_size_xz: u32,
    pub(crate) cell_size_y: u32,
    pub(crate) cell_size_xz_inv: f32,
    pub(crate) cell_size_y_inv: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ClampedYGradient {
    pub(crate) from_y: f32,
    pub(crate) to_y: f32,
    pub(crate) from_value: f32,
    pub(crate) to_value: f32,
}
impl ClampedYGradient {
    pub(crate) fn range(&self) -> Interval {
        Interval::encapsulating(self.from_value, self.to_value)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Gradient {
    pub(crate) axis: Axis,
    pub(crate) tiling: TilingMode,
    pub(crate) from_coordinate: i32,
    pub(crate) to_coordinate: i32,
    pub(crate) from_value: f32,
    pub(crate) to_value: f32,
}

impl Gradient {
    pub(crate) fn range(&self) -> Interval {
        Interval::encapsulating(self.from_value, self.to_value)
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
pub(crate) struct EndIslands {
    pub(crate) noise: Arc<SimplexNoise>,
}

impl EndIslands {
    pub(crate) fn new(world_seed: u64) -> Self {
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

impl EndIslands {
    pub(crate) fn range(&self) -> Interval {
        Interval::of(-0.84375, 0.5625)
    }
}

impl DensityFunction for EndIslands {
    fn sample(&self, pos: IVec3) -> f32 {
        (self.height(pos.x / 8, pos.z / 8) - 8.0) / 128.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FindTopSurface {
    pub(crate) density_index: usize,
    pub(crate) density_members: Box<[u32]>,
    pub(crate) upper_bound_index: usize,
    pub(crate) lower_bound: f32,
    pub(crate) cell_height: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Slice {
    pub(crate) axis: Axis,
    pub(crate) coordinate: i32,
    pub(crate) input_index: usize,
    pub(crate) input_members: Box<[u32]>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DistanceToPoint {
    pub(crate) point: IVec3,
    pub(crate) metric: DistanceMetric,
}

impl DistanceToPoint {
    pub(crate) fn range(&self) -> Interval {
        Interval::of(0.0, f32::INFINITY)
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

impl Slice {
    pub(crate) fn fill(&self, arena: Arena<'_>, volume: &Volume, out: &mut [f32]) {
        let pinned = self.pinned_volume(volume);
        if &pinned == volume {
            arena.fill_members(&self.input_members, volume, out);
            return;
        }
        let mut sliced = vec![0.0f32; pinned.len()];
        arena.fill_members(&self.input_members, &pinned, &mut sliced);
        for z in 0..volume.size().z {
            for x in 0..volume.size().x {
                for y in 0..volume.size().y {
                    let source = match self.axis {
                        Axis::X => pinned.index_unchecked(0, y, z),
                        Axis::Y => pinned.index_unchecked(x, 0, z),
                        Axis::Z => pinned.index_unchecked(x, y, 0),
                    };
                    out[volume.index_unchecked(x, y, z)] = sliced[source];
                }
            }
        }
    }

    /// `volume` with this node's axis collapsed onto its pinned coordinate.
    fn pinned_volume(&self, volume: &Volume) -> Volume {
        let mut size = volume.size();
        let mut min = volume.min_block();
        let axis = match self.axis {
            Axis::X => 0,
            Axis::Y => 1,
            Axis::Z => 2,
        };
        size[axis] = 1;
        min[axis] = self.coordinate;
        Volume::new(size, min, volume.step_block())
    }
}

impl FindTopSurface {
    /// The highest cell boundary at or below each upper bound whose density is
    /// positive, probing downwards one cell at a time.
    pub(crate) fn fill(
        &self,
        arena: Arena<'_>,
        positions: &[IVec3],
        upper_bounds: &[f32],
        out: &mut [f32],
    ) {
        let mut probed = [0.0f32];
        let mut scratch = MemberScratch::default();
        for (p, slot) in out.iter_mut().enumerate() {
            let top_y = (upper_bounds[p] / self.cell_height).floor() * self.cell_height;
            *slot = if top_y <= self.lower_bound {
                self.lower_bound
            } else {
                let mut current_y = top_y;
                loop {
                    let probe =
                        Volume::point(IVec3::new(positions[p].x, current_y as i32, positions[p].z));
                    arena.fill_members_with(
                        &self.density_members,
                        &probe,
                        &mut probed,
                        &mut scratch,
                    );
                    if probed[0] > 0.0 || current_y <= self.lower_bound {
                        break current_y;
                    }
                    current_y -= self.cell_height;
                }
            };
        }
    }
}

impl Interpolated {
    /// Whether `volume` already samples this node's cell lattice, so the input
    /// can be passed straight through with no interpolation.
    pub(crate) fn is_lattice_volume(&self, volume: &Volume) -> bool {
        let xz = self.cell_size_xz as i32;
        let y = self.cell_size_y as i32;
        (volume.step_block().x == xz || volume.size().x == 1)
            && (volume.step_block().y == y || volume.size().y == 1)
            && (volume.step_block().z == xz || volume.size().z == 1)
            && volume.min_block().x.rem_euclid(xz) == 0
            && volume.min_block().y.rem_euclid(y) == 0
            && volume.min_block().z.rem_euclid(xz) == 0
    }

    /// `input_row` must hold the input's values over `volume` whenever
    /// [`Interpolated::is_lattice_volume`] holds; off the lattice this node
    /// resamples its input over its own cell volume and never reads it.
    pub(crate) fn fill_volume(
        &self,
        arena: Arena<'_>,
        volume: &Volume,
        input_row: &[f32],
        out: &mut [f32],
    ) {
        if self.is_lattice_volume(volume) {
            out.copy_from_slice(input_row);
        } else if volume.len() == 1 {
            // A single position combines the eight corners exactly, where a
            // volume accumulates along Y. Vanilla splits the same two ways, and
            // the block values a chunk fill produces come from the second.
            out[0] = self.sample_point(arena, volume.min_block());
        } else if volume.step_block() == IVec3::ONE {
            self.fill_block_step(arena, volume, out);
        } else {
            let block_volume =
                Volume::dense(volume.size() * volume.step_block(), volume.min_block());
            let mut block = vec![0.0f32; block_volume.len()];
            self.fill_block_step(arena, &block_volume, &mut block);
            for z in 0..volume.size().z {
                for x in 0..volume.size().x {
                    for y in 0..volume.size().y {
                        out[volume.index_unchecked(x, y, z)] = block[block_volume.index_unchecked(
                            x * volume.step_block().x,
                            y * volume.step_block().y,
                            z * volume.step_block().z,
                        )];
                    }
                }
            }
        }
    }

    fn sample_point(&self, arena: Arena<'_>, pos: IVec3) -> f32 {
        let size_xz = self.cell_size_xz as i32;
        let size_y = self.cell_size_y as i32;
        let x_in_cell = pos.x.rem_euclid(size_xz);
        let y_in_cell = pos.y.rem_euclid(size_y);
        let z_in_cell = pos.z.rem_euclid(size_xz);
        let cell = Volume::new(
            IVec3::splat(2),
            IVec3::new(pos.x - x_in_cell, pos.y - y_in_cell, pos.z - z_in_cell),
            IVec3::new(size_xz, size_y, size_xz),
        );
        let mut corners = [0.0f32; 8];
        arena.fill_members(&self.input_members, &cell, &mut corners);

        let alpha_x = x_in_cell as f32 / size_xz as f32;
        let alpha_y = y_in_cell as f32 / size_y as f32;
        let alpha_z = z_in_cell as f32 / size_xz as f32;
        let at = |x: i32, y: i32, z: i32| corners[cell.index_unchecked(x, y, z)];
        let along_x = |y: i32, z: i32| lerp(alpha_x, at(0, y, z), at(1, y, z));
        let along_xy = |z: i32| lerp(alpha_y, along_x(0, z), along_x(1, z));
        lerp(alpha_z, along_xy(0), along_xy(1))
    }

    fn fill_block_step(&self, arena: Arena<'_>, volume: &Volume, out: &mut [f32]) {
        let xz = self.cell_size_xz as i32;
        let sy = self.cell_size_y as i32;
        let min_cell_x = volume.min_block().x.div_euclid(xz);
        let min_cell_y = volume.min_block().y.div_euclid(sy);
        let min_cell_z = volume.min_block().z.div_euclid(xz);
        let cell_count_x = volume.max_block().x.div_euclid(xz) - min_cell_x + 1;
        let cell_count_y = volume.max_block().y.div_euclid(sy) - min_cell_y + 1;
        let cell_count_z = volume.max_block().z.div_euclid(xz) - min_cell_z + 1;
        let cell_volume = Volume::new(
            IVec3::new(
                cell_count_x + i32::from(volume.max_block().x.rem_euclid(xz) != 0),
                cell_count_y + i32::from(volume.max_block().y.rem_euclid(sy) != 0),
                cell_count_z + i32::from(volume.max_block().z.rem_euclid(xz) != 0),
            ),
            IVec3::new(min_cell_x * xz, min_cell_y * sy, min_cell_z * xz),
            IVec3::new(xz, sy, xz),
        );

        let mut cell = vec![0.0f32; cell_volume.len()];
        arena.fill_members(&self.input_members, &cell_volume, &mut cell);

        for cell_z in 0..cell_count_z {
            let next_cell_z = (cell_z + 1).min(cell_volume.size().z - 1);
            for cell_x in 0..cell_count_x {
                let next_cell_x = (cell_x + 1).min(cell_volume.size().x - 1);
                let mut v000 = cell[cell_volume.index_unchecked(cell_x, 0, cell_z)];
                let mut v100 = cell[cell_volume.index_unchecked(next_cell_x, 0, cell_z)];
                let mut v001 = cell[cell_volume.index_unchecked(cell_x, 0, next_cell_z)];
                let mut v101 = cell[cell_volume.index_unchecked(next_cell_x, 0, next_cell_z)];
                for cell_y in 0..cell_count_y {
                    let next_cell_y = (cell_y + 1).min(cell_volume.size().y - 1);
                    let v010 = cell[cell_volume.index_unchecked(cell_x, next_cell_y, cell_z)];
                    let v110 = cell[cell_volume.index_unchecked(next_cell_x, next_cell_y, cell_z)];
                    let v011 = cell[cell_volume.index_unchecked(cell_x, next_cell_y, next_cell_z)];
                    let v111 =
                        cell[cell_volume.index_unchecked(next_cell_x, next_cell_y, next_cell_z)];
                    self.fill_cell(
                        out,
                        volume,
                        &cell_volume,
                        IVec3::new(cell_x, cell_y, cell_z),
                        [v000, v100, v010, v110, v001, v101, v011, v111],
                    );
                    v000 = v010;
                    v100 = v110;
                    v001 = v011;
                    v101 = v111;
                }
            }
        }
    }

    fn fill_cell(
        &self,
        out: &mut [f32],
        output_volume: &Volume,
        cell_volume: &Volume,
        cell: IVec3,
        [v000, v100, v010, v110, v001, v101, v011, v111]: [f32; 8],
    ) {
        let cell_output_x = cell_volume.block_x(cell.x) - output_volume.min_block().x;
        let cell_output_y = cell_volume.block_y(cell.y) - output_volume.min_block().y;
        let cell_output_z = cell_volume.block_z(cell.z) - output_volume.min_block().z;
        let x0 = 0.max(-cell_output_x);
        let y0 = 0.max(-cell_output_y);
        let z0 = 0.max(-cell_output_z);
        let x1 = (self.cell_size_xz as i32).min(output_volume.size().x - cell_output_x) - 1;
        let y1 = (self.cell_size_y as i32).min(output_volume.size().y - cell_output_y) - 1;
        let z1 = (self.cell_size_xz as i32).min(output_volume.size().z - cell_output_z) - 1;

        for z in z0..=z1 {
            let output_z = cell_output_z + z;
            let alpha_z = z as f32 * self.cell_size_xz_inv;
            let v00_ = lerp(alpha_z, v000, v001);
            let v01_ = lerp(alpha_z, v010, v011);
            let v10_ = lerp(alpha_z, v100, v101);
            let v11_ = lerp(alpha_z, v110, v111);

            for x in x0..=x1 {
                let output_x = cell_output_x + x;
                let alpha_x = x as f32 * self.cell_size_xz_inv;
                let v_0_ = lerp(alpha_x, v00_, v10_);
                let v_1_ = lerp(alpha_x, v01_, v11_);
                let value_step = (v_1_ - v_0_) * self.cell_size_y_inv;
                let mut value = v_0_ + value_step * y0 as f32;
                let start = output_volume.index_unchecked(output_x, cell_output_y + y0, output_z);

                for slot in &mut out[start..start + (y1 - y0 + 1).max(0) as usize] {
                    *slot = value;
                    value += value_step;
                }
            }
        }
    }
}

impl DensitySampler for Slice {
    fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
        self.fill(ctx.arena, ctx.volume, out);
    }
}

impl DensitySampler for FindTopSurface {
    fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
        self.fill(
            ctx.arena,
            ctx.positions,
            ctx.row(self.upper_bound_index),
            out,
        );
    }
}

impl DensitySampler for Interpolated {
    fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
        self.fill_volume(ctx.arena, ctx.volume, ctx.row(self.input_index), out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dense_layout_is_contiguous_from_min_block() {
        let v = Volume::dense(IVec3::new(3, 5, 2), IVec3::new(-7, 12, 40));
        assert_eq!(v.len(), 30);
        assert_eq!(v.max_block(), IVec3::new(-5, 16, 41));
        let mut seen = vec![false; v.len()];
        for z in 0..v.size().z {
            for x in 0..v.size().x {
                for y in 0..v.size().y {
                    assert_eq!(
                        IVec3::new(v.block_x(x), v.block_y(y), v.block_z(z)),
                        v.min_block() + IVec3::new(x, y, z)
                    );
                    seen[v.index_unchecked(x, y, z)] = true;
                }
            }
        }
        assert!(seen.into_iter().all(|hit| hit));
    }

    #[test]
    fn y_is_the_fastest_axis() {
        let v = Volume::dense(IVec3::new(4, 8, 4), IVec3::ZERO);
        assert_eq!(v.index_unchecked(0, 1, 0), 1);
        assert_eq!(v.index_unchecked(1, 0, 0), 8);
        assert_eq!(v.index_unchecked(0, 0, 1), 32);
    }

    #[test]
    fn a_strided_lattice_index_steps_by_the_cell_size() {
        let v = Volume::new(
            IVec3::new(2, 2, 2),
            IVec3::new(-8, -64, 4),
            IVec3::new(4, 8, 4),
        );
        assert_eq!(v.max_block(), IVec3::new(-1, -49, 11));
        assert_eq!(
            IVec3::new(v.block_x(1), v.block_y(1), v.block_z(1)),
            IVec3::new(-4, -56, 8)
        );
        assert_eq!(v.index_unchecked(0, 1, 0), 1);
        assert_eq!(v.index_unchecked(1, 0, 0), 2);
        assert_eq!(v.index_unchecked(0, 0, 1), 4);
    }

    #[test]
    fn point_is_a_single_dense_cell() {
        let v = Volume::point(IVec3::new(5, -3, 9));
        assert_eq!(v.len(), 1);
        assert_eq!(v.size(), IVec3::ONE);
        assert_eq!(v.step_block(), IVec3::ONE);
        assert_eq!(v.min_block(), IVec3::new(5, -3, 9));
        assert_eq!(v.max_block(), IVec3::new(5, -3, 9));
    }

    #[test]
    #[should_panic]
    fn a_zero_size_volume_cannot_exist() {
        Volume::dense(IVec3::new(1, 0, 1), IVec3::ZERO);
    }

    #[test]
    #[should_panic]
    fn a_zero_step_volume_cannot_exist() {
        Volume::new(IVec3::ONE, IVec3::ZERO, IVec3::new(1, 0, 1));
    }
}
