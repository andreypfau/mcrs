use crate::branch::{self, Fallback, GuardTest, Step};
use crate::interval::Interval;
use crate::jmath;
use crate::kernel::{Runs, at, each_column, map1, zip2, zip3};
use crate::node::blended::BlendedParams;
use crate::node::distance::DistanceParams;
use crate::node::end_island::EndIslandParams;
use crate::node::gradient::GradientParams;
use crate::node::noise::NoiseParams;
use crate::node::spline::CompiledSpline;
use crate::strata::{ALL_AXES, AXIS_Y, Axes, extent, stratum};
use crate::volume::Volume;
use bevy_math::IVec3;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    Abs,
    Square,
    Cube,
    Sqrt,
    Reciprocal,
    Negate,
    Squeeze,
    Log,
    Sign,
}

impl UnaryOp {
    #[inline]
    pub fn apply(self, v: f32) -> f32 {
        match self {
            UnaryOp::Abs => v.abs(),
            UnaryOp::Square => v * v,
            UnaryOp::Cube => v * v * v,
            UnaryOp::Sqrt => jmath::sqrt(v),
            UnaryOp::Reciprocal => 1.0 / v,
            UnaryOp::Negate => -v,
            UnaryOp::Squeeze => {
                let c = jmath::clampf(v, -1.0, 1.0);
                c / 2.0 - (c * c * c) / 24.0
            }
            UnaryOp::Log => jmath::log(v),
            UnaryOp::Sign => jmath::signum(v),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Min,
    Max,
}

impl BinaryOp {
    #[inline]
    pub fn apply(self, a: f32, b: f32) -> f32 {
        match self {
            BinaryOp::Add => a + b,
            BinaryOp::Sub => a - b,
            BinaryOp::Mul => a * b,
            BinaryOp::Div => a / b,
            BinaryOp::Min => jmath::vmin(a, b),
            BinaryOp::Max => jmath::vmax(a, b),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoundKind {
    Floor,
    Round,
    Ceil,
    Truncate,
}

impl RoundKind {
    #[inline]
    pub fn apply(self, v: f32) -> f32 {
        match self {
            RoundKind::Floor => v.floor(),
            // Java rounds halves up, not away from zero: round(-2.5) is -2.
            RoundKind::Round => (v + 0.5).floor(),
            RoundKind::Ceil => v.ceil(),
            RoundKind::Truncate => {
                if v > 0.0 {
                    v.floor()
                } else {
                    v.ceil()
                }
            }
        }
    }
}

pub type NodeId = u32;

/// One operation in the flat graph. Inputs are indices into the same array, so a
/// node reachable from two parents exists once and is evaluated once.
///
/// The specializations are vanilla's, with two additions the previous engine had
/// and vanilla lacks: `Affine` folds a constant multiply and a constant add into
/// one node, and `PiecewiseAffine` folds a leaky rectifier into it. Both compute
/// with two roundings, never a fused multiply-add — Java has no implicit FMA and
/// fusing moves the last bit.
#[derive(Clone, Debug)]
pub enum Node {
    // --- leaves ---
    Constant(f32),
    Gradient(GradientParams),
    Noise {
        params: Arc<NoiseParams>,
    },
    ShiftB {
        params: Arc<NoiseParams>,
    },
    DistanceToPoint(DistanceParams),
    EndOuterIslands(Arc<EndIslandParams>),
    OldBlendedNoise(Arc<BlendedParams>),

    // --- one input ---
    Affine {
        input: NodeId,
        scale: f32,
        offset: f32,
    },
    PiecewiseAffine {
        input: NodeId,
        neg_scale: f32,
        pos_scale: f32,
        offset: f32,
    },
    Unary {
        op: UnaryOp,
        input: NodeId,
    },
    LeakyRelu {
        input: NodeId,
        negative_scale: f32,
    },
    Clamp {
        input: NodeId,
        min: f32,
        max: f32,
    },
    ConstMin {
        input: NodeId,
        value: f32,
    },
    ConstMax {
        input: NodeId,
        value: f32,
    },
    /// `constant - input`. The other order folds into `Affine`.
    ConstSub {
        input: NodeId,
        value: f32,
    },
    /// `constant / input`.
    ConstDiv {
        input: NodeId,
        value: f32,
    },
    ConstBasePow {
        base: f32,
        exponent: NodeId,
    },
    ConstExponentPow {
        input: NodeId,
        exponent: f32,
    },
    IntegerMultipleRound {
        input: NodeId,
        multiple: f32,
        kind: RoundKind,
    },

    // --- two inputs ---
    Binary {
        op: BinaryOp,
        a: NodeId,
        b: NodeId,
    },
    Pow {
        base: NodeId,
        exponent: NodeId,
    },
    Round {
        value: NodeId,
        multiple: NodeId,
        kind: RoundKind,
    },

    // --- three or more ---
    Lerp {
        alpha: NodeId,
        first: NodeId,
        second: NodeId,
    },
    ConstFirstLerp {
        alpha: NodeId,
        first: f32,
        second: NodeId,
    },
    ConstSecondLerp {
        alpha: NodeId,
        first: NodeId,
        second: f32,
    },
    ShiftedNoise {
        params: Arc<NoiseParams>,
        x: NodeId,
        y: NodeId,
        z: NodeId,
    },
    RangeChoice {
        input: NodeId,
        min_inclusive: f32,
        max_exclusive: f32,
        when_in: NodeId,
        when_out: NodeId,
    },
    ConstRangeChoice {
        input: NodeId,
        min_inclusive: f32,
        max_exclusive: f32,
        when_in: f32,
        when_out: f32,
    },
    SingleThreshold {
        input: NodeId,
        threshold: f32,
        below: NodeId,
        above: NodeId,
    },
    IntervalSelect {
        input: NodeId,
        thresholds: Arc<[f32]>,
        arms: Arc<[NodeId]>,
    },
    Spline {
        spline: Arc<CompiledSpline>,
        coords: Arc<[NodeId]>,
    },

    // --- one input, read away from the position being filled ---
    /// Trilinear interpolation of `input` over a cell lattice. `cell` indexes the
    /// lattice buffer the fill materializes for it.
    Interpolated {
        input: NodeId,
        cell_xz: i32,
        cell_y: i32,
        cell: usize,
    },
    /// The highest cell boundary at or below `upper_bound` where `density` is
    /// positive. `upper_bound` is read at y = 0, never at the filled position.
    FindTopSurface {
        density: NodeId,
        upper_bound: NodeId,
        lower_bound: i32,
        cell_height: i32,
    },
}

impl Node {
    /// Every node this one reads, in evaluation order. Drives topological
    /// sorting, dead-node elimination and the guard cones.
    pub fn visit_inputs(&self, f: &mut impl FnMut(NodeId)) {
        match self {
            Node::Constant(_)
            | Node::Gradient(_)
            | Node::Noise { .. }
            | Node::ShiftB { .. }
            | Node::DistanceToPoint(_)
            | Node::EndOuterIslands(_)
            | Node::OldBlendedNoise(_) => {}

            Node::Affine { input, .. }
            | Node::PiecewiseAffine { input, .. }
            | Node::Unary { input, .. }
            | Node::LeakyRelu { input, .. }
            | Node::Clamp { input, .. }
            | Node::ConstMin { input, .. }
            | Node::ConstMax { input, .. }
            | Node::ConstSub { input, .. }
            | Node::ConstDiv { input, .. }
            | Node::ConstExponentPow { input, .. }
            | Node::IntegerMultipleRound { input, .. }
            | Node::ConstRangeChoice { input, .. } => f(*input),
            Node::ConstBasePow { exponent, .. } => f(*exponent),

            Node::Binary { a, b, .. } => {
                f(*a);
                f(*b);
            }
            Node::Pow { base, exponent } => {
                f(*base);
                f(*exponent);
            }
            Node::Round {
                value, multiple, ..
            } => {
                f(*value);
                f(*multiple);
            }

            Node::Lerp {
                alpha,
                first,
                second,
            } => {
                f(*alpha);
                f(*first);
                f(*second);
            }
            Node::ConstFirstLerp { alpha, second, .. } => {
                f(*alpha);
                f(*second);
            }
            Node::ConstSecondLerp { alpha, first, .. } => {
                f(*alpha);
                f(*first);
            }
            Node::ShiftedNoise { x, y, z, .. } => {
                f(*x);
                f(*y);
                f(*z);
            }
            Node::RangeChoice {
                input,
                when_in,
                when_out,
                ..
            } => {
                f(*input);
                f(*when_in);
                f(*when_out);
            }
            Node::SingleThreshold {
                input,
                below,
                above,
                ..
            } => {
                f(*input);
                f(*below);
                f(*above);
            }
            Node::IntervalSelect { input, arms, .. } => {
                f(*input);
                arms.iter().copied().for_each(&mut *f);
            }
            Node::Spline { coords, .. } => coords.iter().copied().for_each(&mut *f),

            Node::Interpolated { input, .. } => f(*input),
            Node::FindTopSurface {
                density,
                upper_bound,
                ..
            } => {
                f(*density);
                f(*upper_bound);
            }
        }
    }

    /// The inputs this node reads at the position being filled. The two kinds
    /// that sample elsewhere hide those inputs here, so a plan built from this
    /// walk never evaluates a subtree at the wrong position — and never at all
    /// unless something else in the plan reads it in place.
    pub(crate) fn visit_local_inputs(&self, f: &mut impl FnMut(NodeId)) {
        match self {
            Node::Interpolated { .. } => {}
            Node::FindTopSurface { upper_bound, .. } => f(*upper_bound),
            other => other.visit_inputs(f),
        }
    }
}

/// A compiled density graph: nodes in topological order, each tagged with the
/// axes it varies over.
///
/// The stratum replaces vanilla's `Slice`. A fill evaluates one node at a time
/// over the whole volume, and each node's buffer holds only the axes it varies
/// over: a node that ignores Y holds one value per column, and one that ignores
/// X and Z holds one value for the fill. Both fall out of `axes`, with no
/// rewrite pass and no slice node.
pub struct Program {
    nodes: Box<[Node]>,
    axes: Box<[Axes]>,
    /// One per node that can be filled on its own: the roots, plus the inputs the
    /// two re-entrant kinds sample away from the filled position.
    plans: Box<[Plan]>,
    plan_of: Box<[u32]>,
    roots: Box<[NodeId]>,
    cell_count: usize,
}

/// What one fill of a given node evaluates, restricted to the nodes that node
/// can reach in place. Filling a root no longer walks the whole array: an
/// unrelated `Interpolated` elsewhere in the graph would otherwise materialize
/// its lattice, whose own fill would materialize the first one's, and so on.
struct Plan {
    /// Every node reached, ordered so that each guarded input's exclusive cone is
    /// one contiguous run and every node still follows its inputs.
    order: Box<[NodeId]>,
    steps: Box<[Step]>,
    /// The `Interpolated` nodes reached, whose lattices are materialized before
    /// the order runs.
    lattice: Box<[NodeId]>,
    /// Which buffer each position in `order` writes, numbered within the run of
    /// buffers that share its node's axes.
    slot: Box<[u32]>,
    /// Buffers needed per axes combination, so a fill holds the width of the
    /// graph rather than one stratum per node.
    slot_count: [u32; 8],
}

#[derive(Default)]
pub struct Workspace {
    /// One stratum per live slot. Slots are recycled, so a node's offset may sit
    /// either side of an offset it reads.
    values: Vec<f32>,
    offset: Vec<u32>,
    lattices: Vec<Lattice>,
    probe: Vec<f32>,
    nested: Option<Box<Workspace>>,
    count: EvalCount,
}

/// Node evaluations, summed over every fill and every nested fill since the last
/// [`Workspace::take_count`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EvalCount {
    pub evaluated: u64,
    pub skipped: u64,
}

/// One `Interpolated` node's input, sampled over the cell lattice enclosing the
/// volume being filled.
#[derive(Default)]
struct Lattice {
    volume: Option<Volume>,
    /// The fill's own volume already was the cell lattice, so `values` holds the
    /// input at exactly the positions asked for and is read straight back.
    direct: bool,
    values: Vec<f32>,
}

impl Program {
    pub fn new(
        nodes: Vec<Node>,
        axes: Vec<Axes>,
        ranges: Vec<Interval>,
        roots: Vec<NodeId>,
    ) -> Self {
        assert_eq!(nodes.len(), axes.len());
        assert_eq!(nodes.len(), ranges.len());
        let mut entries: Vec<NodeId> = Vec::new();
        let add = |entries: &mut Vec<NodeId>, id: NodeId| {
            if !entries.contains(&id) {
                entries.push(id);
            }
        };
        for &root in &roots {
            add(&mut entries, root);
        }
        let mut cell_count = 0;
        for node in &nodes {
            match node {
                Node::Interpolated { input, cell, .. } => {
                    add(&mut entries, *input);
                    cell_count = cell_count.max(cell + 1);
                }
                Node::FindTopSurface {
                    density,
                    upper_bound,
                    ..
                } => {
                    add(&mut entries, *density);
                    if axes[*upper_bound as usize] & AXIS_Y != 0 {
                        add(&mut entries, *upper_bound);
                    }
                }
                _ => {}
            }
        }

        let mut plan_of = vec![u32::MAX; nodes.len()];
        let mut plans = Vec::with_capacity(entries.len());
        for (index, &entry) in entries.iter().enumerate() {
            plan_of[entry as usize] = index as u32;
            plans.push(Plan::new(&nodes, &axes, &ranges, entry));
        }

        Self {
            nodes: nodes.into_boxed_slice(),
            axes: axes.into_boxed_slice(),
            plans: plans.into_boxed_slice(),
            plan_of: plan_of.into_boxed_slice(),
            roots: roots.into_boxed_slice(),
            cell_count,
        }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn root_count(&self) -> usize {
        self.roots.len()
    }

    #[inline]
    pub fn root_node(&self, root: usize) -> NodeId {
        self.roots[root]
    }

    #[inline]
    pub fn axes_of(&self, id: NodeId) -> Axes {
        self.axes[id as usize]
    }

    #[inline]
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
    }

    /// Writes `volume.len()` values for `root` into `out`, laid out Y-fastest to
    /// match [`Volume::index_unchecked`].
    pub fn fill(&self, ws: &mut Workspace, volume: &Volume, root: usize, out: &mut [f32]) {
        self.fill_node(ws, volume, self.roots[root], out);
    }

    /// [`Program::fill`] targeting any node that has a plan: a root, or the input
    /// a re-entrant kind samples away from the position being filled.
    pub fn fill_node(&self, ws: &mut Workspace, volume: &Volume, target: NodeId, out: &mut [f32]) {
        debug_assert_eq!(out.len(), volume.len());
        let plan = self.plan_of(target);
        ws.prepare(self, plan, volume);
        self.fill_lattices(ws, volume, plan);

        let (mut k, mut step) = (0usize, 0usize);
        while step < plan.steps.len() {
            match plan.steps[step] {
                Step::Eval { end } => {
                    let end = end as usize;
                    ws.count.evaluated += (end - k) as u64;
                    while k < end {
                        self.eval(plan.order[k], ws, volume);
                        k += 1;
                    }
                    step += 1;
                }
                Step::Guard {
                    test,
                    fallback,
                    skip_to,
                    next_step,
                } => {
                    if self.guard_holds(test, ws, volume) {
                        step += 1;
                    } else {
                        if let Some(fallback) = fallback {
                            self.apply_fallback(fallback, ws, volume);
                        }
                        ws.count.skipped += (skip_to as usize - k) as u64;
                        k = skip_to as usize;
                        step = next_step as usize;
                    }
                }
            }
        }

        let axes = self.axes_of(target);
        let buf = ws.read(self, target, volume);
        if axes == ALL_AXES {
            out.copy_from_slice(buf);
        } else {
            let size = volume.size();
            let (sx, sy) = (size.x as usize, size.y as usize);
            let runs = Runs::new(buf, axes, volume);
            for iz in 0..size.z as usize {
                for ix in 0..sx {
                    let base = (ix + iz * sx) * sy;
                    let run = runs.col(ix, iz);
                    if run.len() == sy {
                        out[base..base + sy].copy_from_slice(run);
                    } else {
                        out[base..base + sy].fill(run[0]);
                    }
                }
            }
        }
        debug_assert!(
            out.iter().all(|v| !v.is_nan()),
            "density values are guaranteed NaN-free; a NaN here means a division by zero, \
             a log of a negative or a sqrt of a negative reached the graph"
        );
    }

    fn plan_of(&self, target: NodeId) -> &Plan {
        let index = self.plan_of[target as usize];
        assert_ne!(index, u32::MAX, "node {target} has no fill plan");
        &self.plans[index as usize]
    }

    /// Samples each reached `Interpolated` node's input over the cell lattice
    /// enclosing `volume`, once for the whole fill rather than once per column.
    fn fill_lattices(&self, ws: &mut Workspace, volume: &Volume, plan: &Plan) {
        if plan.lattice.is_empty() {
            return;
        }
        let mut nested = ws.nested.take().unwrap_or_default();
        for k in 0..plan.lattice.len() {
            let id = plan.lattice[k];
            let &Node::Interpolated {
                input,
                cell_xz,
                cell_y,
                cell,
            } = &self.nodes[id as usize]
            else {
                unreachable!("the lattice list holds only interpolated nodes")
            };
            let direct = is_lattice_volume(volume, cell_xz, cell_y);
            let lattice = if direct {
                *volume
            } else {
                lattice_volume(volume, self.axes_of(id), cell_xz, cell_y)
            };
            let slot = &mut ws.lattices[cell];
            slot.volume = Some(lattice);
            slot.direct = direct;
            slot.values.clear();
            slot.values.resize(lattice.len(), 0.0);
            self.fill_node(&mut nested, &lattice, input, &mut ws.lattices[cell].values);
        }
        ws.absorb(&mut nested);
        ws.nested = Some(nested);
    }

    #[allow(clippy::too_many_arguments)]
    fn fill_top_surface(
        &self,
        ws: &mut Workspace,
        volume: &Volume,
        id: NodeId,
        density: NodeId,
        upper_bound: NodeId,
        lower_bound: i32,
        cell_height: i32,
    ) {
        let axes = self.axes_of(id);
        let ext = stratum(axes, volume);
        let size = ext.size();
        debug_assert_eq!(size.y, 1, "a surface level does not vary along Y");
        let off = ws.offset[id as usize] as usize;
        let mut slot = off;
        for iz in 0..size.z {
            for ix in 0..size.x {
                let value = self.find_top_surface(
                    ws,
                    volume,
                    &ext,
                    ix as usize,
                    iz as usize,
                    density,
                    upper_bound,
                    lower_bound,
                    cell_height,
                );
                ws.values[slot] = value;
                slot += 1;
            }
        }
    }

    /// `FindTopSurfaceFunction.Sampler.findSurfaceFrom`, probing one batch of
    /// cell boundaries per nested fill so the descent still stops at the first
    /// positive density.
    #[allow(clippy::too_many_arguments)]
    fn find_top_surface(
        &self,
        ws: &mut Workspace,
        volume: &Volume,
        ext: &Volume,
        ix: usize,
        iz: usize,
        density: NodeId,
        upper_bound: NodeId,
        lower_bound: i32,
        cell_height: i32,
    ) -> f32 {
        const BATCH: i64 = 64;

        let bx = ext.block_x(ix as i32);
        let bz = ext.block_z(iz as i32);
        let upper = if self.axes_of(upper_bound) & AXIS_Y == 0 {
            let buf = ws.read(self, upper_bound, volume);
            Runs::new(buf, self.axes_of(upper_bound), volume).col(ix, iz)[0]
        } else {
            let mut nested = ws.nested.take().unwrap_or_default();
            let mut pinned = [0.0f32];
            let at_zero = Volume::point(IVec3::new(bx, 0, bz));
            self.fill_node(&mut nested, &at_zero, upper_bound, &mut pinned);
            ws.absorb(&mut nested);
            ws.nested = Some(nested);
            pinned[0]
        };

        let top_y = jmath::mth_floor(upper / cell_height as f32).wrapping_mul(cell_height);
        if top_y <= lower_bound {
            return lower_bound as f32;
        }

        let mut nested = ws.nested.take().unwrap_or_default();
        let mut probe = std::mem::take(&mut ws.probe);
        let mut found = lower_bound;
        let mut probe_y = top_y;
        while probe_y >= lower_bound {
            let remaining = (probe_y as i64 - lower_bound as i64) / cell_height as i64 + 1;
            let count = remaining.min(BATCH) as usize;
            let min_y = probe_y - (count as i32 - 1) * cell_height;
            probe.clear();
            probe.resize(count, 0.0);
            let span = Volume::new(
                IVec3::new(1, count as i32, 1),
                IVec3::new(bx, min_y, bz),
                IVec3::new(1, cell_height, 1),
            );
            self.fill_node(&mut nested, &span, density, &mut probe);
            if let Some(k) = (0..count).rev().find(|&k| probe[k] > 0.0) {
                found = min_y + k as i32 * cell_height;
                break;
            }
            probe_y = min_y - cell_height;
        }
        ws.absorb(&mut nested);
        ws.nested = Some(nested);
        ws.probe = probe;
        found as f32
    }

    /// Whether any position in the volume reads the guarded run.
    fn guard_holds(&self, test: GuardTest, ws: &Workspace, volume: &Volume) -> bool {
        let all = |id| ws.read(self, id, volume);
        match test {
            GuardTest::InRange {
                selector,
                lo,
                hi,
                want,
            } => all(selector).iter().any(|&v| (v >= lo && v < hi) == want),
            GuardTest::Below {
                selector,
                threshold,
                want,
            } => all(selector).iter().any(|&v| (v < threshold) == want),
            GuardTest::Arm {
                site,
                selector,
                arm,
            } => {
                let Node::IntervalSelect {
                    thresholds, arms, ..
                } = &self.nodes[site as usize]
                else {
                    unreachable!("an arm guard sits on an interval select")
                };
                all(selector).iter().any(|&v| {
                    thresholds
                        .iter()
                        .position(|&t| v < t)
                        .unwrap_or(arms.len() - 1)
                        == arm as usize
                })
            }
            GuardTest::LeftAbove { left, limit } => all(left).iter().any(|&v| v > limit),
            GuardTest::LeftBelow { left, limit } => all(left).iter().any(|&v| v < limit),
            GuardTest::LeftNonZero { left } => all(left).iter().any(|&v| v != 0.0),
        }
    }

    fn apply_fallback(&self, fallback: Fallback, ws: &mut Workspace, volume: &Volume) {
        let axes = self.axes_of(fallback.site);
        let ext = stratum(axes, volume);
        let source_axes = self.axes_of(fallback.source);
        let off = ws.offset[fallback.site as usize] as usize;
        let source_off = ws.offset[fallback.source as usize] as usize;

        let len = extent(axes, volume);
        let (below, rest) = ws.values.split_at_mut(off);
        let (out, above) = rest.split_at_mut(len);
        let source = Runs::new(
            other_slot(below, above, off + len, source_off, extent(source_axes, volume)),
            source_axes,
            volume,
        );
        let negate = fallback.negate;
        each_column(out, &ext, |run, ix, iz| {
            let source = source.col(ix, iz);
            for (i, o) in run.iter_mut().enumerate() {
                *o = if negate {
                    -at(source, i)
                } else {
                    at(source, i)
                };
            }
        });
    }

    fn eval(&self, id: NodeId, ws: &mut Workspace, volume: &Volume) {
        if let &Node::FindTopSurface {
            density,
            upper_bound,
            lower_bound,
            cell_height,
        } = &self.nodes[id as usize]
        {
            self.fill_top_surface(
                ws,
                volume,
                id,
                density,
                upper_bound,
                lower_bound,
                cell_height,
            );
            return;
        }
        let axes = self.axes_of(id);
        let ext = stratum(axes, volume);
        let off = ws.offset[id as usize] as usize;

        let Workspace {
            values,
            offset,
            lattices,
            ..
        } = ws;
        let len = extent(axes, volume);
        let (below, rest) = values.split_at_mut(off);
        let (out, above) = rest.split_at_mut(len);
        let (below, above): (&[f32], &[f32]) = (below, above);
        let end = off + len;

        let read = |j: NodeId| -> Runs<'_> {
            let j_axes = self.axes[j as usize];
            let j_off = offset[j as usize] as usize;
            Runs::new(
                other_slot(below, above, end, j_off, extent(j_axes, volume)),
                j_axes,
                volume,
            )
        };

        match &self.nodes[id as usize] {
            Node::Constant(v) => out.fill(*v),
            Node::Gradient(p) => p.eval(out, &ext),
            Node::Noise { params } => params.eval_plain(out, &ext),
            Node::ShiftB { params } => params.eval_shift_b(out, &ext),
            Node::DistanceToPoint(p) => p.eval(out, &ext),
            Node::EndOuterIslands(p) => p.eval(out, &ext),
            Node::OldBlendedNoise(p) => p.eval(out, &ext),

            Node::Affine {
                input,
                scale,
                offset: o,
            } => {
                let (s, o) = (*scale, *o);
                // Adding +0.0 would turn a -0.0 product into +0.0, which the bare
                // constant multiply this folds from does not do.
                if o == 0.0 && o.is_sign_positive() {
                    map_columns(out, &ext, read(*input), |v| v * s)
                } else {
                    map_columns(out, &ext, read(*input), |v| v * s + o)
                }
            }
            Node::PiecewiseAffine {
                input,
                neg_scale,
                pos_scale,
                offset: o,
            } => {
                let (n, p, o) = (*neg_scale, *pos_scale, *o);
                map_columns(out, &ext, read(*input), |v| {
                    if v < 0.0 { v * n + o } else { v * p + o }
                })
            }
            Node::Unary { op, input } => {
                let op = *op;
                map_columns(out, &ext, read(*input), |v| op.apply(v))
            }
            Node::LeakyRelu {
                input,
                negative_scale,
            } => {
                let k = *negative_scale;
                map_columns(out, &ext, read(*input), |v| if v > 0.0 { v } else { v * k })
            }
            Node::Clamp { input, min, max } => {
                let (lo, hi) = (*min, *max);
                map_columns(out, &ext, read(*input), |v| jmath::clampf(v, lo, hi))
            }
            Node::ConstMin { input, value } => {
                let c = *value;
                map_columns(out, &ext, read(*input), |v| jmath::vmin(v, c))
            }
            Node::ConstMax { input, value } => {
                let c = *value;
                map_columns(out, &ext, read(*input), |v| jmath::vmax(v, c))
            }
            Node::ConstSub { input, value } => {
                let c = *value;
                map_columns(out, &ext, read(*input), |v| c - v)
            }
            Node::ConstDiv { input, value } => {
                let c = *value;
                map_columns(out, &ext, read(*input), |v| c / v)
            }
            Node::ConstBasePow { base, exponent } => {
                let b = *base;
                map_columns(out, &ext, read(*exponent), |e| jmath::pow(b, e))
            }
            Node::ConstExponentPow { input, exponent } => {
                let e = *exponent;
                map_columns(out, &ext, read(*input), |v| jmath::pow(v, e))
            }
            Node::IntegerMultipleRound {
                input,
                multiple,
                kind,
            } => {
                let (m, k) = (*multiple, *kind);
                map_columns(out, &ext, read(*input), |v| k.apply(v / m) * m)
            }

            Node::Binary { op, a, b } => {
                let op = *op;
                zip2_columns(out, &ext, read(*a), read(*b), |x, y| op.apply(x, y))
            }
            Node::Pow { base, exponent } => {
                zip2_columns(out, &ext, read(*base), read(*exponent), jmath::pow)
            }
            Node::Round {
                value,
                multiple,
                kind,
            } => {
                let k = *kind;
                zip2_columns(out, &ext, read(*value), read(*multiple), |v, m| {
                    k.apply(v / m) * m
                })
            }

            Node::Lerp {
                alpha,
                first,
                second,
            } => zip3_columns(
                out,
                &ext,
                read(*alpha),
                read(*first),
                read(*second),
                jmath::sampler_lerp,
            ),
            Node::ConstFirstLerp {
                alpha,
                first,
                second,
            } => {
                let f = *first;
                zip2_columns(out, &ext, read(*alpha), read(*second), |a, s| {
                    jmath::sampler_lerp(a, f, s)
                })
            }
            Node::ConstSecondLerp {
                alpha,
                first,
                second,
            } => {
                let s = *second;
                zip2_columns(out, &ext, read(*alpha), read(*first), |a, f| {
                    jmath::sampler_lerp(a, f, s)
                })
            }
            Node::ShiftedNoise { params, x, y, z } => {
                params.eval_shifted(out, read(*x), read(*y), read(*z), &ext)
            }

            Node::RangeChoice {
                input,
                min_inclusive,
                max_exclusive,
                when_in,
                when_out,
            } => {
                let (lo, hi) = (*min_inclusive, *max_exclusive);
                let (sel, a, b) = (read(*input), read(*when_in), read(*when_out));
                each_column(out, &ext, |run, ix, iz| {
                    let (sel, a, b) = (sel.col(ix, iz), a.col(ix, iz), b.col(ix, iz));
                    for (i, o) in run.iter_mut().enumerate() {
                        let v = at(sel, i);
                        *o = if v >= lo && v < hi {
                            at(a, i)
                        } else {
                            at(b, i)
                        };
                    }
                })
            }
            Node::ConstRangeChoice {
                input,
                min_inclusive,
                max_exclusive,
                when_in,
                when_out,
            } => {
                let (lo, hi, a, b) = (*min_inclusive, *max_exclusive, *when_in, *when_out);
                map_columns(out, &ext, read(*input), |v| {
                    if v >= lo && v < hi { a } else { b }
                })
            }
            Node::SingleThreshold {
                input,
                threshold,
                below,
                above,
            } => {
                let t = *threshold;
                let (sel, lo, hi) = (read(*input), read(*below), read(*above));
                each_column(out, &ext, |run, ix, iz| {
                    let (sel, lo, hi) = (sel.col(ix, iz), lo.col(ix, iz), hi.col(ix, iz));
                    for (i, o) in run.iter_mut().enumerate() {
                        *o = if at(sel, i) < t { at(lo, i) } else { at(hi, i) };
                    }
                })
            }
            Node::IntervalSelect {
                input,
                thresholds,
                arms,
            } => {
                let sel = read(*input);
                each_column(out, &ext, |run, ix, iz| {
                    let sel = sel.col(ix, iz);
                    for (i, o) in run.iter_mut().enumerate() {
                        let v = at(sel, i);
                        let k = thresholds
                            .iter()
                            .position(|&t| v < t)
                            .unwrap_or(arms.len() - 1);
                        *o = at(read(arms[k]).col(ix, iz), i);
                    }
                })
            }
            Node::Spline { spline, coords } => spline.eval(out, &|k| read(coords[k]), &ext),

            Node::Interpolated {
                cell_xz,
                cell_y,
                cell,
                ..
            } => {
                let Lattice {
                    volume: Some(lattice),
                    direct,
                    values,
                } = &lattices[*cell]
                else {
                    unreachable!("every interpolated node in the plan holds a lattice")
                };
                let (cell_xz, cell_y, direct) = (*cell_xz, *cell_y, *direct);
                if direct {
                    each_column(out, &ext, |run, ix, iz| {
                        let bx = ext.block_x(ix as i32);
                        let bz = ext.block_z(iz as i32);
                        let base = lattice
                            .index_of_block(bx, ext.min_block().y, bz)
                            .expect("the lattice is the volume being filled");
                        run.copy_from_slice(&values[base..base + run.len()]);
                    })
                } else if ext.step_block() == IVec3::ONE {
                    interpolate_cells(out, &ext, lattice, values, cell_xz, cell_y)
                } else {
                    each_column(out, &ext, |run, ix, iz| {
                        let bx = ext.block_x(ix as i32);
                        let bz = ext.block_z(iz as i32);
                        interpolate(run, bx, bz, &ext, lattice, values, cell_xz, cell_y);
                    })
                }
            }

            Node::FindTopSurface { .. } => unreachable!("handled before the buffer split"),
        }
    }
}

/// A slot other than the one being written. Recycling puts a node's inputs on
/// either side of its output, so the buffer splits in three and the read picks
/// the side its offset falls on; an offset inside the output would mean a node
/// aliasing its own input, and panics here rather than returning wrong values.
#[inline]
fn other_slot<'a>(
    below: &'a [f32],
    above: &'a [f32],
    end: usize,
    off: usize,
    len: usize,
) -> &'a [f32] {
    if off < below.len() {
        &below[off..off + len]
    } else {
        &above[off - end..off - end + len]
    }
}

#[inline]
fn map_columns(out: &mut [f32], ext: &Volume, a: Runs<'_>, f: impl Fn(f32) -> f32) {
    each_column(out, ext, |run, ix, iz| map1(run, a.col(ix, iz), &f));
}

#[inline]
fn zip2_columns(
    out: &mut [f32],
    ext: &Volume,
    a: Runs<'_>,
    b: Runs<'_>,
    f: impl Fn(f32, f32) -> f32,
) {
    each_column(out, ext, |run, ix, iz| {
        zip2(run, a.col(ix, iz), b.col(ix, iz), &f)
    });
}

#[inline]
fn zip3_columns(
    out: &mut [f32],
    ext: &Volume,
    a: Runs<'_>,
    b: Runs<'_>,
    c: Runs<'_>,
    f: impl Fn(f32, f32, f32) -> f32,
) {
    each_column(out, ext, |run, ix, iz| {
        zip3(run, a.col(ix, iz), b.col(ix, iz), c.col(ix, iz), &f)
    });
}

/// Whether `volume` already samples the cell lattice, in which case vanilla
/// hands the input straight through. The chunk generator leans on it: it fills
/// the lattice itself and would otherwise interpolate values back onto the
/// corners they came from.
fn is_lattice_volume(volume: &Volume, cell_xz: i32, cell_y: i32) -> bool {
    let (size, min, step) = (volume.size(), volume.min_block(), volume.step_block());
    (step.x == cell_xz || size.x == 1)
        && (step.y == cell_y || size.y == 1)
        && (step.z == cell_xz || size.z == 1)
        && jmath::floor_mod(min.x, cell_xz) == 0
        && jmath::floor_mod(min.y, cell_y) == 0
        && jmath::floor_mod(min.z, cell_xz) == 0
}

/// The cell corners enclosing every position `volume` asks for, with one extra
/// sample per axis so the far corner of the last cell exists. A node that does
/// not vary along Y is only ever asked for its first row.
fn lattice_volume(volume: &Volume, axes: Axes, cell_xz: i32, cell_y: i32) -> Volume {
    let (size, min, step) = (volume.size(), volume.min_block(), volume.step_block());
    let last_y = if axes & AXIS_Y != 0 {
        min.y + (size.y - 1) * step.y
    } else {
        min.y
    };
    let cell = IVec3::new(cell_xz, cell_y, cell_xz);
    let first = IVec3::new(
        jmath::floor_div(min.x, cell_xz),
        jmath::floor_div(min.y, cell_y),
        jmath::floor_div(min.z, cell_xz),
    );
    let last = IVec3::new(
        jmath::floor_div(min.x + (size.x - 1) * step.x, cell_xz),
        jmath::floor_div(last_y, cell_y),
        jmath::floor_div(min.z + (size.z - 1) * step.z, cell_xz),
    );
    Volume::new(last - first + IVec3::splat(2), first * cell, cell)
}

/// `InterpolatedFunction.Sampler.fillCell` for one column: Z innermost, then X,
/// then Y accumulated one block at a time from the cell's first row. The Y
/// accumulation runs over every block row the cell covers, including rows a
/// strided volume drops, because vanilla reaches a strided volume by filling the
/// dense one and subsampling it.
///
/// The outer loop walks cells rather than samples: a cell spans the block range
/// `[cell_base, cell_base + cell_y)`, so comparing against that bound advances
/// the run without a division per position.
#[allow(clippy::too_many_arguments)]
fn interpolate(
    out: &mut [f32],
    bx: i32,
    bz: i32,
    ext: &Volume,
    lattice: &Volume,
    values: &[f32],
    cell_xz: i32,
    cell_y: i32,
) {
    let inv_xz = 1.0 / cell_xz as f32;
    let inv_y = 1.0 / cell_y as f32;
    let x_in_cell = jmath::floor_mod(bx, cell_xz);
    let z_in_cell = jmath::floor_mod(bz, cell_xz);
    let alpha_x = x_in_cell as f32 * inv_xz;
    let alpha_z = z_in_cell as f32 * inv_xz;
    let ix = (bx - x_in_cell - lattice.min_block().x) / cell_xz;
    let iz = (bz - z_in_cell - lattice.min_block().z) / cell_xz;
    let first_cell_y = jmath::floor_div(lattice.min_block().y, cell_y);
    let min_y = ext.min_block().y;
    let step_y = ext.step_block().y;

    let mut i = 0usize;
    while i < out.len() {
        let mut block_y = min_y + i as i32 * step_y;
        let cell_base = jmath::floor_div(block_y, cell_y) * cell_y;
        let iy = cell_base / cell_y - first_cell_y;
        let corner = |dx, dy, dz| values[lattice.index_unchecked(ix + dx, iy + dy, iz + dz)];
        let v00 = jmath::lerp(alpha_z, corner(0, 0, 0), corner(0, 0, 1));
        let v10 = jmath::lerp(alpha_z, corner(1, 0, 0), corner(1, 0, 1));
        let v01 = jmath::lerp(alpha_z, corner(0, 1, 0), corner(0, 1, 1));
        let v11 = jmath::lerp(alpha_z, corner(1, 1, 0), corner(1, 1, 1));
        let bottom = jmath::lerp(alpha_x, v00, v10);
        let top = jmath::lerp(alpha_x, v01, v11);
        let value_step = (top - bottom) * inv_y;
        let mut row = (min_y - cell_base).max(0);
        let mut value = bottom + value_step * row as f32;

        let cell_end = cell_base + cell_y;
        while i < out.len() && block_y < cell_end {
            let local = block_y - cell_base;
            while row < local {
                value += value_step;
                row += 1;
            }
            out[i] = value;
            i += 1;
            block_y += step_y;
        }
    }
}

/// The same walk as [`interpolate`] with the cell hoisted out of the column
/// loop, for the unstrided volume the chunk generator asks for: a cell's eight
/// corners and its four Z lerps are loaded once instead of once per column, and
/// the Y run each column contributes is a contiguous write.
///
/// Ranges are clipped to `ext` on every axis rather than assumed to be whole
/// cells, and the Y accumulator keeps `interpolate`'s single multiply into the
/// first row followed by repeated addition: a multiply per row would land a
/// bit away from vanilla.
fn interpolate_cells(
    out: &mut [f32],
    ext: &Volume,
    lattice: &Volume,
    values: &[f32],
    cell_xz: i32,
    cell_y: i32,
) {
    let inv_xz = 1.0 / cell_xz as f32;
    let inv_y = 1.0 / cell_y as f32;
    let (size, min) = (ext.size(), ext.min_block());
    let max = min + size - IVec3::ONE;
    let lattice_min = lattice.min_block();

    for cell_z in jmath::floor_div(min.z, cell_xz)..=jmath::floor_div(max.z, cell_xz) {
        let base_z = cell_z * cell_xz;
        let lz = jmath::floor_div(base_z - lattice_min.z, cell_xz);
        let dz0 = (min.z - base_z).max(0);
        let dz1 = (max.z - base_z).min(cell_xz - 1);
        for cell_x in jmath::floor_div(min.x, cell_xz)..=jmath::floor_div(max.x, cell_xz) {
            let base_x = cell_x * cell_xz;
            let lx = jmath::floor_div(base_x - lattice_min.x, cell_xz);
            let dx0 = (min.x - base_x).max(0);
            let dx1 = (max.x - base_x).min(cell_xz - 1);
            for cell_y_index in jmath::floor_div(min.y, cell_y)..=jmath::floor_div(max.y, cell_y) {
                let base_y = cell_y_index * cell_y;
                let ly = jmath::floor_div(base_y - lattice_min.y, cell_y);
                let dy0 = (min.y - base_y).max(0);
                let dy1 = (max.y - base_y).min(cell_y - 1);
                let rows = (dy1 - dy0 + 1) as usize;

                let corner =
                    |dx, dy, dz| values[lattice.index_unchecked(lx + dx, ly + dy, lz + dz)];
                let (c000, c001) = (corner(0, 0, 0), corner(0, 0, 1));
                let (c100, c101) = (corner(1, 0, 0), corner(1, 0, 1));
                let (c010, c011) = (corner(0, 1, 0), corner(0, 1, 1));
                let (c110, c111) = (corner(1, 1, 0), corner(1, 1, 1));

                for dz in dz0..=dz1 {
                    let alpha_z = dz as f32 * inv_xz;
                    let v00 = jmath::lerp(alpha_z, c000, c001);
                    let v10 = jmath::lerp(alpha_z, c100, c101);
                    let v01 = jmath::lerp(alpha_z, c010, c011);
                    let v11 = jmath::lerp(alpha_z, c110, c111);
                    let out_z = base_z + dz - min.z;
                    for dx in dx0..=dx1 {
                        let alpha_x = dx as f32 * inv_xz;
                        let bottom = jmath::lerp(alpha_x, v00, v10);
                        let top = jmath::lerp(alpha_x, v01, v11);
                        let value_step = (top - bottom) * inv_y;
                        let mut value = bottom + value_step * dy0 as f32;
                        let start =
                            ext.index_unchecked(base_x + dx - min.x, base_y + dy0 - min.y, out_z);
                        for slot in &mut out[start..start + rows] {
                            *slot = value;
                            value += value_step;
                        }
                    }
                }
            }
        }
    }
}

impl Plan {
    fn new(nodes: &[Node], axes: &[Axes], ranges: &[Interval], target: NodeId) -> Self {
        let mut live = vec![false; nodes.len()];
        live[target as usize] = true;
        for i in (0..nodes.len()).rev() {
            if live[i] {
                nodes[i].visit_local_inputs(&mut |j| live[j as usize] = true);
            }
        }
        let (mut order, mut lattice) = (Vec::new(), Vec::new());
        for (i, node) in nodes.iter().enumerate() {
            if !live[i] {
                continue;
            }
            order.push(i as NodeId);
            if matches!(node, Node::Interpolated { .. }) {
                lattice.push(i as NodeId);
            }
        }
        let (order, steps) = branch::build(nodes, ranges, target, &order);

        let mut last_use = vec![usize::MAX; nodes.len()];
        for (p, &id) in order.iter().enumerate() {
            nodes[id as usize].visit_local_inputs(&mut |j| last_use[j as usize] = p);
        }
        // The fill reads the target after the schedule ends, so its slot outlives
        // every other and is never handed back.
        last_use[target as usize] = usize::MAX;

        let mut node_slot = vec![0u32; nodes.len()];
        let mut free: [Vec<u32>; 8] = Default::default();
        let mut slot_count = [0u32; 8];
        let mut slot = Vec::with_capacity(order.len());
        for (p, &id) in order.iter().enumerate() {
            // Taking the output slot before releasing the inputs is what keeps a
            // node from writing over a buffer it still reads.
            let a = axes[id as usize] as usize;
            let taken = free[a].pop().unwrap_or_else(|| {
                slot_count[a] += 1;
                slot_count[a] - 1
            });
            node_slot[id as usize] = taken;
            slot.push(taken);
            nodes[id as usize].visit_local_inputs(&mut |j| {
                if last_use[j as usize] == p {
                    last_use[j as usize] = usize::MAX;
                    free[axes[j as usize] as usize].push(node_slot[j as usize]);
                }
            });
        }

        Self {
            order: order.into_boxed_slice(),
            steps: steps.into_boxed_slice(),
            lattice: lattice.into_boxed_slice(),
            slot: slot.into_boxed_slice(),
            slot_count,
        }
    }
}

impl Workspace {
    pub fn new() -> Self {
        Self::default()
    }

