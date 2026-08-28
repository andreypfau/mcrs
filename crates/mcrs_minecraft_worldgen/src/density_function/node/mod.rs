use super::*;

/// A stack entry that holds no row in the pass being filled.
pub(super) const NO_SLOT: u32 = u32::MAX;

/// Entry `i` at row `i`: the slot map of a caller that materialises the whole
/// stack rather than the entries one root needs.
pub(super) fn identity_slots(count: usize) -> Vec<u32> {
    (0..count as u32).collect()
}

/// What a sampler reads while it fills a volume: the arena, the volume with its
/// materialised positions, and the rows every node below it already wrote.
///
/// Rows are addressed by slot rather than by stack index. A pass writes a row
/// only for the entries it needs, so a graph of two hundred entries evaluated
/// for a dozen live ones costs a dozen rows.
#[derive(Clone, Copy)]
pub(crate) struct Fill<'a> {
    pub(crate) arena: Arena<'a>,
    pub(crate) volume: &'a Volume,
    pub(crate) positions: &'a [IVec3],
    slots: &'a [u32],
    filled: &'a [f32],
}

impl<'a> Fill<'a> {
    #[inline]
    pub(crate) fn len(self) -> usize {
        self.positions.len()
    }

    /// The row node `index` wrote earlier in this pass. Reading an entry the
    /// pass gave no slot is a bug in the liveness walk, and panics here.
    #[inline]
    pub(crate) fn row(self, index: usize) -> &'a [f32] {
        let n = self.len();
        debug_assert_ne!(self.slots[index], NO_SLOT, "no row for entry {index}");
        let base = self.slots[index] as usize * n;
        &self.filled[base..base + n]
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

    /// The inputs this entry reads out of the pass's own rows over `volume`.
    ///
    /// An entry that evaluates a subgraph of its own reads that subgraph from
    /// its own buffer, at positions this volume does not contain. Keeping the
    /// row alive for it would evaluate the whole subtree over the wrong
    /// positions and then discard it.
    pub(super) fn visit_row_inputs(&self, volume: &Volume, f: &mut impl FnMut(usize)) {
        match &self.sampler {
            Sampler::Independent(_) => {}
            Sampler::Dependent(DependentDensityFunction::Slice(_)) => {}
            Sampler::Dependent(DependentDensityFunction::FindTopSurface(x)) => {
                f(x.upper_bound_index)
            }
            Sampler::Dependent(x) => x.visit_input_indices(f),
            Sampler::Interpolated(x) => {
                if x.is_lattice_volume(volume) {
                    f(x.input_index)
                }
            }
        }
    }
}

#[inline]
pub fn lerp(delta: f32, start: f32, end: f32) -> f32 {
    start + delta * (end - start)
}

/// The stack entries a node evaluates for itself, over a volume of its own
/// choosing rather than the one its caller asked for.
///
/// A slice pins an axis, an interpolation drops to its cell lattice, a surface
/// probe walks down a column: none of them can read their input out of the
/// caller's rows, because those rows hold the wrong positions. Each carries the
/// closure of its input under the input edges, split the same way a whole fill
/// is — the entries that hold for a column, then the rest in topological order
/// with the subgraph's own root last.
#[derive(Clone, Debug, PartialEq, Default)]
pub(super) struct Subgraph {
    column: Box<[u32]>,
    per_y: Box<[u32]>,
    slots: Box<[u32]>,
}

impl Subgraph {
    /// `members` ascending, closed under its inputs, its root last.
    pub(super) fn new(members: Vec<u32>, column_ready: &[bool]) -> Self {
        let (column, per_y): (Vec<u32>, Vec<u32>) = members
            .iter()
            .partition(|&&member| column_ready[member as usize]);
        let extent = members.last().map_or(0, |&last| last as usize + 1);
        let mut slots = vec![NO_SLOT; extent];
        for (slot, &member) in column.iter().chain(per_y.iter()).enumerate() {
            slots[member as usize] = slot as u32;
        }
        Self {
            column: column.into_boxed_slice(),
            per_y: per_y.into_boxed_slice(),
            slots: slots.into_boxed_slice(),
        }
    }

    /// How many rows a fill of this subgraph writes before its root.
    #[inline]
    fn stashed(&self) -> usize {
        self.column.len() + self.per_y.len().saturating_sub(1)
    }
}

/// The compiled node arena: everything a fill needs that is not the volume.
#[derive(Clone, Copy)]
pub(crate) struct Arena<'a> {
    stack: &'a [DensityFunctionComponent],
    scratch: &'a FillScratch,
}

impl<'a> Arena<'a> {
    pub(super) fn new(stack: &'a [DensityFunctionComponent], scratch: &'a FillScratch) -> Self {
        Self { stack, scratch }
    }

