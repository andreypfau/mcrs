use crate::bounds::{Bounds, node_bounds};
use crate::branch::{self, Coverage, Fallback, GuardTest, Keep, Scope, Step};
use crate::node::distance::DistanceParams;
use crate::node::end_island::EndIslandParams;
use crate::node::gradient::GradientParams;
use crate::node::noise::NoiseFunctionParams;
use crate::node::spline::CompiledSpline;
use crate::window::{Window, element};
use core::ops::Range;
use glam::IVec3;
use mcrs_minecraft_noise::blended::BlendedNoise;
use mcrs_minecraft_noise::interval::Interval;
use mcrs_minecraft_noise::jmath;
use mcrs_minecraft_noise::kernel::{
    Runs, at, each_column, map_columns, zip2_columns, zip3_columns,
};
use mcrs_minecraft_noise::sample_grid::{Axis, SampleGrid};
use mcrs_minecraft_noise::strata::{
    ALL_AXES, AXIS_X, AXIS_Y, AXIS_Z, Axes, NO_AXES, axis_bit, extent, run_len, stratum,
};
use mcrs_minecraft_numeric as numeric;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
            UnaryOp::Sqrt => v.sqrt(),
            UnaryOp::Reciprocal => 1.0 / v,
            UnaryOp::Negate => -v,
            UnaryOp::Squeeze => {
                let c = numeric::clamp_f32(v, -1.0, 1.0);
                c / 2.0 - (c * c * c) / 24.0
            }
            UnaryOp::Log => numeric::ln_f32(v),
            UnaryOp::Sign => numeric::sign_f32(v),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Min,
    Max,
    Pow,
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
            BinaryOp::Pow => numeric::pow_f32(a, b),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

#[inline]
pub(crate) fn drops_offset(offset: f32) -> bool {
    offset == 0.0 && offset.is_sign_positive()
}

#[derive(Clone, Debug)]
pub enum Node {
    Constant(f32),
    Gradient(GradientParams),
    Noise {
        params: Arc<NoiseFunctionParams>,
    },
    ShiftB {
        params: Arc<NoiseFunctionParams>,
    },
    DistanceToPoint(DistanceParams),
    EndOuterIslands(Arc<EndIslandParams>),
    OldBlendedNoise(Arc<BlendedNoise>),

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
    Clamp {
        input: NodeId,
        min: f32,
        max: f32,
    },
    ConstBinary {
        op: BinaryOp,
        input: NodeId,
        value: f32,
        swapped: bool,
    },
    IntegerMultipleRound {
        input: NodeId,
        multiple: f32,
        kind: RoundKind,
    },

    Binary {
        op: BinaryOp,
        a: NodeId,
        b: NodeId,
    },
    Round {
        value: NodeId,
        multiple: NodeId,
        kind: RoundKind,
    },

    Lerp {
        alpha: NodeId,
        first: NodeId,
        second: NodeId,
    },
    ShiftedNoise {
        params: Arc<NoiseFunctionParams>,
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
    IntervalSelect {
        input: NodeId,
        thresholds: Arc<[f32]>,
        arms: Arc<[NodeId]>,
    },
    Spline {
        spline: Arc<CompiledSpline>,
        coords: Arc<[NodeId]>,
    },

    Interpolated {
        input: NodeId,
        cell_xz: i32,
        cell_y: i32,
        cell: usize,
    },
    Slice {
        input: NodeId,
        axis: Axis,
        coordinate: i32,
    },
    FindTopSurface {
        density: NodeId,
        upper_bound: NodeId,
        lower_bound: i32,
        cell_height: i32,
    },
}

impl Node {
    pub(crate) fn samples_per_element(&self) -> bool {
        matches!(
            self,
            Node::Noise { .. }
                | Node::ShiftedNoise { .. }
                | Node::ShiftB { .. }
                | Node::OldBlendedNoise(_)
                | Node::DistanceToPoint(_)
                | Node::EndOuterIslands(_)
                | Node::Spline { .. }
                | Node::FindTopSurface { .. }
        )
    }

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
            | Node::Clamp { input, .. }
            | Node::ConstBinary { input, .. }
            | Node::IntegerMultipleRound { input, .. }
            | Node::ConstRangeChoice { input, .. } => f(*input),

            Node::Binary { a, b, .. } => {
                f(*a);
                f(*b);
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
            Node::IntervalSelect { input, arms, .. } => {
                f(*input);
                arms.iter().copied().for_each(&mut *f);
            }
            Node::Spline { coords, .. } => coords.iter().copied().for_each(&mut *f),

            Node::Interpolated { input, .. } | Node::Slice { input, .. } => f(*input),
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

    pub(crate) fn visit_inputs_mut(&mut self, f: &mut impl FnMut(&mut NodeId)) {
        fn rewrite_all(slice: &mut Arc<[NodeId]>, f: &mut impl FnMut(&mut NodeId)) {
            let mut owned = slice.to_vec();
            owned.iter_mut().for_each(&mut *f);
            *slice = owned.into();
        }
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
            | Node::Clamp { input, .. }
            | Node::ConstBinary { input, .. }
            | Node::IntegerMultipleRound { input, .. }
            | Node::ConstRangeChoice { input, .. }
            | Node::Interpolated { input, .. }
            | Node::Slice { input, .. } => f(input),

            Node::Binary { a, b, .. } => {
                f(a);
                f(b);
            }
            Node::Round {
                value, multiple, ..
            } => {
                f(value);
                f(multiple);
            }
            Node::Lerp {
                alpha,
                first,
                second,
            } => {
                f(alpha);
                f(first);
                f(second);
            }
            Node::ShiftedNoise { x, y, z, .. } => {
                f(x);
                f(y);
                f(z);
            }
            Node::RangeChoice {
                input,
                when_in,
                when_out,
                ..
            } => {
                f(input);
                f(when_in);
                f(when_out);
            }
            Node::IntervalSelect { input, arms, .. } => {
                f(input);
                rewrite_all(arms, f);
            }
            Node::Spline { coords, .. } => rewrite_all(coords, f),
            Node::FindTopSurface {
                density,
                upper_bound,
                ..
            } => {
                f(density);
                f(upper_bound);
            }
        }
    }

    pub(crate) fn visit_local_inputs(&self, f: &mut impl FnMut(NodeId)) {
        match self {
            Node::Interpolated { .. } | Node::Slice { .. } => {}
            Node::FindTopSurface { upper_bound, .. } => f(*upper_bound),
            other => other.visit_inputs(f),
        }
    }
}

pub fn node_axes(node: &Node, axes: &[Axes]) -> Axes {
    let at = |id: NodeId| axes.get(id as usize).copied().unwrap_or(NO_AXES);
    match node {
        Node::Constant(_) => NO_AXES,
        Node::Gradient(g) => axis_bit(g.axis),
        Node::Noise { params } => params.axes(),
        Node::ShiftedNoise { params, x, y, z } => params.axes() | at(*x) | at(*y) | at(*z),
        Node::ShiftB { .. } | Node::EndOuterIslands(_) => AXIS_X | AXIS_Z,
        Node::DistanceToPoint(_) | Node::OldBlendedNoise(_) => ALL_AXES,
        Node::FindTopSurface {
            density,
            upper_bound,
            ..
        } => (at(*density) | at(*upper_bound)) & !AXIS_Y,
        Node::Interpolated { input, .. } => at(*input) | AXIS_X | AXIS_Z,
        Node::Slice { input, axis, .. } => at(*input) & !axis_bit(*axis),
        other => {
            let mut union = NO_AXES;
            other.visit_inputs(&mut |id| union |= at(id));
            union
        }
    }
}

static ZERO: Node = Node::Constant(0.0);

pub struct Program {
    nodes: Box<[Node]>,
    axes: Box<[Axes]>,
    ranges: Box<[Interval]>,
    plans: Box<[Plan]>,
    plan_of: Box<[u32]>,
    roots: Box<[NodeId]>,
    cell_count: usize,
}

struct Plan {
    order: Box<[NodeId]>,
    steps: Box<[Step]>,
    lattice: Box<[NodeId]>,
    slot: Box<[u32]>,
    given: Box<[NodeId]>,
    given_slot: Box<[u32]>,
    slot_count: [u32; 8],
}

#[derive(Clone, Copy)]
pub(crate) struct SeededPlan {
    plan: u32,
    target: NodeId,
}

pub(crate) struct Seed<'a> {
    pub plane: &'a SampleGrid,
    pub values: &'a [f32],
}