    /// The evaluations counted since the last call, cleared.
    pub fn take_count(&mut self) -> EvalCount {
        std::mem::take(&mut self.count)
    }

    fn absorb(&mut self, nested: &mut Workspace) {
        let inner = nested.take_count();
        self.count.evaluated += inner.evaluated;
        self.count.skipped += inner.skipped;
    }

    /// Sizes the plan's slots for this volume and points every node in the plan
    /// at the one it writes. Nodes outside the plan keep whatever offset a
    /// previous fill left them; nothing reads them.
    fn prepare(&mut self, program: &Program, plan: &Plan, volume: &Volume) {
        if self.offset.len() < program.len() {
            self.offset.resize(program.len(), 0);
        }

        let mut base = [0u32; 8];
        let mut len = 0usize;
        for (axes, &count) in plan.slot_count.iter().enumerate() {
            base[axes] = len as u32;
            len += count as usize * extent(axes as Axes, volume);
        }
        for (&id, &slot) in plan.order.iter().zip(plan.slot.iter()) {
            let axes = program.axes_of(id);
            self.offset[id as usize] = base[axes as usize] + slot * extent(axes, volume) as u32;
        }
        // Every node writes its whole stratum before anything reads it, so the
        // buffer is grown rather than cleared: re-zeroing it costs more than the
        // fill it precedes.
        if self.values.len() < len {
            self.values.resize(len, 0.0);
        }
        if self.lattices.len() < program.cell_count {
            self.lattices
                .resize_with(program.cell_count, Lattice::default);
        }
    }

