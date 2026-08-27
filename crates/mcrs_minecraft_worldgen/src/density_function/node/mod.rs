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
    fn sample_value(&self, ctx: Fill<'_>, index: usize) -> f32;

    fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
        for (index, slot) in out.iter_mut().enumerate() {
            *slot = self.sample_value(ctx, index);
        }
    }
}

/// A sampler that reads nothing but the position, so its bounds follow from its
/// own parameters with no input interval to propagate.
pub(crate) trait IndependentSampler: DensitySampler {
    fn range(&self) -> Interval;

    fn sample(&self, pos: IVec3) -> f32;
}

/// Fills each column of `volume` with one value, for a sampler that does not
/// vary along Y.
fn fill_columns(sampler: &impl IndependentSampler, ctx: Fill<'_>, out: &mut [f32]) {
    let height = ctx.volume.size().y as usize;
    for (column, slots) in out.chunks_mut(height).enumerate() {
        slots.fill(sampler.sample(ctx.positions[column * height]));
    }
}

/// Fills every column of `volume` with the same values, for a sampler that
/// varies along Y alone.
fn fill_shared_column(sampler: &impl IndependentSampler, ctx: Fill<'_>, out: &mut [f32]) {
    let height = ctx.volume.size().y as usize;
    let (first, rest) = out.split_at_mut(height);
    for (slot, pos) in first.iter_mut().zip(ctx.positions) {
        *slot = sampler.sample(*pos);
    }
    for column in rest.chunks_mut(height) {
        column.copy_from_slice(first);
    }
}

mod arith;
mod noise;
mod shape;
mod space;

pub(super) use arith::*;
pub(super) use noise::*;
pub(super) use shape::*;
pub(super) use space::*;

/// Declares the leaf samplers and every dispatch over them at once, so a leaf
/// cannot be added to the enum and forgotten in a match.
macro_rules! independent_density_functions {
    ($($variant:ident($sampler:ident)),* $(,)?) => {
        #[derive(Clone, Debug, PartialEq)]
        pub(super) enum IndependentDensityFunction {
            $($variant($sampler),)*
        }

        impl DensitySampler for IndependentDensityFunction {
            fn sample_value(&self, ctx: Fill<'_>, index: usize) -> f32 {
                match self {
                    $(Self::$variant(x) => x.sample_value(ctx, index),)*
                }
            }

            fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
                match self {
                    $(Self::$variant(x) => x.sample_volume(ctx, out),)*
                }
            }
        }

        impl IndependentSampler for IndependentDensityFunction {
            fn range(&self) -> Interval {
                match self {
                    $(Self::$variant(x) => x.range(),)*
                }
            }

            fn sample(&self, pos: IVec3) -> f32 {
                match self {
                    $(Self::$variant(x) => x.sample(pos),)*
                }
            }
        }
    };
}

/// Declares the operations, the stack indices each one reads, and every
/// dispatch over them.
///
/// The edge list decides which entries a root still needs, so an operation
/// that declares one edge short has a live input marked dead and reads a row
/// some earlier pass left behind. Declaring the edges once is what keeps that
/// from being an oversight per traversal.
macro_rules! dependent_density_functions {
    (
        edges { $($variant:ident($sampler:ident) { $($edge:ident),+ $(,)? }),* $(,)? }
        owns_subtree { $($sub:ident($sub_sampler:ident)),* $(,)? }
    ) => {
        #[derive(Clone, Debug, PartialEq)]
        pub(super) enum DependentDensityFunction {
            $($variant($sampler),)*
            $($sub($sub_sampler),)*
        }

        impl DependentDensityFunction {
            fn rewrite_indices(&mut self, redirect: &[usize]) {
                match self {
                    $(Self::$variant(x) => { $(x.$edge = redirect[x.$edge];)+ })*
                    $(Self::$sub(x) => x.rewrite_indices(redirect),)*
                }
            }

            fn visit_input_indices(&self, f: &mut impl FnMut(usize)) {
                match self {
                    $(Self::$variant(x) => { $(f(x.$edge);)+ })*
                    $(Self::$sub(x) => x.visit_input_indices(f),)*
                }
            }
        }

        impl DensitySampler for DependentDensityFunction {
            fn sample_value(&self, ctx: Fill<'_>, index: usize) -> f32 {
                match self {
                    $(Self::$variant(x) => x.sample_value(ctx, index),)*
                    $(Self::$sub(x) => x.sample_value(ctx, index),)*
                }
            }

            fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
                match self {
                    $(Self::$variant(x) => x.sample_volume(ctx, out),)*
                    $(Self::$sub(x) => x.sample_volume(ctx, out),)*
                }
            }
        }
    };
}