#[derive(Default)]
pub struct Workspace {
    values: Vec<f32>,
    offset: Vec<u32>,
    lattices: Vec<Lattice>,
    probe: Vec<f32>,
    sliced: Vec<f32>,
    windows: Vec<Window>,
    nested: Option<Box<Workspace>>,
    #[cfg(any(test, feature = "eval-count"))]
    count: EvalCount,
}

#[cfg(any(test, feature = "eval-count"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EvalCount {
    pub evaluated: u64,
    pub skipped: u64,
}

#[derive(Default)]
struct Lattice {
    volume: Option<SampleGrid>,
    direct: bool,
    pinned: bool,
    values: Vec<f32>,
}

impl Lattice {
    fn covers(&self, wanted: &SampleGrid) -> bool {
        let Some(held) = self.volume.filter(|_| self.pinned) else {
            return false;
        };
        held.step_block() == wanted.step_block()
            && held.min_block().cmple(wanted.min_block()).all()
            && held.max_block().cmpge(wanted.max_block()).all()
    }
}

#[expect(
    clippy::len_without_is_empty,
    reason = "len counts the nodes callers size their tables by, and nothing asks whether a program is empty"
)]
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
                Node::Slice { input, .. } => add(&mut entries, *input),
                Node::FindTopSurface {
                    density,
                    upper_bound,
                    ..
                } => {
                    add(&mut entries, *density);
                    if axes
                        .get(*upper_bound as usize)
                        .is_some_and(|&axes| axes & AXIS_Y != 0)
                    {
                        add(&mut entries, *upper_bound);
                    }
                }
                _ => {}
            }
        }

        let mut plan_of = vec![u32::MAX; nodes.len()];
        let mut plans = Vec::with_capacity(entries.len());
        for (index, &entry) in entries.iter().enumerate() {
            if let Some(plan) = plan_of.get_mut(entry as usize) {
                *plan = index as u32;
            }
            plans.push(Plan::new(&nodes, &axes, &ranges, entry, &[], &[]));
        }

        Self {
            nodes: nodes.into_boxed_slice(),
            axes: axes.into_boxed_slice(),
            ranges: ranges.into_boxed_slice(),
            plans: plans.into_boxed_slice(),
            plan_of: plan_of.into_boxed_slice(),
            roots: roots.into_boxed_slice(),
            cell_count,
        }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    #[inline]
    pub fn root_node(&self, root: usize) -> NodeId {
        self.roots.get(root).copied().unwrap_or_default()
    }

    #[inline]
    pub(crate) fn axes_of(&self, id: NodeId) -> Axes {
        self.axes.get(id as usize).copied().unwrap_or(NO_AXES)
    }

    #[inline]
    pub fn node(&self, id: NodeId) -> &Node {
        self.nodes.get(id as usize).unwrap_or(&ZERO)
    }

    pub fn fill(&self, ws: &mut Workspace, volume: &SampleGrid, root: usize, out: &mut [f32]) {
        self.fill_node(ws, volume, self.root_node(root), out);
    }

    pub fn fill_node(
        &self,
        ws: &mut Workspace,
        volume: &SampleGrid,
        target: NodeId,
        out: &mut [f32],
    ) {
        debug_assert_eq!(out.len(), volume.len());
        if self.run_plan(ws, volume, target) {
            self.copy_node(ws, volume, target, out);
        }
    }

    pub(crate) fn gather(&mut self, targets: &[NodeId]) -> Option<NodeId> {
        let (&first, rest) = targets.split_first()?;
        let mut nodes = std::mem::take(&mut self.nodes).into_vec();
        let mut axes = std::mem::take(&mut self.axes).into_vec();
        let mut ranges = std::mem::take(&mut self.ranges).into_vec();
        let mut sum = first;
        for &next in rest.iter().chain(rest.is_empty().then_some(&first)) {
            let node = Node::Binary {
                op: BinaryOp::Add,
                a: sum,
                b: next,
            };
            let range = |id: NodeId| ranges.get(id as usize).copied().unwrap_or(Interval::NAI);
            let range = node_bounds(&node, Bounds::Declared, range).unwrap_or(Interval::INFINITE);
            sum = NodeId::try_from(nodes.len()).ok()?;
            nodes.push(node);
            axes.push(ALL_AXES);
            ranges.push(range);
        }
        let plan = Plan::new(&nodes, &axes, &ranges, sum, &[], targets);
        self.nodes = nodes.into_boxed_slice();
        self.axes = axes.into_boxed_slice();
        self.ranges = ranges.into_boxed_slice();
        let mut plan_of = std::mem::take(&mut self.plan_of).into_vec();
        plan_of.resize(self.nodes.len(), u32::MAX);
        let mut plans = std::mem::take(&mut self.plans).into_vec();
        if let Some(index) = plan_of.get_mut(sum as usize) {
            *index = u32::try_from(plans.len()).ok()?;
        }
        plans.push(plan);
        self.plan_of = plan_of.into_boxed_slice();
        self.plans = plans.into_boxed_slice();
        Some(sum)
    }

    pub(crate) fn seeded_plan(&mut self, target: NodeId, given: &[NodeId]) -> Option<SeededPlan> {
        let plan = Plan::new(&self.nodes, &self.axes, &self.ranges, target, given, &[]);
        let id = SeededPlan {
            plan: u32::try_from(self.plans.len()).ok()?,
            target,
        };
        let mut plans = std::mem::take(&mut self.plans).into_vec();
        plans.push(plan);
        self.plans = plans.into_boxed_slice();
        Some(id)
    }

    pub(crate) fn fill_seeded(
        &self,
        ws: &mut Workspace,
        volume: &SampleGrid,
        seeded: SeededPlan,
        seed: &Seed<'_>,
        out: &mut [f32],
    ) -> bool {
        debug_assert_eq!(out.len(), volume.len());
        let Some(plan_ref) = self.plans.get(seeded.plan as usize) else {
            return false;
        };
        let Some(origin) = seed_origin(seed.plane, volume) else {
            return false;
        };
        ws.prepare(self, plan_ref, volume);
        self.fill_lattices(ws, volume, plan_ref);
        self.write_seed(ws, volume, plan_ref, seed, origin);
        ws.windows.clear();
        ws.windows.push(Window::whole(volume));
        self.run_steps(plan_ref, ws, volume, 0..plan_ref.steps.len(), 0, 0..1);
        self.copy_node(ws, volume, seeded.target, out);
        true
    }

    fn write_seed(
        &self,
        ws: &mut Workspace,
        volume: &SampleGrid,
        plan: &Plan,
        seed: &Seed<'_>,
        origin: IVec3,
    ) {
        let count = seed.plane.len();
        for (k, &id) in plan.given.iter().enumerate() {
            let axes = self.axes_of(id);
            let strip = stratum(axes, volume);
            let off = ws.offset_of(id);
            let row = seed
                .values
                .get(k * count..(k + 1) * count)
                .unwrap_or_default();
            for iz in 0..strip.size().z {
                for ix in 0..strip.size().x {
                    let from = seed.plane.index_unchecked(
                        if axes & AXIS_X != 0 { origin.x + ix } else { 0 },
                        0,
                        if axes & AXIS_Z != 0 { origin.z + iz } else { 0 },
                    );
                    if let (Some(slot), Some(&value)) = (
                        ws.values.get_mut(off + strip.index_unchecked(ix, 0, iz)),
                        row.get(from),
                    ) {
                        *slot = value;
                    }
                }
            }
        }
    }

    pub(crate) fn fill_gathered(
        &self,
        ws: &mut Workspace,
        volume: &SampleGrid,
        gather: NodeId,
        targets: &[NodeId],
        out: &mut [f32],
    ) {
        let n = volume.len();
        debug_assert_eq!(out.len(), targets.len() * n);
        if !self.run_plan(ws, volume, gather) {
            return;
        }
        for (&target, out) in targets.iter().zip(out.chunks_exact_mut(n)) {
            self.copy_node(ws, volume, target, out);
        }
    }

    fn run_plan(&self, ws: &mut Workspace, volume: &SampleGrid, target: NodeId) -> bool {
        let Some(plan) = self.plan_of(target) else {
            debug_assert!(false, "node {target} has no fill plan");
            return false;
        };
        ws.prepare(self, plan, volume);
        self.fill_lattices(ws, volume, plan);

        ws.windows.clear();
        ws.windows.push(Window::whole(volume));
        self.run_steps(plan, ws, volume, 0..plan.steps.len(), 0, 0..1);
        true
    }

    fn copy_node(&self, ws: &Workspace, volume: &SampleGrid, target: NodeId, out: &mut [f32]) {
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
                    let Some(column) = out.get_mut(base..base + sy) else {
                        continue;
                    };
                    if run.len() == sy {
                        column.copy_from_slice(run);
                    } else if let Some(&value) = run.first() {
                        column.fill(value);
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

    fn run_steps(
        &self,
        plan: &Plan,
        ws: &mut Workspace,
        volume: &SampleGrid,
        steps: Range<usize>,
        mut k: usize,
        active: Range<usize>,
    ) {
        let mut step = steps.start;
        while step < steps.end {
            let Some(next) = plan.steps.get(step) else {
                return;
            };
            match *next {
                Step::Eval { end } => {
                    let end = end as usize;
                    #[cfg(any(test, feature = "eval-count"))]
                    {
                        ws.count.evaluated += (end - k) as u64;
                    }
                    for &id in plan.order.get(k..end).unwrap_or_default() {
                        self.eval_over(id, ws, volume, active.clone());
                    }
                    k = end;
                    step += 1;
                }
                Step::Guard {
                    test,
                    fallback,
                    scope,
                    skip_to,
                    next_step,
                } => {
                    let kept = ws.windows.len();
                    let coverage = self.keep_windows(test, scope, ws, volume, active.clone());
                    let needed = if coverage == Coverage::Everything {
                        active.clone()
                    } else {
                        kept..ws.windows.len()
                    };
                    if coverage == Coverage::Nothing {
                        #[cfg(any(test, feature = "eval-count"))]
                        {
                            ws.count.skipped += (skip_to as usize - k) as u64;
                        }
                    } else {
                        self.run_steps(plan, ws, volume, step + 1..next_step as usize, k, needed);
                    }
                    if let Some(fallback) = fallback.filter(|_| coverage != Coverage::Everything) {
                        self.apply_fallback(fallback, test, ws, volume, active.clone());
                    }
                    ws.windows.truncate(kept);
                    k = skip_to as usize;
                    step = next_step as usize;
                }
            }
        }
    }

    fn eval_over(&self, id: NodeId, ws: &mut Workspace, volume: &SampleGrid, active: Range<usize>) {
        if !self.node(id).samples_per_element() {
            self.eval(id, ws, volume, &Window::whole(volume));
            return;
        }
        for w in active {
            let Some(&window) = ws.windows.get(w) else {
                continue;
            };
            self.eval(id, ws, volume, &window);
        }
    }

    fn keep_windows(
        &self,
        test: GuardTest,
        scope: Scope,
        ws: &mut Workspace,
        volume: &SampleGrid,
        active: Range<usize>,
    ) -> Coverage {
        let keep = Keep::new(test, |id| self.node(id));
        let selector = test.selector();
        let axes = self.axes_of(selector);
        let Workspace {
            values,
            offset,
            windows,
            ..
        } = ws;
        let off = offset.get(selector as usize).map_or(0, |&off| off as usize);
        let mut coverage = None;
        for window in windows.get(active.clone()).unwrap_or_default() {
            let start = off + window.start(axes, volume);
            let selected = values
                .get(start..start + extent(axes, &window.grid))
                .unwrap_or_default();
            let part = keep.coverage(selected, scope);
            let joined = coverage.map_or(part, |seen: Coverage| seen.and(part));
            coverage = Some(joined);
            if joined == Coverage::Part {
                break;
            }
        }
        let coverage = coverage.unwrap_or(Coverage::Nothing);
        if coverage != Coverage::Part {
            return coverage;
        }
        let column_of = Runs::new(
            values
                .get(off..off + extent(axes, volume))
                .unwrap_or_default(),
            axes,
            volume,
        );
        for w in active {
            let Some(&window) = windows.get(w) else {
                continue;
            };
            let first = windows.len();
            let mut kept_rows = 0;
            for (x, z) in window.columns() {
                keep.kept_runs(column_of.col(x, z), window.rows(), |rows| {
                    kept_rows += rows.len();
                    windows.push(Window::column(volume, x, z, rows));
                });
            }
            if kept_rows == window.grid.len() {
                windows.truncate(first);
                windows.push(window);
            }
        }
        coverage
    }

    fn plan_of(&self, target: NodeId) -> Option<&Plan> {
        let index = *self.plan_of.get(target as usize)?;
        self.plans.get(index as usize)
    }

    fn fill_lattices(&self, ws: &mut Workspace, volume: &SampleGrid, plan: &Plan) {
        if plan.lattice.is_empty() {
            return;
        }
        let mut nested = ws.nested.take().unwrap_or_default();
        for &id in &plan.lattice {
            let &Node::Interpolated {
                input,
                cell_xz,
                cell_y,
                cell,
            } = self.node(id)
            else {
                continue;
            };
            let direct = is_lattice_volume(volume, cell_xz, cell_y);
            let lattice = if direct {
                *volume
            } else {
                lattice_volume(volume, self.axes_of(id), cell_xz, cell_y)
            };
            let Some(slot) = ws.lattices.get_mut(cell) else {
                continue;
            };
            if slot.covers(&lattice) {
                continue;
            }
            slot.volume = Some(lattice);
            slot.direct = direct;
            slot.pinned = false;
            slot.values.clear();
            slot.values.resize(lattice.len(), 0.0);
            self.fill_node(&mut nested, &lattice, input, &mut slot.values);
        }
        #[cfg(any(test, feature = "eval-count"))]
        ws.absorb(&mut nested);
        ws.nested = Some(nested);
    }

    fn fill_slice(
        &self,
        ws: &mut Workspace,
        volume: &SampleGrid,
        id: NodeId,
        input: NodeId,
        axis: Axis,
        coordinate: i32,
    ) {
        let inner = slice_volume(volume, self.axes_of(input), axis, coordinate);
        let len = extent(self.axes_of(id), volume);
        debug_assert_eq!(inner.len(), len, "a slice drops exactly the pinned axis");

        let mut nested = ws.nested.take().unwrap_or_default();
        let mut sliced = std::mem::take(&mut ws.sliced);
        sliced.clear();
        sliced.resize(len, 0.0);
        self.fill_node(&mut nested, &inner, input, &mut sliced);
        #[cfg(any(test, feature = "eval-count"))]
        ws.absorb(&mut nested);
        ws.nested = Some(nested);

        let off = ws.offset_of(id);
        if let Some(out) = ws.values.get_mut(off..off + len) {
            out.copy_from_slice(&sliced);
        }
        ws.sliced = sliced;
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "independent inputs of one top-surface fill"
    )]
    fn fill_top_surface(
        &self,
        ws: &mut Workspace,
        volume: &SampleGrid,
        window: &Window,
        id: NodeId,
        density: NodeId,
        upper_bound: NodeId,
        lower_bound: i32,
        cell_height: i32,
    ) {
        let axes = self.axes_of(id);
        let ext = stratum(axes, &window.grid);
        let size = ext.size();
        debug_assert_eq!(size.y, 1, "a surface level does not vary along Y");
        let mut slot = ws.offset_of(id) + window.start(axes, volume);
        for iz in 0..size.z {
            for ix in 0..size.x {
                let value = self.find_top_surface(
                    ws,
                    volume,
                    ext.block_x(ix),
                    ext.block_z(iz),
                    window.x + ix as usize,
                    window.z + iz as usize,
                    density,
                    upper_bound,
                    lower_bound,
                    cell_height,
                );
                if let Some(out) = ws.values.get_mut(slot) {
                    *out = value;
                }
                slot += 1;
            }
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "independent inputs of one surface descent"
    )]
    fn find_top_surface(
        &self,
        ws: &mut Workspace,
        volume: &SampleGrid,
        bx: i32,
        bz: i32,
        ix: usize,
        iz: usize,
        density: NodeId,
        upper_bound: NodeId,
        lower_bound: i32,
        cell_height: i32,
    ) -> f32 {
        const BATCH: i64 = 64;

        let upper = if self.axes_of(upper_bound) & AXIS_Y == 0 {
            let buf = ws.read(self, upper_bound, volume);
            Runs::new(buf, self.axes_of(upper_bound), volume)
                .col(ix, iz)
                .first()
                .copied()
                .unwrap_or_default()
        } else {
            let mut nested = ws.nested.take().unwrap_or_default();
            let mut pinned = [0.0f32];
            let at_zero = SampleGrid::point(IVec3::new(bx, 0, bz));
            self.fill_node(&mut nested, &at_zero, upper_bound, &mut pinned);
            #[cfg(any(test, feature = "eval-count"))]
            ws.absorb(&mut nested);
            ws.nested = Some(nested);
            let [upper] = pinned;
            upper
        };

        let top_y = numeric::floor_to_i32(upper / cell_height as f32).wrapping_mul(cell_height);
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
            let span = SampleGrid::new(
                IVec3::new(1, count as i32, 1),
                IVec3::new(bx, min_y, bz),
                IVec3::new(1, cell_height, 1),
            );
            self.fill_node(&mut nested, &span, density, &mut probe);
            if let Some(k) = probe.iter().rposition(|&v| v > 0.0) {
                found = min_y + k as i32 * cell_height;
                break;
            }
            probe_y = min_y - cell_height;
        }
        #[cfg(any(test, feature = "eval-count"))]
        ws.absorb(&mut nested);
        ws.nested = Some(nested);
        ws.probe = probe;
        found as f32
    }

    fn apply_fallback(
        &self,
        fallback: Fallback,
        test: GuardTest,
        ws: &mut Workspace,
        volume: &SampleGrid,
        active: Range<usize>,
    ) {
        let keep = Keep::new(test, |id| self.node(id));
        let selector = test.selector();
        let (site_axes, source_axes, selector_axes) = (
            self.axes_of(fallback.site),
            self.axes_of(fallback.source),
            self.axes_of(selector),
        );
        let (site_off, source_off, selector_off) = (
            ws.offset_of(fallback.site),
            ws.offset_of(fallback.source),
            ws.offset_of(selector),
        );
        let Workspace {
            values, windows, ..
        } = ws;
        let negate = fallback.negate;
        let size_y = volume.size().y as usize;
        for w in active {
            let Some(window) = windows.get(w) else {
                continue;
            };
            let rows = window.rows();
            for (x, z) in window.columns() {
                let column_at = |off: usize, axes: Axes| {
                    (off + element(axes, volume, x, z, 0), run_len(axes, size_y))
                };
                let (site_at, site_len) = column_at(site_off, site_axes);
                let Some((below, rest)) = values.split_at_mut_checked(site_at) else {
                    continue;
                };
                let Some((site, above)) = rest.split_at_mut_checked(site_len) else {
                    continue;
                };
                let (below, above): (&[f32], &[f32]) = (below, above);
                let other = |(at, len): (usize, usize)| {
                    other_slot(below, above, site_at + site_len, at, len)
                };
                let selector = other(column_at(selector_off, selector_axes));
                let source = other(column_at(source_off, source_axes));
                let mut fill = |gap: Range<usize>| fill_gap(site, source, gap, negate);
                let mut gap_from = rows.start;
                keep.kept_runs(selector, rows.clone(), |kept| {
                    fill(gap_from..kept.start);
                    gap_from = kept.end;
                });
                fill(gap_from..rows.end);
            }
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one arm per node kind, in the node's declaration order, so each fill reads against its kind"
    )]
    fn eval(&self, id: NodeId, ws: &mut Workspace, volume: &SampleGrid, window: &Window) {
        let node = self.node(id);
        if let &Node::Slice {
            input,
            axis,
            coordinate,
        } = node
        {
            self.fill_slice(ws, volume, id, input, axis, coordinate);
            return;
        }
        if let &Node::FindTopSurface {
            density,
            upper_bound,
            lower_bound,
            cell_height,
        } = node
        {
            self.fill_top_surface(
                ws,
                volume,
                window,
                id,
                density,
                upper_bound,
                lower_bound,
                cell_height,
            );
            return;
        }
        let axes = self.axes_of(id);
        let grid = &window.grid;
        let ext = stratum(axes, grid);
        let off = ws.offset_of(id) + window.start(axes, volume);

        let Workspace {
            values,
            offset,
            lattices,
            ..
        } = ws;
        let len = extent(axes, grid);
        let Some((below, rest)) = values.split_at_mut_checked(off) else {
            return;
        };
        let Some((out, above)) = rest.split_at_mut_checked(len) else {
            return;
        };
        let (below, above): (&[f32], &[f32]) = (below, above);
        let end = off + len;

        let read = |j: NodeId| -> Runs<'_> {
            let j_axes = self.axes_of(j);
            let j_off = offset.get(j as usize).map_or(0, |&off| off as usize)
                + window.start(j_axes, volume);
            Runs::new(
                other_slot(below, above, end, j_off, extent(j_axes, grid)),
                j_axes,
                grid,
            )
        };

        match node {
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
                if drops_offset(o) {
                    map_columns(out, &ext, read(*input), |v| v * s)
                } else {
                    map_columns(out, &ext, read(*input), |v| jmath::mul_add(v, s, o))
                }
            }
            Node::PiecewiseAffine {
                input,
                neg_scale,
                pos_scale,
                offset: o,
            } => {
                let (n, p, o) = (*neg_scale, *pos_scale, *o);
                if drops_offset(o) {
                    map_columns(
                        out,
                        &ext,
                        read(*input),
                        |v| if v < 0.0 { v * n } else { v * p },
                    )
                } else {
                    map_columns(out, &ext, read(*input), |v| {
                        if v < 0.0 {
                            jmath::mul_add(v, n, o)
                        } else {
                            jmath::mul_add(v, p, o)
                        }
                    })
                }
            }
            Node::Unary { op, input } => {
                let op = *op;
                map_columns(out, &ext, read(*input), |v| op.apply(v))
            }
            Node::Clamp { input, min, max } => {
                let (lo, hi) = (*min, *max);
                map_columns(out, &ext, read(*input), |v| numeric::clamp_f32(v, lo, hi))
            }
            Node::ConstBinary {
                op,
                input,
                value,
                swapped,
            } => {
                let (op, c) = (*op, *value);
                if *swapped {
                    map_columns(out, &ext, read(*input), |v| op.apply(c, v))
                } else {
                    map_columns(out, &ext, read(*input), |v| op.apply(v, c))
                }
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
                        *o = arms.get(k).map_or(0.0, |&arm| at(read(arm).col(ix, iz), i));
                    }
                })
            }
            Node::Spline { spline, coords } => spline.eval(
                out,
                &|k| {
                    coords
                        .get(k)
                        .map_or_else(|| Runs::new(&[], NO_AXES, grid), |&j| read(j))
                },
                &ext,
            ),

            Node::Interpolated {
                cell_xz,
                cell_y,
                cell,
                ..
            } => {
                let Some(Lattice {
                    volume: Some(lattice),
                    direct,
                    values,
                    ..
                }) = lattices.get(*cell)
                else {
                    out.fill(0.0);
                    return;
                };
                let (cell_xz, cell_y, direct) = (*cell_xz, *cell_y, *direct);
                if direct {
                    each_column(out, &ext, |run, ix, iz| {
                        let bx = ext.block_x(ix as i32);
                        let bz = ext.block_z(iz as i32);
                        let source = lattice
                            .index_of_block(bx, ext.min_block().y, bz)
                            .and_then(|base| values.get(base..base + run.len()));
                        match source {
                            Some(source) => run.copy_from_slice(source),
                            None => run.fill(0.0),
                        }
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

            Node::Slice { .. } | Node::FindTopSurface { .. } => {}
        }
    }
}

#[inline]
fn seed_origin(plane: &SampleGrid, volume: &SampleGrid) -> Option<IVec3> {
    let step = plane.step_block();
    if step.x != volume.step_block().x || step.z != volume.step_block().z {
        return None;
    }
    let offset = volume.min_block() - plane.min_block();
    if offset.x.checked_rem(step.x)? != 0 || offset.z.checked_rem(step.z)? != 0 {
        return None;
    }
    let origin = IVec3::new(offset.x / step.x, 0, offset.z / step.z);
    let end = origin + volume.size();
    let inside =
        origin.x >= 0 && origin.z >= 0 && end.x <= plane.size().x && end.z <= plane.size().z;
    inside.then_some(origin)
}

fn fill_gap(site: &mut [f32], source: &[f32], gap: Range<usize>, negate: bool) {
    let broadcast = site.len() == 1;
    for y in gap {
        let value = at(source, y);
        if let Some(out) = site.get_mut(if broadcast { 0 } else { y }) {
            *out = if negate { -value } else { value };
        }
    }
}

fn other_slot<'a>(
    below: &'a [f32],
    above: &'a [f32],
    end: usize,
    off: usize,
    len: usize,
) -> &'a [f32] {
    if off < below.len() {
        below.get(off..off + len).unwrap_or_default()
    } else {
        off.checked_sub(end)
            .and_then(|off| above.get(off..off + len))
            .unwrap_or_default()
    }
}