    #[inline]
    pub(super) fn pool(self) -> &'a BufferPool {
        &self.scratch.pool
    }

    #[inline]
    pub(super) fn cache(self) -> &'a RefCell<ColumnCache> {
        &self.scratch.column
    }

    /// Evaluate `subgraph` over every position of `volume`.
    pub(super) fn fill_subgraph(self, subgraph: &Subgraph, volume: &Volume, out: &mut [f32]) {
        let mut positions = self.pool().positions(volume.len());
        volume.positions_into(&mut positions);
        self.fill_subgraph_at(subgraph, volume, &positions, out);
    }

    /// [`Arena::fill_subgraph`] against positions the caller already
    /// materialised, for a node whose volume matches its caller's.
    pub(super) fn fill_subgraph_at(
        self,
        subgraph: &Subgraph,
        volume: &Volume,
        positions: &[IVec3],
        out: &mut [f32],
    ) {
        let n = volume.len();
        let mut rows = self.pool().floats(subgraph.stashed() * n);
        if !subgraph.column.is_empty() {
            self.spread_column(&subgraph.column, &subgraph.slots, volume, &mut rows);
        }
        let Some((root, rest)) = subgraph.per_y.split_last() else {
            // Nothing in the subgraph varies with Y, so its root is the last of
            // the rows the column pass just spread.
            let last = *subgraph.column.last().expect("a subgraph has a root") as usize;
            let base = subgraph.slots[last] as usize * n;
            out.copy_from_slice(&rows[base..base + n]);
            return;
        };
        for &member in rest {
            self.fill_node(member as usize, volume, positions, &subgraph.slots, &mut rows);
        }
        // The root has no reader inside the subgraph, so it writes where the
        // caller wants it rather than into a row that would then be copied.
        self.fill_into(
            *root as usize,
            volume,
            positions,
            &subgraph.slots,
            &rows,
            out,
        );
    }

    /// Bring the cached column up to date with `entries`, then repeat each of
    /// its values down the matching column of `rows`.
    ///
    /// The cache is borrowed across the fills, which is sound because an entry
    /// the column pass evaluates never re-enters one: `column_ready` excludes
    /// every node that resamples a subgraph of its own.
    pub(super) fn spread_column(
        self,
        entries: &[u32],
        slots: &[u32],
        volume: &Volume,
        rows: &mut [f32],
    ) {
        let n = volume.len();
        let height = volume.size().y as usize;
        let column_volume = volume.column();
        let columns = column_volume.len();

        let mut cache = self.cache().borrow_mut();
        let column = cache.column(&column_volume, self.stack.len());

        // Claiming every row before filling any keeps the buffer from moving
        // under a fill that is already reading it.
        let mut fresh = self.pool().slots(entries.len());
        let mut count = 0usize;
        for &entry in entries {
            if column.claim(entry as usize, columns) {
                fresh[count] = entry;
                count += 1;
            }
        }
        for &entry in &fresh[..count] {
            let Column {
                rows,
                slots,
                positions,
                ..
            } = &mut *column;
            self.fill_node(entry as usize, &column_volume, positions, slots, rows);
        }

        for &entry in entries {
            let base = slots[entry as usize] as usize * n;
            for (c, &value) in column.row(entry as usize, columns).iter().enumerate() {
                rows[base + c * height..base + (c + 1) * height].fill(value);
            }
        }
    }

    /// Fill entry `i` into `out`, reading its inputs from `filled`.
    pub(super) fn fill_into(
        self,
        i: usize,
        volume: &Volume,
        positions: &[IVec3],
        slots: &[u32],
        filled: &[f32],
        out: &mut [f32],
    ) {
        let ctx = Fill {
            arena: self,
            volume,
            positions,
            slots,
            filled,
        };
        self.stack[i].sample_volume(ctx, out);
    }

    /// [`Arena::fill_node`] for a single position of the same pass.
    pub(super) fn sample_node(
        self,
        i: usize,
        volume: &Volume,
        positions: &[IVec3],
        slots: &[u32],
        rows: &[f32],
        index: usize,
    ) -> f32 {
        let ctx = Fill {
            arena: self,
            volume,
            positions,
            slots,
            filled: &rows[..slots[i] as usize * volume.len()],
        };
        self.stack[i].sample_value(ctx, index)
    }

    /// Fill entry `i` into its own slot of `rows`.
    ///
    /// Slots ascend with topological order, so every input of `i` sits below
    /// `i`'s own slot and the split hands the writer exactly what it may read.
    pub(super) fn fill_node(
        self,
        i: usize,
        volume: &Volume,
        positions: &[IVec3],
        slots: &[u32],
        rows: &mut [f32],
    ) {
        let n = volume.len();
        let base = slots[i] as usize * n;
        let (filled, out) = rows.split_at_mut(base);
        self.fill_into(i, volume, positions, slots, filled, &mut out[..n]);
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
