use crate::interval::Interval;
use crate::program::{BinaryOp, Node, NodeId, Program, UnaryOp};
use crate::proto::round_range;
use bevy_math::IVec3;

/// The lattice a router with no `interpolated` node is still asked about.
const DEFAULT_CELL: IVec3 = IVec3::new(4, 8, 4);

/// One root's subgraph split at every `interpolated` node: the wrappers and
/// everything above them are bounded per cell, while their inputs are only ever
/// read off the cell lattice.
pub struct CellBounds {
    inputs: Box<[NodeId]>,
    wrappers: Box<[NodeId]>,
    /// Every reached node, ascending, wrappers included.
    terms: Box<[NodeId]>,
    /// Dense slot per node id; `NO_SLOT` for a node the walk never reached.
    slot: Box<[u32]>,
    root: NodeId,
    cell_size: Option<IVec3>,
    boundable: bool,
}

const NO_SLOT: u32 = u32::MAX;

impl CellBounds {
    pub(crate) fn new(program: &Program, root: NodeId) -> Self {
        let is_wrapper = |id: NodeId| matches!(program.node(id), Node::Interpolated { .. });

        let mut reached = vec![false; program.len()];
        let mut pending = vec![root];
        reached[root as usize] = true;
        while let Some(id) = pending.pop() {
            if is_wrapper(id) {
                continue;
            }
            program.node(id).visit_inputs(&mut |dep| {
                if !reached[dep as usize] {
                    reached[dep as usize] = true;
                    pending.push(dep);
                }
            });
        }

        let terms: Vec<NodeId> = (0..program.len() as NodeId)
            .filter(|&id| reached[id as usize])
            .collect();
        let wrappers: Vec<NodeId> = terms.iter().copied().filter(|&id| is_wrapper(id)).collect();
        let inputs: Vec<NodeId> = wrappers
            .iter()
            .map(|&id| match program.node(id) {
                Node::Interpolated { input, .. } => *input,
                _ => unreachable!("the wrapper list holds only interpolated nodes"),
            })
            .collect();

        let mut geometries = wrappers.iter().map(|&id| match program.node(id) {
            Node::Interpolated {
                cell_xz, cell_y, ..
            } => IVec3::new(*cell_xz, *cell_y, *cell_xz),
            _ => unreachable!("the wrapper list holds only interpolated nodes"),
        });
        let first = geometries.next();
        // Mixed geometries have no common lattice, so no whole-cell shortcut.
        let cell_size = geometries
            .all(|g| Some(g) == first)
            .then(|| first.unwrap_or(DEFAULT_CELL));

        let mut slot = vec![NO_SLOT; program.len()];
        for (index, &id) in terms.iter().enumerate() {
            slot[id as usize] = index as u32;
        }

        let mut bounds = Self {
            inputs: inputs.into_boxed_slice(),
            wrappers: wrappers.into_boxed_slice(),
            terms: terms.into_boxed_slice(),
            slot: slot.into_boxed_slice(),
            root,
            cell_size,
            boundable: true,
        };
        // Whether a node can be bounded depends on its kind and never on the
        // values, so one probe settles it for every cell.
        let probe = vec![Interval::exact(0.0); bounds.wrappers.len()];
        bounds.boundable = bounds.eval(program, &probe).is_some();
        bounds
    }

    /// The nodes to sample over the cell-corner lattice, in the order
    /// [`CellBounds::eval`] expects their corner intervals.
    #[inline]
    pub fn inputs(&self) -> &[NodeId] {
        &self.inputs
    }

    /// The `interpolated` nodes themselves, parallel to [`Self::inputs`].
    pub fn wrappers(&self) -> &[NodeId] {
        &self.wrappers
    }

    #[inline]
    pub fn cell_size(&self) -> Option<IVec3> {
        self.cell_size
    }

    /// Bounds on the root across a whole cell, given each `interpolated`
    /// wrapper's own bounds over the cell's eight corners. Trilinear
    /// interpolation is a convex combination, so it never leaves the corner
    /// hull; interval arithmetic over the terms above carries that up.
    ///
    /// `None` when a term has a kind this cannot bound, which simply means the
    /// caller must evaluate the cell block by block.
    pub fn eval(&self, program: &Program, corners: &[Interval]) -> Option<Interval> {
        if !self.boundable {
            return None;
        }
        assert_eq!(
            corners.len(),
            self.wrappers.len(),
            "one corner interval per interpolated wrapper"
        );
        let mut values = vec![Interval::NAI; self.terms.len()];
        for (k, &id) in self.wrappers.iter().enumerate() {
            values[self.slot[id as usize] as usize] = corners[k];
        }
        for &id in self.terms.iter() {
            let node = program.node(id);
            if matches!(node, Node::Interpolated { .. }) {
                continue;
            }
            let bound = node_bounds(node, &|dep| values[self.slot[dep as usize] as usize])?;
            values[self.slot[id as usize] as usize] = bound;
        }
        Some(values[self.slot[self.root as usize] as usize])
    }
}