fn is_lattice_volume(volume: &SampleGrid, cell_xz: i32, cell_y: i32) -> bool {
    let (size, min, step) = (volume.size(), volume.min_block(), volume.step_block());
    (step.x == cell_xz || size.x == 1)
        && (step.y == cell_y || size.y == 1)
        && (step.z == cell_xz || size.z == 1)
        && numeric::floor_mod(min.x, cell_xz) == 0
        && numeric::floor_mod(min.y, cell_y) == 0
        && numeric::floor_mod(min.z, cell_xz) == 0
}

fn slice_volume(volume: &SampleGrid, input_axes: Axes, axis: Axis, coordinate: i32) -> SampleGrid {
    let ext = stratum(input_axes, volume);
    let (mut size, mut min) = (ext.size(), ext.min_block());
    match axis {
        Axis::X => (size.x, min.x) = (1, coordinate),
        Axis::Y => (size.y, min.y) = (1, coordinate),
        Axis::Z => (size.z, min.z) = (1, coordinate),
    }
    SampleGrid::new(size, min, ext.step_block())
}

fn lattice_volume(volume: &SampleGrid, axes: Axes, cell_xz: i32, cell_y: i32) -> SampleGrid {
    let (size, min, step) = (volume.size(), volume.min_block(), volume.step_block());
    let last_y = if axes & AXIS_Y != 0 {
        min.y + (size.y - 1) * step.y
    } else {
        min.y
    };
    let cell = IVec3::new(cell_xz, cell_y, cell_xz);
    let first = IVec3::new(
        numeric::floor_div(min.x, cell_xz),
        numeric::floor_div(min.y, cell_y),
        numeric::floor_div(min.z, cell_xz),
    );
    let last = IVec3::new(
        numeric::floor_div(min.x + (size.x - 1) * step.x, cell_xz),
        numeric::floor_div(last_y, cell_y),
        numeric::floor_div(min.z + (size.z - 1) * step.z, cell_xz),
    );
    SampleGrid::new(last - first + IVec3::splat(2), first * cell, cell)
}

