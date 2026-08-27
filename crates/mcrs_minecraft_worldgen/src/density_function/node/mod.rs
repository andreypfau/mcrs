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

impl IndependentDensityFunction {
    fn range(&self) -> Interval {
        match self {
            IndependentDensityFunction::Constant(x) => Interval::exact(*x),
            IndependentDensityFunction::OldBlendedNoise(x) => x.range(),
            IndependentDensityFunction::Noise(x) => x.range(),
            IndependentDensityFunction::ShiftB(x) => x.range(),
            IndependentDensityFunction::ClampedYGradient(x) => x.range(),
            IndependentDensityFunction::Gradient(x) => x.range(),
            IndependentDensityFunction::DistanceToPoint(x) => x.range(),
            IndependentDensityFunction::EndOuterIslands(x) => x.range(),
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
    Affine(Affine),
    PiecewiseAffine(PiecewiseAffine),
    Slide(Slide),
    ConstMin(ConstMin),
    ConstMax(ConstMax),
    ConstSub(ConstSub),
    ConstDiv(ConstDiv),
    Abs(Abs),
    Square(Square),
    Cube(Cube),
    Negate(Negate),
    Reciprocal(Reciprocal),
    Sqrt(Sqrt),
    Log(Log),
    Sign(Sign),
    Squeeze(Squeeze),
    LeakyReLU(LeakyReLU),
    IntegerMultipleRound(IntegerMultipleRound),
    ConstExponentPow(ConstExponentPow),
    ConstBasePow(ConstBasePow),
    Add(Add),
    Sub(Sub),
    Mul(Mul),
    Div(Div),
    Min(Min),
    Max(Max),
    Pow(Pow),
    Round(Round),
    ShiftedNoise(ShiftedNoise),
    Clamp(Clamp),
    RangeChoice(RangeChoice),
    Spline(Spline),
    FindTopSurface(FindTopSurface),
    Lerp(Lerp),
    Slice(Slice),
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
pub(super) enum Sampler {
    Independent(IndependentDensityFunction),
    Dependent(DependentDensityFunction),
    Interpolated(Interpolated),
}

/// A stack entry: what it computes, and the bounds on what it can compute.
/// Nothing on the sampler side ever reads the range.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct DensityFunctionComponent {
    pub(super) range: Interval,
    pub(super) sampler: Sampler,
}

impl DensityFunctionComponent {
    pub(super) fn new(range: Interval, sampler: Sampler) -> Self {
        Self { range, sampler }
    }

    pub(super) fn independent(function: IndependentDensityFunction) -> Self {
        Self::new(function.range(), Sampler::Independent(function))
    }

    pub(super) fn dependent(range: Interval, function: DependentDensityFunction) -> Self {
        Self::new(range, Sampler::Dependent(function))
    }

    pub(super) fn constant(value: f32) -> Self {
        Self::independent(IndependentDensityFunction::Constant(value))
    }

    pub(super) fn as_constant(&self) -> Option<f32> {
        match &self.sampler {
            Sampler::Independent(IndependentDensityFunction::Constant(v)) => Some(*v),
            _ => None,
        }
    }

    pub(super) fn rewrite_indices(&mut self, redirect: &[usize]) {
        match &mut self.sampler {
            Sampler::Independent(_) => {}
            Sampler::Dependent(dep) => match dep {
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
                DependentDensityFunction::Abs(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::Square(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::Cube(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::Negate(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::Reciprocal(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::Sqrt(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::Log(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::Sign(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::Squeeze(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::LeakyReLU(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::IntegerMultipleRound(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::ConstExponentPow(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::ConstBasePow(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::Add(x) => {
                    x.input1_index = redirect[x.input1_index];
                    x.input2_index = redirect[x.input2_index];
                }
                DependentDensityFunction::Sub(x) => {
                    x.input1_index = redirect[x.input1_index];
                    x.input2_index = redirect[x.input2_index];
                }
                DependentDensityFunction::Mul(x) => {
                    x.input1_index = redirect[x.input1_index];
                    x.input2_index = redirect[x.input2_index];
                }
                DependentDensityFunction::Div(x) => {
                    x.input1_index = redirect[x.input1_index];
                    x.input2_index = redirect[x.input2_index];
                }
                DependentDensityFunction::Min(x) => {
                    x.input1_index = redirect[x.input1_index];
                    x.input2_index = redirect[x.input2_index];
                }
                DependentDensityFunction::Max(x) => {
                    x.input1_index = redirect[x.input1_index];
                    x.input2_index = redirect[x.input2_index];
                }
                DependentDensityFunction::Pow(x) => {
                    x.input1_index = redirect[x.input1_index];
                    x.input2_index = redirect[x.input2_index];
                }
                DependentDensityFunction::Round(x) => {
                    x.input1_index = redirect[x.input1_index];
                    x.input2_index = redirect[x.input2_index];
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
            Sampler::Interpolated(x) => {
                x.input_index = redirect[x.input_index];
            }
        }
    }

    pub(super) fn visit_input_indices(&self, f: &mut impl FnMut(usize)) {
        match &self.sampler {
            Sampler::Independent(_) => {}
            Sampler::Dependent(dep) => match dep {
                DependentDensityFunction::ConstMin(x) => f(x.input_index),
                DependentDensityFunction::ConstMax(x) => f(x.input_index),
                DependentDensityFunction::ConstSub(x) => f(x.input_index),
                DependentDensityFunction::ConstDiv(x) => f(x.input_index),
                DependentDensityFunction::Abs(x) => f(x.input_index),
                DependentDensityFunction::Square(x) => f(x.input_index),
                DependentDensityFunction::Cube(x) => f(x.input_index),
                DependentDensityFunction::Negate(x) => f(x.input_index),
                DependentDensityFunction::Reciprocal(x) => f(x.input_index),
                DependentDensityFunction::Sqrt(x) => f(x.input_index),
                DependentDensityFunction::Log(x) => f(x.input_index),
                DependentDensityFunction::Sign(x) => f(x.input_index),
                DependentDensityFunction::Squeeze(x) => f(x.input_index),
                DependentDensityFunction::LeakyReLU(x) => f(x.input_index),
                DependentDensityFunction::IntegerMultipleRound(x) => f(x.input_index),
                DependentDensityFunction::ConstExponentPow(x) => f(x.input_index),
                DependentDensityFunction::ConstBasePow(x) => f(x.input_index),
                DependentDensityFunction::Add(x) => {
                    f(x.input1_index);
                    f(x.input2_index);
                }
                DependentDensityFunction::Sub(x) => {
                    f(x.input1_index);
                    f(x.input2_index);
                }
                DependentDensityFunction::Mul(x) => {
                    f(x.input1_index);
                    f(x.input2_index);
                }
                DependentDensityFunction::Div(x) => {
                    f(x.input1_index);
                    f(x.input2_index);
                }
                DependentDensityFunction::Min(x) => {
                    f(x.input1_index);
                    f(x.input2_index);
                }
                DependentDensityFunction::Max(x) => {
                    f(x.input1_index);
                    f(x.input2_index);
                }
                DependentDensityFunction::Pow(x) => {
                    f(x.input1_index);
                    f(x.input2_index);
                }
                DependentDensityFunction::Round(x) => {
                    f(x.input1_index);
                    f(x.input2_index);
                }
                DependentDensityFunction::Affine(x) => f(x.input_index),
                DependentDensityFunction::PiecewiseAffine(x) => f(x.input_index),
                DependentDensityFunction::Slide(x) => f(x.input_index),
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
            Sampler::Interpolated(x) => f(x.input_index),
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
            DependentDensityFunction::Affine(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::PiecewiseAffine(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::Slide(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::ConstMin(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::ConstMax(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::ConstSub(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::ConstDiv(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::Abs(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::Square(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::Cube(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::Negate(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::Reciprocal(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::Sqrt(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::Log(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::Sign(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::Squeeze(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::LeakyReLU(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::IntegerMultipleRound(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::ConstExponentPow(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::ConstBasePow(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::Add(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::Sub(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::Mul(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::Div(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::Min(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::Max(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::Pow(x) => x.sample_volume(ctx, out),
            DependentDensityFunction::Round(x) => x.sample_volume(ctx, out),
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
        match &self.sampler {
            Sampler::Independent(x) => x.sample_volume(ctx, out),
            Sampler::Dependent(x) => x.sample_volume(ctx, out),
            Sampler::Interpolated(x) => x.sample_volume(ctx, out),
        }
    }
}
