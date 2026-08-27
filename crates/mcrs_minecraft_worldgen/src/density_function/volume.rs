use super::{
    DensityFunctionComponent, DependentDensityFunction, IndependentDensityFunction, LinearOperation,
    NoiseRouter, WrapperDensityFunction, eval_subgraph,
};
use crate::density_function::DensityFunction;
use crate::density_function::proto::Axis;
use bevy_math::IVec3;

/// A strided box of block positions: `size` samples per axis, starting at
/// `min_block`, spaced `step_block` apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Volume {
    size_x: i32,
    size_y: i32,
    size_z: i32,
    min_block_x: i32,
    min_block_y: i32,
    min_block_z: i32,
    step_block_x: i32,
    step_block_y: i32,
    step_block_z: i32,
}

impl Volume {
    pub fn new(size: IVec3, min_block: IVec3, step_block: IVec3) -> Self {
        assert!(
            size.x > 0 && size.y > 0 && size.z > 0,
            "size must be positive, was: {}x{}x{}",
            size.x,
            size.y,
            size.z
        );
        assert!(
            step_block.x > 0 && step_block.y > 0 && step_block.z > 0,
            "step must be positive, was: {}; {}; {}",
            step_block.x,
            step_block.y,
            step_block.z
        );
        Self {
            size_x: size.x,
            size_y: size.y,
            size_z: size.z,
            min_block_x: min_block.x,
            min_block_y: min_block.y,
            min_block_z: min_block.z,
            step_block_x: step_block.x,
            step_block_y: step_block.y,
            step_block_z: step_block.z,
        }
    }

    pub fn dense(size: IVec3, min_block: IVec3) -> Self {
        Self::new(size, min_block, IVec3::ONE)
    }

    pub fn point(pos: IVec3) -> Self {
        Self::new(IVec3::ONE, pos, IVec3::ONE)
    }

    #[inline]
    pub fn size_x(&self) -> i32 {
        self.size_x
    }

    #[inline]
    pub fn size_y(&self) -> i32 {
        self.size_y
    }

    #[inline]
    pub fn size_z(&self) -> i32 {
        self.size_z
    }

    #[inline]
    pub fn min_block_x(&self) -> i32 {
        self.min_block_x
    }

    #[inline]
    pub fn min_block_y(&self) -> i32 {
        self.min_block_y
    }

    #[inline]
    pub fn min_block_z(&self) -> i32 {
        self.min_block_z
    }

    #[inline]
    pub fn step_block_x(&self) -> i32 {
        self.step_block_x
    }

    #[inline]
    pub fn step_block_y(&self) -> i32 {
        self.step_block_y
    }

    #[inline]
    pub fn step_block_z(&self) -> i32 {
        self.step_block_z
    }

    #[inline]
    pub fn index_unchecked(&self, x: i32, y: i32, z: i32) -> usize {
        (y + (x + z * self.size_x) * self.size_y) as usize
    }

    #[inline]
    pub fn block_x(&self, x: i32) -> i32 {
        self.min_block_x + x * self.step_block_x
    }

    #[inline]
    pub fn block_y(&self, y: i32) -> i32 {
        self.min_block_y + y * self.step_block_y
    }

    #[inline]
    pub fn block_z(&self, z: i32) -> i32 {
        self.min_block_z + z * self.step_block_z
    }

    #[inline]
    pub fn max_block_x(&self) -> i32 {
        self.min_block_x + self.size_x * self.step_block_x - 1
    }

    #[inline]
    pub fn max_block_y(&self) -> i32 {
        self.min_block_y + self.size_y * self.step_block_y - 1
    }

    #[inline]
    pub fn max_block_z(&self) -> i32 {
        self.min_block_z + self.size_z * self.step_block_z - 1
    }