struct YRamp {
    value: f32,
    step: f32,
    row: i32,
}

impl YRamp {
    #[inline(always)]
    fn new(bottom: f32, step: f32, first: i32) -> Self {
        Self {
            value: jmath::mul_add(step, first as f32, bottom),
            step,
            row: first,
        }
    }

    #[inline(always)]
    fn at(&mut self, local: i32) -> f32 {
        while self.row < local {
            self.value += self.step;
            self.row += 1;
        }
        self.value
    }
}

#[inline(always)]
fn fill_rows(out: &mut [f32], bottom: f32, step: f32, first: i32) {
    let mut ramp = YRamp::new(bottom, step, first);
    for (k, slot) in out.iter_mut().enumerate() {
        *slot = ramp.at(first + k as i32);
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "independent inputs of one trilinear interpolation"
)]
fn interpolate(
    out: &mut [f32],
    bx: i32,
    bz: i32,
    ext: &SampleGrid,
    lattice: &SampleGrid,
    values: &[f32],
    cell_xz: i32,
    cell_y: i32,
) {
    let inv_xz = 1.0 / cell_xz as f32;
    let inv_y = 1.0 / cell_y as f32;
    let x_in_cell = numeric::floor_mod(bx, cell_xz);
    let z_in_cell = numeric::floor_mod(bz, cell_xz);
    let alpha_x = x_in_cell as f32 * inv_xz;
    let alpha_z = z_in_cell as f32 * inv_xz;
    let ix = (bx - x_in_cell - lattice.min_block().x) / cell_xz;
    let iz = (bz - z_in_cell - lattice.min_block().z) / cell_xz;
    let first_cell_y = numeric::floor_div(lattice.min_block().y, cell_y);
    let min_y = ext.min_block().y;
    let step_y = ext.step_block().y;

    let mut i = 0usize;
    while i < out.len() {
        let mut block_y = min_y + i as i32 * step_y;
        let cell_base = numeric::floor_div(block_y, cell_y) * cell_y;
        let iy = cell_base / cell_y - first_cell_y;
        let corner = |dx, dy, dz| {
            let index = lattice.index_unchecked(ix + dx, iy + dy, iz + dz);
            values.get(index).copied().unwrap_or_default()
        };
        let v00 = jmath::lerp(alpha_z, corner(0, 0, 0), corner(0, 0, 1));
        let v10 = jmath::lerp(alpha_z, corner(1, 0, 0), corner(1, 0, 1));
        let v01 = jmath::lerp(alpha_z, corner(0, 1, 0), corner(0, 1, 1));
        let v11 = jmath::lerp(alpha_z, corner(1, 1, 0), corner(1, 1, 1));
        let bottom = jmath::lerp(alpha_x, v00, v10);
        let top = jmath::lerp(alpha_x, v01, v11);
        let value_step = (top - bottom) * inv_y;
        let mut ramp = YRamp::new(bottom, value_step, (min_y - cell_base).max(0));

        let cell_end = cell_base + cell_y;
        while i < out.len() && block_y < cell_end {
            if let Some(slot) = out.get_mut(i) {
                *slot = ramp.at(block_y - cell_base);
            }
            i += 1;
            block_y += step_y;
        }
    }
}

