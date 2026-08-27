use super::{
    DensityFunctionComponent, DependentDensityFunction, IndependentDensityFunction, Interpolated,
    LinearOperation, NoiseRouter, Slice, WrapperDensityFunction, branch_schedule::Step,
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
        let on_lattice =
            |r: i32, size: i32, step: i32| r >= 0 && r < size * step && r.rem_euclid(step) == 0;
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

/// Reusable buffers for [`NoiseRouter::sample_volume`]: one row per live node
/// over the volume, the same over the volume's columns, and the register file
/// the spline opcode reads its inputs from.
#[derive(Default)]
pub struct FillScratch {
    rows: Vec<f32>,
    column_rows: Vec<f32>,
    column_positions: Vec<IVec3>,
    point: Vec<f32>,
    positions: Vec<IVec3>,
    needed: Vec<bool>,
}

impl FillScratch {
    pub fn new() -> Self {
        Self::default()
    }
}

impl NoiseRouter {
    /// Evaluate `root` at one position.
    pub fn sample_value(&self, root: usize, pos: IVec3, scratch: &mut FillScratch) -> f32 {
        let mut out = [0.0f32];
        self.sample_volume(root, &Volume::point(pos), &mut out, scratch);
        out[0]
    }

    /// Evaluate `root` over every position of `volume`, writing `volume.len()`
    /// values into `out` in the volume's own index layout.
    pub fn sample_volume(
        &self,
        root: usize,
        volume: &Volume,
        out: &mut [f32],
        scratch: &mut FillScratch,
    ) {
        self.sample_volume_roots(&[root], volume, out, scratch);
    }

    /// Evaluate several roots over `volume` in one pass, writing one
    /// `volume.len()`-long row per root into `out`, in the order given.
    pub fn sample_volume_roots(
        &self,
        roots: &[usize],
        volume: &Volume,
        out: &mut [f32],
        scratch: &mut FillScratch,
    ) {
        let n = volume.len();
        assert_eq!(
            out.len(),
            roots.len() * n,
            "output length must match the volume"
        );
        let live = roots.iter().copied().max().expect("at least one root") + 1;

        // Resized, never cleared: a row is read only after this pass writes it,
        // so re-zeroing the arena is a memset the size of the whole volume.
        scratch.rows.resize(live * n, 0.0);
        scratch.point.resize(self.scratch_len, 0.0);
        volume.positions_into(&mut scratch.positions);

        let column_end = if live <= self.fd_boundary {
            self.column_boundary.min(live)
        } else {
            live
        };

        let needed = &mut scratch.needed;
        needed.clear();
        needed.resize(live, false);
        for &root in roots {
            needed[root] = true;
        }
        for i in (0..live).rev() {
            if !needed[i] {
                continue;
            }
            // An off-lattice `interpolated` refills its own input subtree over the
            // cell lattice, so evaluating that subtree here would be dead work.
            // Only above `column_end`, though: the column pass reads the input row
            // straight back out whenever the position is a cell corner.
            if i >= column_end
                && let DensityFunctionComponent::Wrapper(WrapperDensityFunction::Interpolated(x)) =
                    &self.stack[i]
                && !x.is_lattice_volume(volume)
            {
                continue;
            }
            self.stack[i].visit_input_indices(&mut |dep| {
                if dep < live {
                    needed[dep] = true;
                }
            });
        }

        let column_volume = Volume::new(
            IVec3::new(volume.size_x(), 1, volume.size_z()),
            IVec3::new(volume.min_block_x(), 0, volume.min_block_z()),
            IVec3::new(volume.step_block_x(), 1, volume.step_block_z()),
        );
        let columns = column_volume.len();
        scratch.column_rows.resize(column_end * columns, 0.0);
        column_volume.positions_into(&mut scratch.column_positions);

        let rows = &mut scratch.rows;
        let column_rows = &mut scratch.column_rows;
        let point = &mut scratch.point;
        let size_y = volume.size_y() as usize;
        for i in 0..column_end {
            if !needed[i] {
                continue;
            }
            fill_node(
                &self.stack,
                self.scratch_len,
                i,
                &column_volume,
                &scratch.column_positions,
                column_rows,
                point,
            );
            for c in 0..columns {
                let base = i * n + c * size_y;
                rows[base..base + size_y].fill(column_rows[i * columns + c]);
            }
        }

        let positions = &scratch.positions;
        if live > self.fd_boundary {
            for i in 0..live {
                if self.per_block[i] && needed[i] {
                    fill_node(
                        &self.stack,
                        self.scratch_len,
                        i,
                        volume,
                        positions,
                        rows,
                        point,
                    );
                }
            }
        } else if roots.iter().all(|r| self.zone_b_roots.contains(r)) {
            self.fill_zone_b_scheduled(volume, positions, rows, point, needed);
        } else {
            for i in self.column_boundary..live {
                if needed[i] {
                    fill_node(
                        &self.stack,
                        self.scratch_len,
                        i,
                        volume,
                        positions,
                        rows,
                        point,
                    );
                }
            }
        }

        for (k, &root) in roots.iter().enumerate() {
            out[k * n..k * n + n].copy_from_slice(&rows[root * n..root * n + n]);
        }
    }

    /// Zone B over the whole volume, jumping over every arm-exclusive run no
    /// position in the volume can select. The schedule only preserves the Zone B
    /// roots, so it may not drive a fill of anything else.
    fn fill_zone_b_scheduled(
        &self,
        volume: &Volume,
        positions: &[IVec3],
        rows: &mut [f32],
        point: &mut [f32],
        needed: &[bool],
    ) {
        let n = volume.len();
        let sched = &self.zone_b_schedule;
        let mut s = 0usize;
        while s < sched.steps.len() {
            match sched.steps[s] {
                Step::Eval { start, end } => {
                    for &i in &sched.order[start as usize..end as usize] {
                        if i < needed.len() && needed[i] {
                            fill_node(
                                &self.stack,
                                self.scratch_len,
                                i,
                                volume,
                                positions,
                                rows,
                                point,
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
                    let input = input as usize;
                    let reachable = input >= needed.len()
                        || !needed[input]
                        || rows[input * n..input * n + n]
                            .iter()
                            .any(|&v| (v >= min_inclusive && v < max_exclusive) == want_in);
                    s = if reachable { s + 1 } else { unguard as usize };
                }
                Step::Unguard => s += 1,
            }
        }
        #[cfg(debug_assertions)]
        self.verify_fill_zone_b(volume, positions, rows, point, needed);
    }

    /// Re-evaluate the skipped runs and check nothing the caller reads moved.
    #[cfg(debug_assertions)]
    fn verify_fill_zone_b(
        &self,
        volume: &Volume,
        positions: &[IVec3],
        rows: &mut [f32],
        point: &mut [f32],
        needed: &[bool],
    ) {
        let n = volume.len();
        let live = needed.len();
        let roots = || self.zone_b_roots.iter().copied().filter(|&r| r < live);
        let guarded: Vec<f32> = roots()
            .flat_map(|r| rows[r * n..r * n + n].to_vec())
            .collect();
        for i in self.column_boundary..=self.final_density_index {
            if i < live && needed[i] {
                fill_node(
                    &self.stack,
                    self.scratch_len,
                    i,
                    volume,
                    positions,
                    rows,
                    point,
                );
            }
        }
        for (k, root) in roots().enumerate() {
            for p in 0..n {
                assert_eq!(
                    guarded[k * n + p].to_bits(),
                    rows[root * n + p].to_bits(),
                    "branch skip changed node {root} at {:?}",
                    positions[p]
                );
            }
        }
    }
}

/// Evaluate the subgraph `members` — topologically ordered, its own root last —
/// over every position of `volume`.
pub(super) fn fill_members(
    stack: &[DensityFunctionComponent],
    scratch_len: usize,
    members: &[u32],
    volume: &Volume,
    out: &mut [f32],
) {
    let n = volume.len();
    let root = *members.last().expect("a subgraph has at least one member") as usize;
    let mut rows = vec![0.0f32; (root + 1) * n];
    let mut point = vec![0.0f32; scratch_len];
    let mut positions = Vec::new();
    volume.positions_into(&mut positions);
    for &member in members {
        fill_node(
            stack,
            scratch_len,
            member as usize,
            volume,
            &positions,
            &mut rows,
            &mut point,
        );
    }
    out.copy_from_slice(&rows[root * n..root * n + n]);
}

pub(super) fn fill_node(
    stack: &[DensityFunctionComponent],
    scratch_len: usize,
    i: usize,
    volume: &Volume,
    positions: &[IVec3],
    rows: &mut [f32],
    point: &mut [f32],
) {
    let n = volume.len();
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
                    rows[out_base + p] = x.sample(point);
                }
            }
            DependentDensityFunction::Slice(x) => {
                let pinned = x.pinned_volume(volume);
                if &pinned == volume {
                    let (_, out) = rows.split_at_mut(out_base);
                    fill_members(stack, scratch_len, &x.input_members, volume, &mut out[..n]);
                } else {
                    let mut sliced = vec![0.0f32; pinned.len()];
                    fill_members(stack, scratch_len, &x.input_members, &pinned, &mut sliced);
                    for z in 0..volume.size_z() {
                        for vx in 0..volume.size_x() {
                            for y in 0..volume.size_y() {
                                let source = match x.axis {
                                    Axis::X => pinned.index_unchecked(0, y, z),
                                    Axis::Y => pinned.index_unchecked(vx, 0, z),
                                    Axis::Z => pinned.index_unchecked(vx, y, 0),
                                };
                                rows[out_base + volume.index_unchecked(vx, y, z)] = sliced[source];
                            }
                        }
                    }
                }
            }
            DependentDensityFunction::FindTopSurface(x) => {
                let a = x.upper_bound_index * n;
                let mut probed = [0.0f32];
                for p in 0..n {
                    let top_y = (rows[a + p] / x.cell_height).floor() * x.cell_height;
                    rows[out_base + p] = if top_y <= x.lower_bound {
                        x.lower_bound
                    } else {
                        let mut current_y = top_y;
                        loop {
                            let probe = Volume::point(IVec3::new(
                                positions[p].x,
                                current_y as i32,
                                positions[p].z,
                            ));
                            fill_members(
                                stack,
                                scratch_len,
                                &x.density_members,
                                &probe,
                                &mut probed,
                            );
                            if probed[0] > 0.0 || current_y <= x.lower_bound {
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
                if x.is_lattice_volume(volume) {
                    rows.copy_within(x.input_index * n..x.input_index * n + n, out_base);
                } else {
                    let (_, out) = rows.split_at_mut(out_base);
                    x.sample_volume(stack, scratch_len, volume, &mut out[..n]);
                }
            }
        },
    }
}

impl Slice {
    /// `volume` with this node's axis collapsed onto its pinned coordinate.
    fn pinned_volume(&self, volume: &Volume) -> Volume {
        let mut size = IVec3::new(volume.size_x(), volume.size_y(), volume.size_z());
        let mut min = IVec3::new(
            volume.min_block_x(),
            volume.min_block_y(),
            volume.min_block_z(),
        );
        let step = IVec3::new(
            volume.step_block_x(),
            volume.step_block_y(),
            volume.step_block_z(),
        );
        let axis = match self.axis {
            Axis::X => 0,
            Axis::Y => 1,
            Axis::Z => 2,
        };
        size[axis] = 1;
        min[axis] = self.coordinate;
        Volume::new(size, min, step)
    }
}

impl Interpolated {
    /// Whether `volume` already samples this node's cell lattice, so the input
    /// can be passed straight through with no interpolation.
    pub(super) fn is_lattice_volume(&self, volume: &Volume) -> bool {
        let xz = self.cell_size_xz as i32;
        let y = self.cell_size_y as i32;
        (volume.step_block_x() == xz || volume.size_x() == 1)
            && (volume.step_block_y() == y || volume.size_y() == 1)
            && (volume.step_block_z() == xz || volume.size_z() == 1)
            && volume.min_block_x().rem_euclid(xz) == 0
            && volume.min_block_y().rem_euclid(y) == 0
            && volume.min_block_z().rem_euclid(xz) == 0
    }

    pub(super) fn sample_volume(
        &self,
        stack: &[DensityFunctionComponent],
        scratch_len: usize,
        volume: &Volume,
        out: &mut [f32],
    ) {
        if self.is_lattice_volume(volume) {
            fill_members(stack, scratch_len, &self.input_members, volume, out);
        } else if volume.len() == 1 {
            // A single position combines the eight corners exactly, where a
            // volume accumulates along Y. Vanilla splits the same two ways, and
            // the block values a chunk fill produces come from the second.
            out[0] = self.sample_point(
                stack,
                scratch_len,
                IVec3::new(
                    volume.min_block_x(),
                    volume.min_block_y(),
                    volume.min_block_z(),
                ),
            );
        } else if volume.step_block_x() == 1
            && volume.step_block_y() == 1
            && volume.step_block_z() == 1
        {
            self.fill_block_step(stack, scratch_len, volume, out);
        } else {
            let block_volume = Volume::dense(
                IVec3::new(
                    volume.size_x() * volume.step_block_x(),
                    volume.size_y() * volume.step_block_y(),
                    volume.size_z() * volume.step_block_z(),
                ),
                IVec3::new(
                    volume.min_block_x(),
                    volume.min_block_y(),
                    volume.min_block_z(),
                ),
            );
            let mut block = vec![0.0f32; block_volume.len()];
            self.fill_block_step(stack, scratch_len, &block_volume, &mut block);
            for z in 0..volume.size_z() {
                for x in 0..volume.size_x() {
                    for y in 0..volume.size_y() {
                        out[volume.index_unchecked(x, y, z)] = block[block_volume.index_unchecked(
                            x * volume.step_block_x(),
                            y * volume.step_block_y(),
                            z * volume.step_block_z(),
                        )];
                    }
                }
            }
        }
    }

    fn sample_point(
        &self,
        stack: &[DensityFunctionComponent],
        scratch_len: usize,
        pos: IVec3,
    ) -> f32 {
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
        fill_members(stack, scratch_len, &self.input_members, &cell, &mut corners);

        let alpha_x = x_in_cell as f32 / size_xz as f32;
        let alpha_y = y_in_cell as f32 / size_y as f32;
        let alpha_z = z_in_cell as f32 / size_xz as f32;
        let at = |x: i32, y: i32, z: i32| corners[cell.index_unchecked(x, y, z)];
        let along_x = |y: i32, z: i32| lerp(alpha_x, at(0, y, z), at(1, y, z));
        let along_xy = |z: i32| lerp(alpha_y, along_x(0, z), along_x(1, z));
        lerp(alpha_z, along_xy(0), along_xy(1))
    }

    fn fill_block_step(
        &self,
        stack: &[DensityFunctionComponent],
        scratch_len: usize,
        volume: &Volume,
        out: &mut [f32],
    ) {
        let xz = self.cell_size_xz as i32;
        let sy = self.cell_size_y as i32;
        let min_cell_x = volume.min_block_x().div_euclid(xz);
        let min_cell_y = volume.min_block_y().div_euclid(sy);
        let min_cell_z = volume.min_block_z().div_euclid(xz);
        let cell_count_x = volume.max_block_x().div_euclid(xz) - min_cell_x + 1;
        let cell_count_y = volume.max_block_y().div_euclid(sy) - min_cell_y + 1;
        let cell_count_z = volume.max_block_z().div_euclid(xz) - min_cell_z + 1;
        let cell_volume = Volume::new(
            IVec3::new(
                cell_count_x + i32::from(volume.max_block_x().rem_euclid(xz) != 0),
                cell_count_y + i32::from(volume.max_block_y().rem_euclid(sy) != 0),
                cell_count_z + i32::from(volume.max_block_z().rem_euclid(xz) != 0),
            ),
            IVec3::new(min_cell_x * xz, min_cell_y * sy, min_cell_z * xz),
            IVec3::new(xz, sy, xz),
        );

        let mut cell = vec![0.0f32; cell_volume.len()];
        fill_members(
            stack,
            scratch_len,
            &self.input_members,
            &cell_volume,
            &mut cell,
        );

        for cell_z in 0..cell_count_z {
            let next_cell_z = (cell_z + 1).min(cell_volume.size_z() - 1);
            for cell_x in 0..cell_count_x {
                let next_cell_x = (cell_x + 1).min(cell_volume.size_x() - 1);
                let mut v000 = cell[cell_volume.index_unchecked(cell_x, 0, cell_z)];
                let mut v100 = cell[cell_volume.index_unchecked(next_cell_x, 0, cell_z)];
                let mut v001 = cell[cell_volume.index_unchecked(cell_x, 0, next_cell_z)];
                let mut v101 = cell[cell_volume.index_unchecked(next_cell_x, 0, next_cell_z)];
                for cell_y in 0..cell_count_y {
                    let next_cell_y = (cell_y + 1).min(cell_volume.size_y() - 1);
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
        let cell_output_x = cell_volume.block_x(cell.x) - output_volume.min_block_x();
        let cell_output_y = cell_volume.block_y(cell.y) - output_volume.min_block_y();
        let cell_output_z = cell_volume.block_z(cell.z) - output_volume.min_block_z();
        let x0 = 0.max(-cell_output_x);
        let y0 = 0.max(-cell_output_y);
        let z0 = 0.max(-cell_output_z);
        let x1 = (self.cell_size_xz as i32).min(output_volume.size_x() - cell_output_x) - 1;
        let y1 = (self.cell_size_y as i32).min(output_volume.size_y() - cell_output_y) - 1;
        let z1 = (self.cell_size_xz as i32).min(output_volume.size_z() - cell_output_z) - 1;

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

#[inline]
fn lerp(alpha: f32, p0: f32, p1: f32) -> f32 {
    p0 + alpha * (p1 - p0)
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