    #[inline]
    pub fn len(&self) -> usize {
        (self.size_x * self.size_y * self.size_z) as usize
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn index_of_block(&self, block_x: i32, block_y: i32, block_z: i32) -> Option<usize> {
        let rx = block_x - self.min_block_x;
        let ry = block_y - self.min_block_y;
        let rz = block_z - self.min_block_z;
        if self.step_block_x == 1 && self.step_block_y == 1 && self.step_block_z == 1 {
            let inside = (0..self.size_x).contains(&rx)
                && (0..self.size_y).contains(&ry)
                && (0..self.size_z).contains(&rz);
            return inside.then(|| self.index_unchecked(rx, ry, rz));
        }
        let on_lattice = |r: i32, size: i32, step: i32| {
            r >= 0 && r < size * step && r.rem_euclid(step) == 0
        };
        (on_lattice(rx, self.size_x, self.step_block_x)
            && on_lattice(ry, self.size_y, self.step_block_y)
            && on_lattice(rz, self.size_z, self.step_block_z))
        .then(|| {
            self.index_unchecked(
                rx / self.step_block_x,
                ry / self.step_block_y,
                rz / self.step_block_z,
            )
        })
    }

    fn positions_into(&self, out: &mut Vec<IVec3>) {
        out.clear();
        out.reserve(self.len());
        for z in 0..self.size_z {
            let bz = self.block_z(z);
            for x in 0..self.size_x {
                let bx = self.block_x(x);
                for y in 0..self.size_y {
                    out.push(IVec3::new(bx, self.block_y(y), bz));
                }
            }
        }
    }
}

/// Reusable buffers for [`NoiseRouter::fill`]: one row per live node plus the
/// register files the per-position opcodes still need.
#[derive(Default)]
pub struct FillScratch {
    rows: Vec<f32>,
    column: Vec<f32>,
    point: Vec<f32>,
    positions: Vec<IVec3>,
}

impl FillScratch {
    pub fn new() -> Self {
        Self::default()
    }
}

impl NoiseRouter {
    /// Evaluate `root` over every position of `volume`, writing `volume.len()`
    /// values into `out` in the volume's own index layout.
    ///
    /// Bit-identical to `sample_root` at each position.
    pub fn fill(&self, root: usize, volume: &Volume, out: &mut [f32], scratch: &mut FillScratch) {
        let n = volume.len();
        assert_eq!(out.len(), n, "output length must match the volume");
        let live = root + 1;

        scratch.rows.clear();
        scratch.rows.resize(live * n, 0.0);
        scratch.column.clear();
        scratch.column.resize(self.scratch_len, 0.0);
        scratch.point.clear();
        scratch.point.resize(self.scratch_len, 0.0);
        volume.positions_into(&mut scratch.positions);

        let column_needed = if root < self.fd_boundary {
            self.column_boundary
        } else {
            live
        };

        let rows = &mut scratch.rows;
        let column = &mut scratch.column;
        let size_y = volume.size_y as usize;
        for z in 0..volume.size_z {
            for x in 0..volume.size_x {
                let y0 = IVec3::new(volume.block_x(x), 0, volume.block_z(z));
                for i in 0..column_needed {
                    let value = self.stack[i].sample_cached(column, &self.stack, y0);
                    column[i] = value;
                }
                let base = volume.index_unchecked(x, 0, z);
                for i in 0..column_needed.min(live) {
                    rows[i * n + base..i * n + base + size_y].fill(column[i]);
                }
            }
        }

        let positions = &scratch.positions;
        let point = &mut scratch.point;
        if root >= self.fd_boundary {
            for i in 0..live {
                if self.per_block[i] {
                    self.fill_node(i, n, positions, rows, point);
                }
            }
        } else if root >= self.column_boundary {
            for i in self.column_boundary..live {
                self.fill_node(i, n, positions, rows, point);
            }
        }

        out.copy_from_slice(&rows[root * n..root * n + n]);
    }