fn interpolate_cells(
    out: &mut [f32],
    ext: &SampleGrid,
    lattice: &SampleGrid,
    values: &[f32],
    cell_xz: i32,
    cell_y: i32,
) {
    let inv_xz = 1.0 / cell_xz as f32;
    let inv_y = 1.0 / cell_y as f32;
    let (size, min) = (ext.size(), ext.min_block());
    let max = min + size - IVec3::ONE;
    let lattice_min = lattice.min_block();

    for cell_z in numeric::floor_div(min.z, cell_xz)..=numeric::floor_div(max.z, cell_xz) {
        let base_z = cell_z * cell_xz;
        let lz = numeric::floor_div(base_z - lattice_min.z, cell_xz);
        let dz0 = (min.z - base_z).max(0);
        let dz1 = (max.z - base_z).min(cell_xz - 1);
        for cell_x in numeric::floor_div(min.x, cell_xz)..=numeric::floor_div(max.x, cell_xz) {
            let base_x = cell_x * cell_xz;
            let lx = numeric::floor_div(base_x - lattice_min.x, cell_xz);
            let dx0 = (min.x - base_x).max(0);
            let dx1 = (max.x - base_x).min(cell_xz - 1);
            for cell_y_index in
                numeric::floor_div(min.y, cell_y)..=numeric::floor_div(max.y, cell_y)
            {
                let base_y = cell_y_index * cell_y;
                let ly = numeric::floor_div(base_y - lattice_min.y, cell_y);
                let dy0 = (min.y - base_y).max(0);
                let dy1 = (max.y - base_y).min(cell_y - 1);
                let rows = (dy1 - dy0 + 1) as usize;

                let corner = |dx, dy, dz| {
                    let index = lattice.index_unchecked(lx + dx, ly + dy, lz + dz);
                    values.get(index).copied().unwrap_or_default()
                };
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
                        let start =
                            ext.index_unchecked(base_x + dx - min.x, base_y + dy0 - min.y, out_z);
                        let run = out.get_mut(start..start + rows).unwrap_or_default();
                        fill_rows(run, bottom, value_step, dy0);
                    }
                }
            }
        }
    }
}