    fn read<'a>(&'a self, program: &Program, id: NodeId, volume: &Volume) -> &'a [f32] {
        let off = self.offset[id as usize] as usize;
        &self.values[off..off + extent(program.axes_of(id), volume)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::gradient::Tiling;
    use crate::strata::{AXIS_X, AXIS_Y, AXIS_Z, NO_AXES};
    use crate::volume::Axis;
    use bevy_math::IVec3;

    fn gradient(axis: Axis, from: f32, to: f32, from_value: f32, to_value: f32) -> Node {
        Node::Gradient(GradientParams {
            axis,
            tiling: Tiling::ClampToEdge,
            from,
            to,
            from_value,
            to_value,
        })
    }

    fn program(nodes: Vec<Node>, axes: Vec<Axes>, roots: Vec<NodeId>) -> Program {
        let ranges = vec![Interval::INFINITE; nodes.len()];
        Program::new(nodes, axes, ranges, roots)
    }

    fn run(program: &Program, volume: &Volume, root: usize) -> Vec<f32> {
        let mut ws = Workspace::new();
        let mut out = vec![0.0; volume.len()];
        program.fill(&mut ws, volume, root, &mut out);
        out
    }

    #[test]
    fn a_constant_fills_the_whole_volume() {
        let p = program(vec![Node::Constant(3.5)], vec![NO_AXES], vec![0]);
        let v = Volume::dense(IVec3::new(2, 3, 2), IVec3::ZERO);
        assert_eq!(run(&p, &v, 0), vec![3.5; 12]);
    }

    #[test]
    fn a_y_only_node_is_evaluated_once_and_shared_by_every_column() {
        let p = program(
            vec![gradient(Axis::Y, 0.0, 8.0, 0.0, 8.0)],
            vec![AXIS_Y],
            vec![0],
        );
        let v = Volume::dense(IVec3::new(2, 4, 2), IVec3::ZERO);
        let out = run(&p, &v, 0);
        for c in 0..4 {
            assert_eq!(&out[c * 4..c * 4 + 4], &[0.0, 1.0, 2.0, 3.0]);
        }
        assert_eq!(
            extent(AXIS_Y, &v),
            4,
            "one buffer of four values backs all four columns"
        );
    }

    #[test]
    fn a_scalar_stratum_broadcasts_into_a_y_run() {
        let nodes = vec![
            gradient(Axis::Y, 0.0, 4.0, 0.0, 4.0),
            gradient(Axis::X, 0.0, 2.0, 0.0, 20.0),
            Node::Binary {
                op: BinaryOp::Add,
                a: 0,
                b: 1,
            },
        ];
        let p = program(nodes, vec![AXIS_Y, AXIS_X, AXIS_X | AXIS_Y], vec![2]);
        let v = Volume::dense(IVec3::new(2, 4, 1), IVec3::ZERO);
        let out = run(&p, &v, 0);
        assert_eq!(&out[0..4], &[0.0, 1.0, 2.0, 3.0], "x = 0 adds nothing");
        assert_eq!(&out[4..8], &[10.0, 11.0, 12.0, 13.0], "x = 1 adds 10");
    }

    #[test]
    fn a_bare_constant_multiply_keeps_a_negative_zero() {
        let nodes = vec![
            Node::Constant(-1.0),
            Node::Affine {
                input: 0,
                scale: 0.0,
                offset: 0.0,
            },
        ];
        let p = program(nodes, vec![NO_AXES; 2], vec![1]);
        assert!(
            run(&p, &Volume::point(IVec3::ZERO), 0)[0].is_sign_negative(),
            "-1.0 * 0.0 is -0.0; folding in a +0.0 offset would lose the sign"
        );
    }

    #[test]
    fn affine_rounds_twice_and_never_fuses() {
        let x = 1.000_000_1_f32;
        let (scale, offset) = (3.000_000_5_f32, -3.000_001_5_f32);
        let nodes = vec![
            Node::Constant(x),
            Node::Affine {
                input: 0,
                scale,
                offset,
            },
        ];
        let p = program(nodes, vec![NO_AXES; 2], vec![1]);
        let v = Volume::point(IVec3::ZERO);
        assert_eq!(
            run(&p, &v, 0)[0],
            x * scale + offset,
            "two roundings, as Java does; mul_add would round once"
        );
    }

    #[test]
    fn a_node_reading_across_phases_sees_both_buffers() {
        let nodes = vec![
            gradient(Axis::Y, 0.0, 2.0, 0.0, 2.0),
            gradient(Axis::Z, 0.0, 2.0, 0.0, 100.0),
            Node::Binary {
                op: BinaryOp::Add,
                a: 0,
                b: 1,
            },
        ];
        let p = program(nodes, vec![AXIS_Y, AXIS_Z, AXIS_Y | AXIS_Z], vec![2]);
        let v = Volume::dense(IVec3::new(1, 2, 2), IVec3::ZERO);
        let out = run(&p, &v, 0);
        assert_eq!(&out[0..2], &[0.0, 1.0], "z = 0");
        assert_eq!(&out[2..4], &[50.0, 51.0], "z = 1 adds 50");
    }

    /// `x` on the lattice, `x * x` off it, so a value that came through the
    /// interpolation is telling apart from one that came from the input.
    fn squared_gradient_interpolated(cell_xz: i32) -> Program {
        let nodes = vec![
            gradient(Axis::X, 0.0, 8.0, 0.0, 8.0),
            Node::Unary {
                op: UnaryOp::Square,
                input: 0,
            },
            Node::Interpolated {
                input: 1,
                cell_xz,
                cell_y: 4,
                cell: 0,
            },
        ];
        program(nodes, vec![AXIS_X, AXIS_X, AXIS_X | AXIS_Z], vec![2])
    }

    #[test]
    fn a_volume_that_is_the_cell_lattice_passes_the_input_through() {
        let p = squared_gradient_interpolated(4);
        let v = Volume::new(IVec3::new(3, 1, 1), IVec3::ZERO, IVec3::new(4, 4, 4));
        assert_eq!(run(&p, &v, 0), vec![0.0, 16.0, 64.0]);
    }

    #[test]
    fn a_volume_off_the_cell_lattice_interpolates_between_corners() {
        let p = squared_gradient_interpolated(4);
        let v = Volume::dense(IVec3::new(5, 1, 1), IVec3::ZERO);
        assert_eq!(
            run(&p, &v, 0),
            vec![0.0, 4.0, 8.0, 12.0, 16.0],
            "the corners at x = 0 and x = 4 are 0 and 16, and the interior is the \
             straight line between them rather than x squared"
        );
    }

    fn top_surface(upper_bound: f32, lower_bound: i32) -> Program {
        let nodes = vec![
            gradient(Axis::Y, 0.0, 8.0, 1.0, -1.0),
            Node::Constant(upper_bound),
            Node::FindTopSurface {
                density: 0,
                upper_bound: 1,
                lower_bound,
                cell_height: 8,
            },
        ];
        program(nodes, vec![AXIS_Y, NO_AXES, NO_AXES], vec![2])
    }

    #[test]
    fn find_top_surface_descends_to_the_first_positive_density() {
        let p = top_surface(40.0, -64);
        let v = Volume::dense(IVec3::new(1, 3, 1), IVec3::ZERO);
        assert_eq!(
            run(&p, &v, 0),
            vec![0.0; 3],
            "the density is positive below y = 4 and the probe steps by 8 from y = 40"
        );
    }

    #[test]
    fn find_top_surface_stops_at_its_lower_bound() {
        let p = top_surface(-100.0, -64);
        let v = Volume::point(IVec3::ZERO);
        assert_eq!(
            run(&p, &v, 0),
            vec![-64.0],
            "flooring -100 onto the cell grid lands below the lower bound"
        );
    }

    fn program_with_ranges(
        nodes: Vec<Node>,
        axes: Vec<Axes>,
        ranges: Vec<Interval>,
        roots: Vec<NodeId>,
    ) -> Program {
        Program::new(nodes, axes, ranges, roots)
    }

    fn run_counting(program: &Program, volume: &Volume, root: usize) -> (Vec<f32>, EvalCount) {
        let mut ws = Workspace::new();
        let mut out = vec![0.0; volume.len()];
        program.fill(&mut ws, volume, root, &mut out);
        (out, ws.take_count())
    }

    /// The `when_in` subtree is reachable only through that one arm, so a fill
    /// whose selector never lands in the range must not evaluate it.
    #[test]
    fn a_range_choice_arm_no_position_selects_is_skipped() {
        let nodes = vec![
            gradient(Axis::X, 0.0, 2.0, 0.0, 10.0),
            gradient(Axis::Z, 0.0, 2.0, 0.0, 100.0),
            Node::Unary {
                op: UnaryOp::Negate,
                input: 1,
            },
            gradient(Axis::Z, 0.0, 2.0, 0.0, 5.0),
            Node::RangeChoice {
                input: 0,
                min_inclusive: 100.0,
                max_exclusive: 200.0,
                when_in: 2,
                when_out: 3,
            },
        ];
        let axes = vec![AXIS_X, AXIS_Z, AXIS_Z, AXIS_Z, AXIS_X | AXIS_Z];
        let p = program(nodes, axes, vec![4]);
        let v = Volume::dense(IVec3::new(2, 1, 2), IVec3::ZERO);
        let (out, count) = run_counting(&p, &v, 0);
        assert_eq!(
            out,
            vec![0.0, 0.0, 2.5, 2.5],
            "every position takes when_out"
        );
        assert_eq!(
            (count.evaluated, count.skipped),
            (3, 2),
            "the selector, the taken arm and the choice; the other arm's two nodes go"
        );
    }

    /// The two-armed lowering of `interval_select`, which no shipped noise
    /// settings produces: the corpus only has selects with several thresholds.
    #[test]
    fn a_single_threshold_skips_the_arm_no_position_takes() {
        let nodes = vec![
            gradient(Axis::X, 0.0, 2.0, 0.0, 10.0),
            gradient(Axis::Z, 0.0, 2.0, 0.0, 100.0),
            Node::Unary {
                op: UnaryOp::Negate,
                input: 1,
            },
            gradient(Axis::Z, 0.0, 2.0, 0.0, 5.0),
            Node::SingleThreshold {
                input: 0,
                threshold: 100.0,
                below: 2,
                above: 3,
            },
        ];
        let axes = vec![AXIS_X, AXIS_Z, AXIS_Z, AXIS_Z, AXIS_X | AXIS_Z];
        let p = program(nodes, axes, vec![4]);
        let v = Volume::dense(IVec3::new(2, 1, 2), IVec3::ZERO);
        let (out, count) = run_counting(&p, &v, 0);
        assert_eq!(out, vec![-0.0, -0.0, -50.0, -50.0]);
        assert_eq!((count.evaluated, count.skipped), (4, 1));
    }

    /// A guard nested inside a skipped cone is skipped with it, which the
    /// contiguous cone gives for free.
    #[test]
    fn a_guard_inside_a_skipped_range_never_runs() {
        let nodes = vec![
            gradient(Axis::X, 0.0, 2.0, 0.0, 10.0),
            gradient(Axis::Z, 0.0, 2.0, 0.0, 100.0),
            gradient(Axis::Z, 0.0, 2.0, 1.0, 2.0),
            gradient(Axis::Z, 0.0, 2.0, 3.0, 4.0),
            Node::RangeChoice {
                input: 1,
                min_inclusive: 0.0,
                max_exclusive: 1.0,
                when_in: 2,
                when_out: 3,
            },
            Node::Constant(9.0),
            Node::RangeChoice {
                input: 0,
                min_inclusive: 100.0,
                max_exclusive: 200.0,
                when_in: 4,
                when_out: 5,
            },
        ];
        let axes = vec![
            AXIS_X,
            AXIS_Z,
            AXIS_Z,
            AXIS_Z,
            AXIS_Z,
            NO_AXES,
            AXIS_X | AXIS_Z,
        ];
        let p = program(nodes, axes, vec![6]);
        let v = Volume::dense(IVec3::new(2, 1, 2), IVec3::ZERO);
        let (out, count) = run_counting(&p, &v, 0);
        assert_eq!(out, vec![9.0; 4], "the selector never reaches 100");
        assert_eq!(
            (count.evaluated, count.skipped),
            (3, 4),
            "only the outer selector, the constant and the choice survive"
        );
    }

    fn min_over_a_disjoint_right(right: Interval) -> Program {
        let nodes = vec![
            gradient(Axis::X, 0.0, 2.0, 0.0, 10.0),
            gradient(Axis::Z, 0.0, 2.0, 50.0, 100.0),
            Node::Affine {
                input: 1,
                scale: 2.0,
                offset: 0.0,
            },
            Node::Binary {
                op: BinaryOp::Min,
                a: 0,
                b: 2,
            },
        ];
        let axes = vec![AXIS_X, AXIS_Z, AXIS_Z, AXIS_X | AXIS_Z];
        let ranges = vec![
            Interval::of(0.0, 10.0),
            Interval::of(50.0, 100.0),
            right,
            Interval::of(0.0, 10.0),
        ];
        program_with_ranges(nodes, axes, ranges, vec![3])
    }

    /// The right operand of a `min` cannot change the result once the left run
    /// sits at or below every value the right can take.
    #[test]
    fn a_min_whose_left_run_never_reaches_the_right_skips_it() {
        let v = Volume::dense(IVec3::new(2, 1, 1), IVec3::ZERO);
        let (proved, count) = run_counting(
            &min_over_a_disjoint_right(Interval::of(100.0, 200.0)),
            &v,
            0,
        );
        let (computed, plain) = run_counting(&min_over_a_disjoint_right(Interval::INFINITE), &v, 0);
        assert_eq!(proved, computed, "the skip is value-identical");
        assert_eq!(proved, vec![0.0, 5.0]);
        assert_eq!((count.evaluated, count.skipped), (1, 3));
        assert_eq!((plain.evaluated, plain.skipped), (4, 0));
    }

    /// A zero times a right operand known to be negative is a negative zero, and
    /// the fill compares bit for bit.
    #[test]
    fn a_multiply_by_zero_keeps_the_sign_the_skipped_operand_would_have_given() {
        let nodes = vec![
            gradient(Axis::X, 0.0, 2.0, 0.0, 0.0),
            gradient(Axis::Z, 0.0, 2.0, -5.0, -3.0),
            Node::Binary {
                op: BinaryOp::Mul,
                a: 0,
                b: 1,
            },
        ];
        let axes = vec![AXIS_X, AXIS_Z, AXIS_X | AXIS_Z];
        let ranges = vec![
            Interval::exact(0.0),
            Interval::of(-5.0, -3.0),
            Interval::exact(0.0),
        ];
        let p = program_with_ranges(nodes, axes, ranges, vec![2]);
        let v = Volume::dense(IVec3::new(1, 1, 1), IVec3::ZERO);
        let (out, count) = run_counting(&p, &v, 0);
        assert!(out[0] == 0.0 && out[0].is_sign_negative());
        assert_eq!((count.evaluated, count.skipped), (1, 2));
    }

    #[test]
    fn visit_inputs_reaches_every_edge_of_a_select() {
        let node = Node::IntervalSelect {
            input: 7,
            thresholds: vec![0.0, 1.0].into(),
            arms: vec![10, 11, 12].into(),
        };
        let mut seen = Vec::new();
        node.visit_inputs(&mut |id| seen.push(id));
        assert_eq!(seen, vec![7, 10, 11, 12]);
    }
}
