use super::*;

/// What a sampler reads while it fills a volume: the arena, the volume with its
/// materialised positions, and the rows every node below it already wrote.
#[derive(Clone, Copy)]
pub(crate) struct Fill<'a> {
    pub(crate) arena: Arena<'a>,
    pub(crate) volume: &'a Volume,
    pub(crate) positions: &'a [IVec3],
    filled: &'a [f32],
}

impl<'a> Fill<'a> {
    #[inline]
    pub(crate) fn len(self) -> usize {
        self.positions.len()
    }

    /// The row node `index` wrote earlier in this pass.
    #[inline]
    pub(crate) fn row(self, index: usize) -> &'a [f32] {
        let n = self.len();
        &self.filled[index * n..index * n + n]
    }

    /// How many nodes precede the one being filled.
    #[inline]
    pub(crate) fn depth(self) -> usize {
        self.filled.len() / self.len()
    }
}

/// The compiled form of a density function: the thing that actually reads noise.
pub(crate) trait DensitySampler {
    fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]);
}

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

impl IndependentDensityFunction {}

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
    ConstMin(ConstMin),
    ConstMax(ConstMax),
    ConstSub(ConstSub),
    ConstDiv(ConstDiv),
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
            DependentDensityFunction::ConstMin(x) => x.min_value(),
            DependentDensityFunction::ConstMax(x) => x.min_value(),
            DependentDensityFunction::ConstSub(x) => x.min_value(),
            DependentDensityFunction::ConstDiv(x) => x.min_value(),
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
            DependentDensityFunction::ConstMin(x) => x.max_value(),
            DependentDensityFunction::ConstMax(x) => x.max_value(),
            DependentDensityFunction::ConstSub(x) => x.max_value(),
            DependentDensityFunction::ConstDiv(x) => x.max_value(),
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
                DependentDensityFunction::ConstMin(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::ConstMax(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::ConstSub(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::ConstDiv(x) => {
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
                DependentDensityFunction::ConstMin(x) => f(x.input_index),
                DependentDensityFunction::ConstMax(x) => f(x.input_index),
                DependentDensityFunction::ConstSub(x) => f(x.input_index),
                DependentDensityFunction::ConstDiv(x) => f(x.input_index),
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
pub(crate) struct Arena<'a> {
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
    ) {
        let n = volume.len();
        let (filled, out) = rows.split_at_mut(i * n);
        let ctx = Fill {
            arena: self,
            volume,
            positions,
            filled,
        };
        self.stack[i].sample_volume(ctx, &mut out[..n]);
    }
}

impl DensitySampler for IndependentDensityFunction {
    fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
        match self {
            IndependentDensityFunction::Noise(x) => x.sample_volume(ctx, out),
            IndependentDensityFunction::ShiftB(x) => x.sample_volume(ctx, out),
            _ => {
                for (p, slot) in out.iter_mut().enumerate() {
                    *slot = self.sample(ctx.positions[p]);
                }
            }
        }
    }
}

impl DensitySampler for DependentDensityFunction {
    fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
        match self {
            DependentDensityFunction::Linear(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::Affine(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::PiecewiseAffine(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::Slide(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::Unary(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::Binary(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::ConstMin(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::ConstMax(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::ConstSub(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::ConstDiv(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::ShiftedNoise(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::Clamp(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::RangeChoice(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::Lerp(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::Spline(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::Slice(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::FindTopSurface(x) => x.sample_volume(ctx, out),
        }
    }
}

impl DensitySampler for DensityFunctionComponent {
    fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
        match self {
            DensityFunctionComponent::Independent(x) => x.sample_volume(ctx, out),
            DensityFunctionComponent::Dependent(x) => x.sample_volume(ctx, out),
            DensityFunctionComponent::Interpolated(x) => x.sample_volume(ctx, out),
        }
    }
}