impl Plan {
    fn new(
        nodes: &[Node],
        axes: &[Axes],
        ranges: &[Interval],
        target: NodeId,
        given: &[NodeId],
        kept: &[NodeId],
    ) -> Self {
        let mut live = vec![false; nodes.len()];
        if let Some(live) = live.get_mut(target as usize) {
            *live = true;
        }
        for (i, node) in nodes.iter().enumerate().rev() {
            if live.get(i) == Some(&true) && !given.contains(&(i as NodeId)) {
                node.visit_local_inputs(&mut |j| {
                    if let Some(live) = live.get_mut(j as usize) {
                        *live = true;
                    }
                });
            }
        }
        let (mut order, mut lattice) = (Vec::new(), Vec::new());
        for (i, (node, &live)) in nodes.iter().zip(&live).enumerate() {
            if !live || given.contains(&(i as NodeId)) {
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
            if let Some(node) = nodes.get(id as usize) {
                node.visit_local_inputs(&mut |j| {
                    if let Some(last) = last_use.get_mut(j as usize) {
                        *last = p;
                    }
                });
            }
        }
        for &id in given.iter().chain([&target]).chain(kept) {
            if let Some(last) = last_use.get_mut(id as usize) {
                *last = usize::MAX;
            }
        }

        let axes_of = |id: NodeId| axes.get(id as usize).map_or(0, |&a| usize::from(a));
        let mut node_slot = vec![0u32; nodes.len()];
        let mut free: [Vec<u32>; 8] = Default::default();
        let mut slot_count = [0u32; 8];
        let given_slot: Vec<u32> = given
            .iter()
            .map(|&id| {
                slot_count.get_mut(axes_of(id)).map_or(0, |count| {
                    *count += 1;
                    *count - 1
                })
            })
            .collect();
        let mut slot = Vec::with_capacity(order.len());
        for (p, &id) in order.iter().enumerate() {
            let a = axes_of(id);
            let taken = match (free.get_mut(a), slot_count.get_mut(a)) {
                (Some(free), Some(count)) => free.pop().unwrap_or_else(|| {
                    *count += 1;
                    *count - 1
                }),
                _ => 0,
            };
            if let Some(node_slot) = node_slot.get_mut(id as usize) {
                *node_slot = taken;
            }
            slot.push(taken);
            let Some(node) = nodes.get(id as usize) else {
                continue;
            };
            node.visit_local_inputs(&mut |j| {
                let Some(last) = last_use.get_mut(j as usize).filter(|last| **last == p) else {
                    return;
                };
                *last = usize::MAX;
                if let (Some(free), Some(&released)) =
                    (free.get_mut(axes_of(j)), node_slot.get(j as usize))
                {
                    free.push(released);
                }
            });
        }

        Self {
            order: order.into_boxed_slice(),
            steps: steps.into_boxed_slice(),
            lattice: lattice.into_boxed_slice(),
            slot: slot.into_boxed_slice(),
            given: given.into(),
            given_slot: given_slot.into_boxed_slice(),
            slot_count,
        }
    }
}

impl Workspace {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(any(test, feature = "eval-count"))]
    pub fn take_count(&mut self) -> EvalCount {
        std::mem::take(&mut self.count)
    }