independent_density_functions! {
    Constant(Constant),
    OldBlendedNoise(BlendedNoise),
    Noise(Noise),
    ShiftB(ShiftB),
    ClampedYGradient(ClampedYGradient),
    Gradient(Gradient),
    DistanceToPoint(DistanceToPoint),
    EndOuterIslands(EndIslands),
}

dependent_density_functions! {
    edges {
        Affine(Affine) { input_index },
        PiecewiseAffine(PiecewiseAffine) { input_index },
        Slide(Slide) { input_index },
        ConstMin(ConstMin) { input_index },
        ConstMax(ConstMax) { input_index },
        ConstSub(ConstSub) { input_index },
        ConstDiv(ConstDiv) { input_index },
        Abs(Abs) { input_index },
        Square(Square) { input_index },
        Cube(Cube) { input_index },
        Negate(Negate) { input_index },
        Reciprocal(Reciprocal) { input_index },
        Sqrt(Sqrt) { input_index },
        Log(Log) { input_index },
        Sign(Sign) { input_index },
        Squeeze(Squeeze) { input_index },
        LeakyReLU(LeakyReLU) { input_index },
        IntegerMultipleRound(IntegerMultipleRound) { input_index },
        ConstExponentPow(ConstExponentPow) { input_index },
        ConstBasePow(ConstBasePow) { input_index },
        Add(Add) { input1_index, input2_index },
        Sub(Sub) { input1_index, input2_index },
        Mul(Mul) { input1_index, input2_index },
        Div(Div) { input1_index, input2_index },
        Min(Min) { input1_index, input2_index },
        Max(Max) { input1_index, input2_index },
        Pow(Pow) { input1_index, input2_index },
        Round(Round) { input1_index, input2_index },
        ShiftedNoise(ShiftedNoise) { input_x_index, input_y_index, input_z_index },
        Clamp(Clamp) { input_index },
        RangeChoice(RangeChoice) { input_index, when_in_index, when_out_index },
        FindTopSurface(FindTopSurface) { density_index, upper_bound_index },
        Lerp(Lerp) { alpha_index, first_index, second_index },
        Slice(Slice) { input_index },
    }
    owns_subtree {
        Spline(Spline),
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
        Self::independent(IndependentDensityFunction::Constant(Constant { value }))
    }

    pub(super) fn as_constant(&self) -> Option<f32> {
        match &self.sampler {
            Sampler::Independent(IndependentDensityFunction::Constant(c)) => Some(c.value),
            _ => None,
        }
    }

    pub(super) fn rewrite_indices(&mut self, redirect: &[usize]) {
        match &mut self.sampler {
            Sampler::Independent(_) => {}
            Sampler::Dependent(x) => x.rewrite_indices(redirect),
            Sampler::Interpolated(x) => x.input_index = redirect[x.input_index],
        }
    }

    pub(super) fn visit_input_indices(&self, f: &mut impl FnMut(usize)) {
        match &self.sampler {
            Sampler::Independent(_) => {}
            Sampler::Dependent(x) => x.visit_input_indices(f),
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
        // Resized, never cleared: `members` is closed under its own inputs, so
        // every row this pass reads is one it already wrote.
        scratch.rows.resize((root + 1) * n, 0.0);
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

    /// [`Arena::fill_node`] for a single position of the same pass.
    pub(super) fn sample_node(
        self,
        i: usize,
        volume: &Volume,
        positions: &[IVec3],
        rows: &[f32],
        index: usize,
    ) -> f32 {
        let ctx = Fill {
            arena: self,
            volume,
            positions,
            filled: &rows[..i * volume.len()],
        };
        self.stack[i].sample_value(ctx, index)
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

impl DensitySampler for DensityFunctionComponent {
    fn sample_value(&self, ctx: Fill<'_>, index: usize) -> f32 {
        match &self.sampler {
            Sampler::Independent(x) => x.sample_value(ctx, index),
            Sampler::Dependent(x) => x.sample_value(ctx, index),
            Sampler::Interpolated(x) => x.sample_value(ctx, index),
        }
    }

    fn sample_volume(&self, ctx: Fill<'_>, out: &mut [f32]) {
        match &self.sampler {
            Sampler::Independent(x) => x.sample_volume(ctx, out),
            Sampler::Dependent(x) => x.sample_volume(ctx, out),
            Sampler::Interpolated(x) => x.sample_volume(ctx, out),
        }
    }
}
