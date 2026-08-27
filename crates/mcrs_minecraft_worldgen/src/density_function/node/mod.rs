use super::*;

mod arith;
mod noise;
mod shape;
mod space;

pub(super) use arith::*;
pub(super) use noise::*;
pub(super) use shape::*;
pub(super) use space::*;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum IndependentDensityFunction {
    Constant(f32),
    OldBlendedNoise(BlendedNoise),
    Noise(Noise),
    ShiftB(ShiftB),
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
            IndependentDensityFunction::ShiftB(x) => x.min_value(),
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
            IndependentDensityFunction::ShiftB(x) => x.max_value(),
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
            IndependentDensityFunction::ShiftB(x) => x.sample(pos),
            IndependentDensityFunction::ClampedYGradient(x) => x.sample(pos),
            IndependentDensityFunction::Gradient(x) => x.sample(pos),
            IndependentDensityFunction::DistanceToPoint(x) => x.sample(pos),
            IndependentDensityFunction::EndOuterIslands(x) => x.sample(pos),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum DependentDensityFunction {
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

#[inline]
pub(super) fn round_to_integer(value: f32, mode: RoundingMode) -> f32 {
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
pub(super) enum DensityFunctionComponent {
    Independent(IndependentDensityFunction),
    Dependent(DependentDensityFunction),
    Interpolated(Interpolated),
}

impl DensityFunctionComponent {
    pub(super) fn as_constant(&self) -> Option<f32> {
        match self {
            DensityFunctionComponent::Independent(x) => match x {
                IndependentDensityFunction::Constant(v) => Some(*v),
                _ => None,
            },
            _ => None,
        }
    }
}

impl DensityFunctionComponent {
    pub(super) fn rewrite_indices(&mut self, redirect: &[usize]) {
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

    pub(super) fn visit_input_indices(&self, f: &mut impl FnMut(usize)) {
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

/// The compiled node arena: everything a fill needs that is not the volume.
#[derive(Clone, Copy)]
pub(super) struct Arena<'a> {
    stack: &'a [DensityFunctionComponent],
}

#[derive(Default)]
pub(super) struct MemberScratch {
    rows: Vec<f32>,
    point: Vec<f32>,
    positions: Vec<IVec3>,
}

impl<'a> Arena<'a> {
    pub(super) fn new(stack: &'a [DensityFunctionComponent]) -> Self {
        Self { stack }
    }

    /// Evaluate the subgraph `members` — topologically ordered, its own root last —
    /// over every position of `volume`.
    pub(super) fn fill_members(self, members: &[u32], volume: &Volume, out: &mut [f32]) {
        self.fill_members_with(members, volume, out, &mut MemberScratch::default());
    }

    /// [`Arena::fill_members`] against caller-owned buffers, for callers that
    /// evaluate the same subgraph many times over.
    pub(super) fn fill_members_with(
        self,
        members: &[u32],
        volume: &Volume,
        out: &mut [f32],
        scratch: &mut MemberScratch,
    ) {
        let n = volume.len();
        let root = *members.last().expect("a subgraph has at least one member") as usize;
        scratch.rows.clear();
        scratch.rows.resize((root + 1) * n, 0.0);
        scratch.point.clear();
        scratch.point.resize(self.stack.len(), 0.0);
        volume.positions_into(&mut scratch.positions);
        for &member in members {
            self.fill_node(
                member as usize,
                volume,
                &scratch.positions,
                &mut scratch.rows,
                &mut scratch.point,
            );
        }
        out.copy_from_slice(&scratch.rows[root * n..root * n + n]);
    }

    pub(super) fn fill_node(
        self,
        i: usize,
        volume: &Volume,
        positions: &[IVec3],
        rows: &mut [f32],
        point: &mut [f32],
    ) {
        let stack = self.stack;
        let n = volume.len();
        let out_base = i * n;
        match &stack[i] {
            DensityFunctionComponent::Independent(f) => {
                let out = &mut rows[out_base..out_base + n];
                if !f.fill_columns(volume, positions, out) {
                    for (p, slot) in out.iter_mut().enumerate() {
                        *slot = f.sample(positions[p]);
                    }
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
                    let (_, out) = rows.split_at_mut(out_base);
                    x.fill(self, volume, &mut out[..n]);
                }
                DependentDensityFunction::FindTopSurface(x) => {
                    let a = x.upper_bound_index * n;
                    let (filled, out) = rows.split_at_mut(out_base);
                    x.fill(self, positions, &filled[a..a + n], &mut out[..n]);
                }
            },
            DensityFunctionComponent::Interpolated(x) => {
                let (filled, out) = rows.split_at_mut(out_base);
                let input = x.input_index * n;
                x.sample_volume(self, volume, &filled[input..input + n], &mut out[..n]);
            }
        }
    }
}