    #[cfg(any(test, feature = "eval-count"))]
    fn absorb(&mut self, nested: &mut Workspace) {
        let inner = nested.take_count();
        self.count.evaluated += inner.evaluated;
        self.count.skipped += inner.skipped;
    }

    fn prepare(&mut self, program: &Program, plan: &Plan, volume: &SampleGrid) {
        if self.offset.len() < program.len() {
            self.offset.resize(program.len(), 0);
        }

        let mut base = [0u32; 8];
        let mut len = 0usize;
        for ((base, &count), axes) in base.iter_mut().zip(&plan.slot_count).zip(0..) {
            *base = len as u32;
            len += count as usize * extent(axes, volume);
        }
        let order = plan.order.iter().chain(&plan.given);
        for (&id, &slot) in order.zip(plan.slot.iter().chain(&plan.given_slot)) {
            let axes = program.axes_of(id);
            if let (Some(offset), Some(&base)) = (
                self.offset.get_mut(id as usize),
                base.get(usize::from(axes)),
            ) {
                *offset = base + slot * extent(axes, volume) as u32;
            }
        }
        if self.values.len() < len {
            self.values.resize(len, 0.0);
        }
        if self.lattices.len() < program.cell_count {
            self.lattices
                .resize_with(program.cell_count, Lattice::default);
        }
    }

    pub(crate) fn pin_lattice(&mut self, cell: usize, volume: &SampleGrid, values: &[f32]) {
        if self.lattices.len() <= cell {
            self.lattices.resize_with(cell + 1, Lattice::default);
        }
        let Some(slot) = self.lattices.get_mut(cell) else {
            return;
        };
        slot.volume = Some(*volume);
        slot.direct = false;
        slot.pinned = true;
        slot.values.clear();
        slot.values.extend_from_slice(values);
    }