    fn fill_node(
        &self,
        i: usize,
        n: usize,
        positions: &[IVec3],
        rows: &mut [f32],
        point: &mut [f32],
    ) {
        let stack = &self.stack;
        let out_base = i * n;
        match &stack[i] {
            DensityFunctionComponent::Independent(f) => {
                for p in 0..n {
                    rows[out_base + p] = f.sample(positions[p]);
                }
            }
            DensityFunctionComponent::Dependent(f) => match f {
                DependentDensityFunction::Linear(x) => {
                    let a = x.input_index * n;
                    for p in 0..n {
                        let input = rows[a + p];
                        rows[out_base + p] = match x.operation {
                            LinearOperation::Add => input + x.argument,
                            LinearOperation::Multiply => input * x.argument,
                        };
                    }
                }
                DependentDensityFunction::Affine(x) => {
                    let a = x.input_index * n;
                    for p in 0..n {
                        rows[out_base + p] = rows[a + p].mul_add(x.scale, x.offset);
                    }
                }
                DependentDensityFunction::PiecewiseAffine(x) => {
                    let a = x.input_index * n;
                    for p in 0..n {
                        let input = rows[a + p];
                        let scale = if input < 0.0 {
                            x.neg_scale
                        } else {
                            x.pos_scale
                        };
                        rows[out_base + p] = input.mul_add(scale, x.offset);
                    }
                }
                DependentDensityFunction::Slide(x) => {
                    let a = x.input_index * n;
                    for p in 0..n {
                        rows[out_base + p] = x.compute(rows[a + p], positions[p].y as f32);
                    }
                }
                DependentDensityFunction::Unary(x) => {
                    let a = x.input_index * n;
                    for p in 0..n {
                        rows[out_base + p] = x.operation.apply(rows[a + p]);
                    }
                }
                DependentDensityFunction::Binary(x) => {
                    let a = x.input1_index * n;
                    let b = x.input2_index * n;
                    for p in 0..n {
                        rows[out_base + p] = x.operation.apply(rows[a + p], rows[b + p]);
                    }
                }
                DependentDensityFunction::ShiftedNoise(x) => {
                    let (sx, sy, sz) = (
                        x.input_x_index * n,
                        x.input_y_index * n,
                        x.input_z_index * n,
                    );
                    for p in 0..n {
                        let pos = positions[p];
                        rows[out_base + p] = x.sampler.get(
                            pos.x as f64 * x.xz_scale + rows[sx + p] as f64,
                            pos.y as f64 * x.y_scale + rows[sy + p] as f64,
                            pos.z as f64 * x.xz_scale + rows[sz + p] as f64,
                        );
                    }
                }
                DependentDensityFunction::Clamp(x) => {
                    let a = x.input_index * n;
                    for p in 0..n {
                        rows[out_base + p] = rows[a + p].clamp(x.min_value, x.max_value);
                    }
                }
                DependentDensityFunction::RangeChoice(x) => {
                    let a = x.input_index * n;
                    let win = x.when_in_index * n;
                    let wout = x.when_out_index * n;
                    for p in 0..n {
                        let input = rows[a + p];
                        rows[out_base + p] =
                            if input >= x.min_inclusion_value && input < x.max_exclusion_value {
                                rows[win + p]
                            } else {
                                rows[wout + p]
                            };
                    }
                }
                DependentDensityFunction::Lerp(x) => {
                    let al = x.alpha_index * n;
                    let fi = x.first_index * n;
                    let se = x.second_index * n;
                    for p in 0..n {
                        let alpha = rows[al + p];
                        rows[out_base + p] = if alpha == 0.0 {
                            rows[fi + p]
                        } else if alpha == 1.0 {
                            rows[se + p]
                        } else {
                            let first = rows[fi + p];
                            first + alpha * (rows[se + p] - first)
                        };
                    }
                }
                DependentDensityFunction::Spline(x) => {
                    for p in 0..n {
                        for j in 0..i {
                            point[j] = rows[j * n + p];
                        }
                        rows[out_base + p] = x.sample_cached(point, stack, positions[p]);
                    }
                }
                DependentDensityFunction::Slice(x) => {
                    let (_, sub) = point.split_at_mut(stack.len());
                    for p in 0..n {
                        let pos = positions[p];
                        let pinned = match x.axis {
                            Axis::X => IVec3::new(x.coordinate, pos.y, pos.z),
                            Axis::Y => IVec3::new(pos.x, x.coordinate, pos.z),
                            Axis::Z => IVec3::new(pos.x, pos.y, x.coordinate),
                        };
                        rows[out_base + p] = eval_subgraph(&x.input_members, stack, pinned, sub);
                    }
                }
                DependentDensityFunction::FindTopSurface(x) => {
                    let a = x.upper_bound_index * n;
                    let (_, sub) = point.split_at_mut(stack.len());
                    for p in 0..n {
                        let top_y = (rows[a + p] / x.cell_height).floor() * x.cell_height;
                        rows[out_base + p] = if top_y <= x.lower_bound {
                            x.lower_bound
                        } else {
                            let mut current_y = top_y;
                            loop {
                                let probe =
                                    IVec3::new(positions[p].x, current_y as i32, positions[p].z);
                                let density =
                                    eval_subgraph(&x.density_members, stack, probe, sub);
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
                WrapperDensityFunction::Cache(x) => {
                    let a = x.input_index * n;
                    for p in 0..n {
                        rows[out_base + p] = rows[a + p];
                    }
                }
                WrapperDensityFunction::Interpolated(x) => {
                    let a = x.input_index * n;
                    let (_, sub) = point.split_at_mut(stack.len());
                    for p in 0..n {
                        let pos = positions[p];
                        rows[out_base + p] = if x.is_cell_corner(pos) {
                            rows[a + p]
                        } else {
                            x.interpolate(stack, pos, sub)
                        };
                    }
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_of_block_round_trips_the_dense_layout() {
        let v = Volume::dense(IVec3::new(3, 5, 2), IVec3::new(-7, 12, 40));
        assert_eq!(v.len(), 30);
        assert_eq!(v.max_block_x(), -5);
        assert_eq!(v.max_block_y(), 16);
        assert_eq!(v.max_block_z(), 41);
        for z in 0..v.size_z() {
            for x in 0..v.size_x() {
                for y in 0..v.size_y() {
                    let i = v.index_unchecked(x, y, z);
                    assert_eq!(
                        v.index_of_block(v.block_x(x), v.block_y(y), v.block_z(z)),
                        Some(i)
                    );
                }
            }
        }
        assert_eq!(v.index_of_block(-8, 12, 40), None);
        assert_eq!(v.index_of_block(-7, 11, 40), None);
        assert_eq!(v.index_of_block(-7, 12, 42), None);
    }

    #[test]
    fn y_is_the_fastest_axis() {
        let v = Volume::dense(IVec3::new(4, 8, 4), IVec3::ZERO);
        assert_eq!(v.index_unchecked(0, 1, 0), 1);
        assert_eq!(v.index_unchecked(1, 0, 0), 8);
        assert_eq!(v.index_unchecked(0, 0, 1), 32);
    }

    #[test]
    fn index_of_block_rejects_off_lattice_positions() {
        let v = Volume::new(
            IVec3::new(2, 2, 2),
            IVec3::new(-8, -64, 4),
            IVec3::new(4, 8, 4),
        );
        assert_eq!(v.index_of_block(-8, -64, 4), Some(0));
        assert_eq!(v.index_of_block(-8, -56, 4), Some(1));
        assert_eq!(v.index_of_block(-4, -64, 4), Some(2));
        assert_eq!(v.index_of_block(-8, -64, 8), Some(4));
        assert_eq!(v.index_of_block(-8, -64, 7), None);
        assert_eq!(v.index_of_block(-7, -64, 4), None);
        assert_eq!(v.index_of_block(-8, -60, 4), None);
        assert_eq!(v.index_of_block(0, -64, 4), None);
        assert_eq!(v.index_of_block(-12, -64, 4), None);
    }

    #[test]
    fn point_is_a_single_dense_cell() {
        let v = Volume::point(IVec3::new(5, -3, 9));
        assert_eq!(v.len(), 1);
        assert_eq!(v.index_of_block(5, -3, 9), Some(0));
        assert_eq!(v.index_of_block(5, -2, 9), None);
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