/// Interval arithmetic over the operators, deliberately not sharing
/// [`crate::proto::range`]: that one reports the bound vanilla declares, which
/// for a noise is statistical and can be exceeded. Here the bound decides
/// substance for a whole cell without sampling it, so every leaf whose value is
/// not rigorously bounded answers `None` and sends the cell down the per-block
/// path instead.
fn node_bounds(node: &Node, at: &dyn Fn(NodeId) -> Interval) -> Option<Interval> {
    Some(match node {
        Node::Constant(value) => Interval::exact(*value),

        Node::Affine {
            input,
            scale,
            offset,
        } => {
            // Two roundings, matching the sampler: a fused multiply-add here
            // would place the bound an ulp off the values it must contain.
            let input = at(*input);
            Interval::encapsulating(input.min() * scale + offset, input.max() * scale + offset)
        }
        Node::PiecewiseAffine {
            input,
            neg_scale,
            pos_scale,
            offset,
        } => {
            let apply = |v: f32| {
                if v < 0.0 {
                    v * neg_scale + offset
                } else {
                    v * pos_scale + offset
                }
            };
            let input = at(*input);
            let hull = Interval::encapsulating(apply(input.min()), apply(input.max()));
            // The breakpoint is an extremum too, but only where it is reachable.
            if input.min() < 0.0 && input.max() >= 0.0 {
                hull.union_value(*offset)
            } else {
                hull
            }
        }
        Node::Unary { op, input } => unary_bounds(*op, at(*input)),
        Node::LeakyRelu {
            input,
            negative_scale,
        } => at(*input).map_monotonic(|v| if v > 0.0 { v } else { v * negative_scale }),
        Node::Clamp { input, min, max } => at(*input).clamped(*min, *max),
        Node::ConstMin { input, value } => at(*input).pointwise_min(Interval::exact(*value)),
        Node::ConstMax { input, value } => at(*input).pointwise_max(Interval::exact(*value)),
        Node::ConstSub { input, value } => Interval::exact(*value) - at(*input),
        Node::ConstDiv { input, value } => Interval::exact(*value) / at(*input),
        Node::ConstBasePow { base, exponent } => Interval::exact(*base).pow(at(*exponent)),
        Node::ConstExponentPow { input, exponent } => at(*input).pow(Interval::exact(*exponent)),
        Node::IntegerMultipleRound {
            input,
            multiple,
            kind,
        } => round_range(at(*input), Interval::exact(*multiple), *kind),

        Node::Binary { op, a, b } => binary_bounds(*op, at(*a), at(*b)),
        Node::Pow { base, exponent } => at(*base).pow(at(*exponent)),
        Node::Round {
            value,
            multiple,
            kind,
        } => round_range(at(*value), at(*multiple), *kind),

        Node::Lerp {
            alpha,
            first,
            second,
        } => Interval::lerp(at(*alpha), at(*first), at(*second)),
        Node::ConstFirstLerp {
            alpha,
            first,
            second,
        } => Interval::lerp(at(*alpha), Interval::exact(*first), at(*second)),
        Node::ConstSecondLerp {
            alpha,
            first,
            second,
        } => Interval::lerp(at(*alpha), at(*first), Interval::exact(*second)),

        Node::RangeChoice {
            input,
            min_inclusive,
            max_exclusive,
            when_in,
            when_out,
        } => range_choice_bounds(
            at(*input),
            *min_inclusive,
            *max_exclusive,
            at(*when_in),
            at(*when_out),
        ),
        Node::ConstRangeChoice {
            input,
            min_inclusive,
            max_exclusive,
            when_in,
            when_out,
        } => range_choice_bounds(
            at(*input),
            *min_inclusive,
            *max_exclusive,
            Interval::exact(*when_in),
            Interval::exact(*when_out),
        ),
        Node::SingleThreshold {
            input,
            threshold,
            below,
            above,
        } => {
            let input = at(*input);
            if input.max() < *threshold {
                at(*below)
            } else if input.min() >= *threshold {
                at(*above)
            } else {
                at(*below).union(at(*above))
            }
        }
        Node::IntervalSelect {
            input,
            thresholds,
            arms,
        } => {
            let input = at(*input);
            let first = thresholds
                .iter()
                .position(|&t| input.min() < t)
                .unwrap_or(arms.len() - 1);
            let last = thresholds
                .iter()
                .position(|&t| input.max() < t)
                .unwrap_or(arms.len() - 1);
            arms[first..=last]
                .iter()
                .fold(Interval::NAI, |acc, &arm| acc.union(at(arm)))
        }

        Node::Gradient(_)
        | Node::Noise { .. }
        | Node::ShiftB { .. }
        | Node::DistanceToPoint(_)
        | Node::EndOuterIslands(_)
        | Node::OldBlendedNoise(_)
        | Node::ShiftedNoise { .. }
        | Node::Spline { .. }
        | Node::Interpolated { .. }
        | Node::FindTopSurface { .. } => return None,
    })
}

fn unary_bounds(op: UnaryOp, input: Interval) -> Interval {
    match op {
        UnaryOp::Abs => input.abs(),
        UnaryOp::Square => input.square(),
        UnaryOp::Log => input.log(),
        UnaryOp::Sign => input.sign(),
        UnaryOp::Reciprocal => input.reciprocal(),
        UnaryOp::Negate => Interval::exact(0.0) - input,
        UnaryOp::Sqrt => input
            .pointwise_max(Interval::exact(0.0))
            .map_monotonic(|v| op.apply(v)),
        UnaryOp::Cube | UnaryOp::Squeeze => input.map_monotonic(|v| op.apply(v)),
    }
}

fn binary_bounds(op: BinaryOp, a: Interval, b: Interval) -> Interval {
    match op {
        BinaryOp::Add => a + b,
        BinaryOp::Sub => a - b,
        BinaryOp::Mul => a * b,
        BinaryOp::Div => a / b,
        BinaryOp::Min => a.pointwise_min(b),
        BinaryOp::Max => a.pointwise_max(b),
    }
}

fn range_choice_bounds(
    input: Interval,
    min_inclusive: f32,
    max_exclusive: f32,
    when_in: Interval,
    when_out: Interval,
) -> Interval {
    if input.min() >= min_inclusive && input.max() < max_exclusive {
        when_in
    } else if input.max() < min_inclusive || input.min() >= max_exclusive {
        when_out
    } else {
        when_in.union(when_out)
    }
}