    fn read<'a>(&'a self, program: &Program, id: NodeId, volume: &SampleGrid) -> &'a [f32] {
        let off = self.offset_of(id);
        self.values
            .get(off..off + extent(program.axes_of(id), volume))
            .unwrap_or_default()
    }

    #[inline]
    fn offset_of(&self, id: NodeId) -> usize {
        self.offset.get(id as usize).map_or(0, |&off| off as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::distance::DistanceMetric;
    use crate::node::gradient::Tiling;
    use glam::IVec3;
    use mcrs_minecraft_noise::strata::{AXIS_X, AXIS_Y, AXIS_Z, NO_AXES};

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

    fn run(program: &Program, volume: &SampleGrid, root: usize) -> Vec<f32> {
        let mut ws = Workspace::new();
        let mut out = vec![0.0; volume.len()];
        program.fill(&mut ws, volume, root, &mut out);
        out
    }

    #[test]
    fn a_constant_fills_the_whole_volume() {
        let p = program(vec![Node::Constant(3.5)], vec![NO_AXES], vec![0]);
        let v = SampleGrid::dense(IVec3::new(2, 3, 2), IVec3::ZERO);
        assert_eq!(run(&p, &v, 0), vec![3.5; 12]);
    }

    #[test]
    fn a_y_only_node_is_evaluated_once_and_shared_by_every_column() {
        let p = program(
            vec![gradient(Axis::Y, 0.0, 8.0, 0.0, 8.0)],
            vec![AXIS_Y],
            vec![0],
        );
        let v = SampleGrid::dense(IVec3::new(2, 4, 2), IVec3::ZERO);
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
        let v = SampleGrid::dense(IVec3::new(2, 4, 1), IVec3::ZERO);
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
            run(&p, &SampleGrid::point(IVec3::ZERO), 0)[0].is_sign_negative(),
            "-1.0 * 0.0 is -0.0; folding in a +0.0 offset would lose the sign"
        );
    }

    fn affine_at_a_rounding_boundary() -> (f32, f32, f32, f32) {
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
        let evaluated = run(&p, &SampleGrid::point(IVec3::ZERO), 0)[0];
        (evaluated, x, scale, offset)
    }

    #[cfg(not(feature = "fast"))]
    #[test]
    fn the_strict_affine_rounds_twice_as_java_does() {
        let (evaluated, x, scale, offset) = affine_at_a_rounding_boundary();
        assert_ne!(
            x * scale + offset,
            x.mul_add(scale, offset),
            "the operands must separate the two forms"
        );
        assert_eq!(evaluated, x * scale + offset);
    }

    #[cfg(feature = "fast")]
    #[test]
    fn the_fast_affine_fuses_into_one_rounding() {
        let (evaluated, x, scale, offset) = affine_at_a_rounding_boundary();
        assert_eq!(evaluated, x.mul_add(scale, offset));
    }

    fn ramp_at_a_rounding_boundary() -> ([f32; 8], [f32; 8], f32, f32) {
        let (bottom, step) = (1.0_f32, 0.1_f32);
        let mut rows = [0.0f32; 8];
        fill_rows(&mut rows, bottom, step, 0);

        let mut accumulated = [0.0f32; 8];
        let mut value = bottom;
        for slot in accumulated.iter_mut() {
            *slot = value;
            value += step;
        }
        (rows, accumulated, bottom, step)
    }

    #[test]
    fn the_ramp_accumulates_a_row_at_a_time() {
        let (rows, accumulated, bottom, step) = ramp_at_a_rounding_boundary();
        assert_ne!(
            accumulated[7],
            jmath::mul_add(step, 7.0, bottom),
            "the operands must separate the two forms"
        );
        assert_eq!(rows, accumulated);
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
        let v = SampleGrid::dense(IVec3::new(1, 2, 2), IVec3::ZERO);
        let out = run(&p, &v, 0);
        assert_eq!(&out[0..2], &[0.0, 1.0], "z = 0");
        assert_eq!(&out[2..4], &[50.0, 51.0], "z = 1 adds 50");
    }

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
        let v = SampleGrid::new(IVec3::new(3, 1, 1), IVec3::ZERO, IVec3::new(4, 4, 4));
        assert_eq!(run(&p, &v, 0), vec![0.0, 16.0, 64.0]);
    }

    #[test]
    fn a_volume_off_the_cell_lattice_interpolates_between_corners() {
        let p = squared_gradient_interpolated(4);
        let v = SampleGrid::dense(IVec3::new(5, 1, 1), IVec3::ZERO);
        assert_eq!(
            run(&p, &v, 0),
            vec![0.0, 4.0, 8.0, 12.0, 16.0],
            "the corners at x = 0 and x = 4 are 0 and 16, and the interior is the \
             straight line between them rather than x squared"
        );
    }

    #[test]
    fn a_slice_broadcasts_the_pinned_axis_over_every_column() {
        let nodes = vec![
            gradient(Axis::X, 0.0, 4.0, 0.0, 40.0),
            gradient(Axis::Z, 0.0, 4.0, 0.0, 4.0),
            Node::Binary {
                op: BinaryOp::Add,
                a: 0,
                b: 1,
            },
            Node::Slice {
                input: 2,
                axis: Axis::X,
                coordinate: 3,
            },
        ];
        let p = program(
            nodes,
            vec![AXIS_X, AXIS_Z, AXIS_X | AXIS_Z, AXIS_Z],
            vec![3],
        );
        let v = SampleGrid::dense(IVec3::new(2, 1, 4), IVec3::ZERO);
        let out = run(&p, &v, 0);
        assert_eq!(out, vec![30.0, 30.0, 31.0, 31.0, 32.0, 32.0, 33.0, 33.0]);
    }

    #[test]
    fn a_slice_of_a_node_that_already_drops_the_axis_is_the_input() {
        let nodes = vec![
            gradient(Axis::Z, 0.0, 4.0, 0.0, 4.0),
            Node::Slice {
                input: 0,
                axis: Axis::X,
                coordinate: 100,
            },
        ];
        let p = program(nodes, vec![AXIS_Z, AXIS_Z], vec![1]);
        let v = SampleGrid::dense(IVec3::new(2, 1, 3), IVec3::ZERO);
        assert_eq!(run(&p, &v, 0), vec![0.0, 0.0, 1.0, 1.0, 2.0, 2.0]);
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
        let v = SampleGrid::dense(IVec3::new(1, 3, 1), IVec3::ZERO);
        assert_eq!(
            run(&p, &v, 0),
            vec![0.0; 3],
            "the density is positive below y = 4 and the probe steps by 8 from y = 40"
        );
    }

    #[test]
    fn find_top_surface_stops_at_its_lower_bound() {
        let p = top_surface(-100.0, -64);
        let v = SampleGrid::point(IVec3::ZERO);
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

    fn run_counting(program: &Program, volume: &SampleGrid, root: usize) -> (Vec<f32>, EvalCount) {
        let mut ws = Workspace::new();
        let mut out = vec![0.0; volume.len()];
        program.fill(&mut ws, volume, root, &mut out);
        (out, ws.take_count())
    }

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
        let v = SampleGrid::dense(IVec3::new(2, 1, 2), IVec3::ZERO);
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

    #[test]
    fn a_two_armed_select_skips_the_arm_no_position_takes() {
        let nodes = vec![
            gradient(Axis::X, 0.0, 2.0, 0.0, 10.0),
            gradient(Axis::Z, 0.0, 2.0, 0.0, 100.0),
            Node::Unary {
                op: UnaryOp::Negate,
                input: 1,
            },
            gradient(Axis::Z, 0.0, 2.0, 0.0, 5.0),
            Node::IntervalSelect {
                input: 0,
                thresholds: [100.0].into(),
                arms: [2, 3].into(),
            },
        ];
        let axes = vec![AXIS_X, AXIS_Z, AXIS_Z, AXIS_Z, AXIS_X | AXIS_Z];
        let p = program(nodes, axes, vec![4]);
        let v = SampleGrid::dense(IVec3::new(2, 1, 2), IVec3::ZERO);
        let (out, count) = run_counting(&p, &v, 0);
        assert_eq!(out, vec![-0.0, -0.0, -50.0, -50.0]);
        assert_eq!((count.evaluated, count.skipped), (4, 1));
    }

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
        let v = SampleGrid::dense(IVec3::new(2, 1, 2), IVec3::ZERO);
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

    #[test]
    fn a_min_whose_left_run_never_reaches_the_right_skips_it() {
        let v = SampleGrid::dense(IVec3::new(2, 1, 1), IVec3::ZERO);
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
        let v = SampleGrid::dense(IVec3::new(1, 1, 1), IVec3::ZERO);
        let (out, count) = run_counting(&p, &v, 0);
        assert!(out[0] == 0.0 && out[0].is_sign_negative());
        assert_eq!((count.evaluated, count.skipped), (1, 2));
    }

    #[test]
    fn a_gathered_target_keeps_its_value_until_it_is_copied() {
        let nodes = vec![
            gradient(Axis::Y, 0.0, 8.0, 1.0, 1.0),
            gradient(Axis::Y, 0.0, 8.0, 2.0, 2.0),
            gradient(Axis::Y, 0.0, 8.0, 4.0, 4.0),
        ];
        let mut p = program(nodes, vec![ALL_AXES; 3], vec![0]);
        let targets = [0, 1, 2];
        let gather = p.gather(&targets).unwrap();
        let v = SampleGrid::point(IVec3::ZERO);
        let mut out = [0.0; 3];
        p.fill_gathered(&mut Workspace::new(), &v, gather, &targets, &mut out);
        assert_eq!(out, [1.0, 2.0, 4.0]);
    }

    #[test]
    fn a_lerp_at_zero_alpha_keeps_a_first_input_that_the_second_reads() {
        let nodes = vec![
            Node::Constant(0.0),
            gradient(Axis::Y, 0.0, 4.0, 0.0, 4.0),
            Node::Affine {
                input: 1,
                scale: 2.0,
                offset: 0.0,
            },
            Node::Lerp {
                alpha: 0,
                first: 1,
                second: 2,
            },
        ];
        let axes = vec![NO_AXES, AXIS_Y, AXIS_Y, AXIS_Y];
        let p = program(nodes, axes, vec![3]);
        let v = SampleGrid::dense(IVec3::new(1, 4, 1), IVec3::ZERO);
        assert_eq!(run(&p, &v, 0), vec![0.0, 1.0, 2.0, 3.0]);
    }

    fn y_plus_three_x() -> Vec<Node> {
        vec![
            gradient(Axis::Y, 0.0, 8.0, 0.0, 8.0),
            gradient(Axis::X, 0.0, 2.0, 0.0, 6.0),
            Node::Binary {
                op: BinaryOp::Add,
                a: 0,
                b: 1,
            },
        ]
    }

    fn distance_from(y: i32) -> Node {
        Node::DistanceToPoint(DistanceParams::new(0, y, 0, DistanceMetric::Manhattan))
    }

    fn choice_inside_a_band_of_rows() -> Program {
        let mut nodes = y_plus_three_x();
        nodes.extend([
            distance_from(-10),
            Node::Constant(-1.0),
            Node::RangeChoice {
                input: 2,
                min_inclusive: 5.0,
                max_exclusive: 8.0,
                when_in: 3,
                when_out: 4,
            },
        ]);
        let axes = vec![AXIS_Y, AXIS_X, AXIS_X | AXIS_Y, ALL_AXES, NO_AXES, ALL_AXES];
        program(nodes, axes, vec![5])
    }

    fn min_reaching_its_right_in_the_top_rows() -> Program {
        let mut nodes = y_plus_three_x();
        nodes.extend([
            distance_from(-6),
            Node::Binary {
                op: BinaryOp::Min,
                a: 2,
                b: 3,
            },
        ]);
        let axes = vec![AXIS_Y, AXIS_X, AXIS_X | AXIS_Y, ALL_AXES, ALL_AXES];
        let ranges = vec![
            Interval::INFINITE,
            Interval::INFINITE,
            Interval::INFINITE,
            Interval::of(5.5, 20.0),
            Interval::INFINITE,
        ];
        program_with_ranges(nodes, axes, ranges, vec![4])
    }

    #[test]
    fn a_guard_that_holds_on_part_of_a_column_runs_its_subgraph_on_exactly_those_rows() {
        let v = SampleGrid::dense(IVec3::new(2, 8, 1), IVec3::ZERO);
        let cases: [(Program, [f32; 16]); 2] = [
            (
                choice_inside_a_band_of_rows(),
                [
                    -1.0, -1.0, -1.0, -1.0, -1.0, 15.0, 16.0, 17.0, -1.0, -1.0, 13.0, 14.0, 15.0,
                    -1.0, -1.0, -1.0,
                ],
            ),
            (
                min_reaching_its_right_in_the_top_rows(),
                [
                    0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0,
                ],
            ),
        ];
        for (p, expected) in cases {
            assert_eq!(run(&p, &v, 0), expected);
        }
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
