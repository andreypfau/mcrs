use crate::interval::Interval;
use crate::jmath::{jmax, mul_add};
use crate::node::gradient::{GradientParams, Tiling};
use crate::program::{BinaryOp, Node, NodeId, RoundKind, UnaryOp};
use crate::volume::Axis;
use bevy_math::IVec3;

/// Which of the two bounds a walk over the graph is after. The arithmetic over
/// the operators is the same either way; only the leaves and the selections
/// differ, and they differ because the two answers are used for different
/// things.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bounds {
    /// What vanilla's `DensityFunction.range()` reports: a noise's bound is
    /// statistical and a sample may legitimately fall outside it, and a
    /// selection reports the union over its arms without consulting its input.
    /// Branch elimination consumes this one, so narrowing it deletes branches
    /// vanilla keeps.
    Declared,
    /// Rigorous over one cell, the inclusive block box given. A leaf whose
    /// value is not rigorously bounded answers `None`, which sends the cell
    /// down the per-block path, and a selection narrows to the arms its input
    /// can actually reach.
    Cell { min: IVec3, max: IVec3 },
}

/// Interval arithmetic over the operators. `at` answers for a node's inputs,
/// which are always resolved first.
pub fn node_bounds(node: &Node, mode: Bounds, at: &dyn Fn(NodeId) -> Interval) -> Option<Interval> {
    Some(match node {
        // A fold can land a NaN in a constant, and nothing is known about it.
        Node::Constant(value) if value.is_nan() => Interval::NAI,
        Node::Constant(value) => Interval::exact(*value),

        Node::Affine {
            input,
            scale,
            offset,
        } => {
            // Rounded the way the sampler rounds: a bound that fuses where the
            // sampler does not, or the other way round, lands an ulp off the
            // values it has to contain.
            let input = at(*input);
            Interval::encapsulating(
                mul_add(input.min(), *scale, *offset),
                mul_add(input.max(), *scale, *offset),
            )
        }
        Node::PiecewiseAffine {
            input,
            neg_scale,
            pos_scale,
            offset,
        } => {
            let apply = |v: f32| {
                if v < 0.0 {
                    mul_add(v, *neg_scale, *offset)
                } else {
                    mul_add(v, *pos_scale, *offset)
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
            mode,
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
            mode,
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
            match mode {
                Bounds::Declared => at(*below).union(at(*above)),
                Bounds::Cell { .. } if input.max() < *threshold => at(*below),
                Bounds::Cell { .. } if input.min() >= *threshold => at(*above),
                Bounds::Cell { .. } => at(*below).union(at(*above)),
            }
        }
        Node::IntervalSelect {
            input,
            thresholds,
            arms,
        } => {
            let reachable = match mode {
                Bounds::Declared => 0..=arms.len() - 1,
                Bounds::Cell { .. } => {
                    let input = at(*input);
                    let position = |bound: f32| {
                        thresholds
                            .iter()
                            .position(|&t| bound < t)
                            .unwrap_or(arms.len() - 1)
                    };
                    position(input.min())..=position(input.max())
                }
            };
            arms[reachable]
                .iter()
                .fold(Interval::NAI, |acc, &arm| acc.union(at(arm)))
        }

        Node::Interpolated { input, .. } => match mode {
            // The interpolant is a convex combination of its corners, so it
            // never leaves the hull of the values it reads.
            Bounds::Declared => at(*input),
            // The corner intervals are supplied from outside, so the walk is
            // never asked about a wrapper.
            Bounds::Cell { .. } => return None,
        },

        Node::Gradient(g) => match mode {
            Bounds::Declared => Interval::encapsulating(g.from_value, g.to_value),
            Bounds::Cell { min, max } => gradient_over(g, min, max),
        },

        Node::Noise { .. }
        | Node::ShiftB { .. }
        | Node::DistanceToPoint(_)
        | Node::EndOuterIslands(_)
        | Node::OldBlendedNoise(_)
        | Node::ShiftedNoise { .. }
        | Node::Spline { .. }
        | Node::FindTopSurface { .. } => match mode {
            Bounds::Declared => declared_leaf(node, at),
            Bounds::Cell { .. } => return None,
        },
    })
}

/// The bound each leaf publishes about itself, which for a noise is the
/// six-sigma estimate its parameters declare rather than anything the sampler
/// is held to.
fn declared_leaf(node: &Node, at: &dyn Fn(NodeId) -> Interval) -> Interval {
    match node {
        Node::Noise { params } | Node::ShiftedNoise { params, .. } => params.range(),
        Node::ShiftB { params } => params.range() * Interval::exact(4.0),
        Node::DistanceToPoint(_) => Interval::of(0.0, f32::INFINITY),
        Node::EndOuterIslands(_) => crate::node::end_island::range(),
        Node::OldBlendedNoise(params) => params.range(),
        Node::Spline { spline, coords } => {
            let coord_ranges: Vec<Interval> = coords.iter().map(|&id| at(id)).collect();
            spline.range(&coord_ranges)
        }
        // `jmax`, so a NaI upper bound propagates NaN into `Interval::of` and
        // aborts the load, as vanilla does.
        Node::FindTopSurface {
            upper_bound,
            lower_bound,
            ..
        } => {
            let lower = *lower_bound as f32;
            Interval::of(lower, jmax(lower, at(*upper_bound).max()))
        }
        _ => unreachable!("not a leaf"),
    }
}

/// A clamped gradient is monotone along its axis, so the box's two end
/// coordinates bound it; a repeating one may wrap inside the box, and then only
/// the hull of its two values does.
fn gradient_over(g: &GradientParams, min: IVec3, max: IVec3) -> Interval {
    if g.tiling != Tiling::ClampToEdge {
        return Interval::encapsulating(g.from_value, g.to_value);
    }
    let (lo, hi) = match g.axis {
        Axis::X => (min.x, max.x),
        Axis::Y => (min.y, max.y),
        Axis::Z => (min.z, max.z),
    };
    let mut ends = [0.0f32; 2];
    g.eval_coordinates(&mut ends, [lo, hi]);
    Interval::encapsulating(ends[0], ends[1])
}

/// Does not model the sampler's `multiple == 0.0` passthrough: an exactly
/// `[0, 0]` multiple yields NaI here while the sampler returns its input.
/// Vanilla has the same omission.
pub fn round_range(value: Interval, multiple: Interval, kind: RoundKind) -> Interval {
    (value / multiple).map_monotonic(|v| kind.apply(v)) * multiple
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
    mode: Bounds,
    input: Interval,
    min_inclusive: f32,
    max_exclusive: f32,
    when_in: Interval,
    when_out: Interval,
) -> Interval {
    if mode == Bounds::Declared {
        return when_in.union(when_out);
    }
    if input.min() >= min_inclusive && input.max() < max_exclusive {
        when_in
    } else if input.max() < min_inclusive || input.min() >= max_exclusive {
        when_out
    } else {
        when_in.union(when_out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(tiling: Tiling) -> GradientParams {
        GradientParams {
            axis: Axis::Y,
            tiling,
            from: -64.0,
            to: 320.0,
            from_value: -64.0,
            to_value: 320.0,
        }
    }

    fn over(g: &GradientParams, lo: i32, hi: i32) -> Interval {
        gradient_over(g, IVec3::new(0, lo, 0), IVec3::new(3, hi, 3))
    }

    #[test]
    fn a_clamped_gradient_is_bounded_by_the_box_ends() {
        let g = gradient(Tiling::ClampToEdge);
        assert_eq!(over(&g, 8, 15), Interval::of(8.0, 15.0));
        assert_eq!(over(&g, -100, -70), Interval::exact(-64.0));
        assert_eq!(over(&g, 300, 400), Interval::of(300.0, 320.0));
    }

    #[test]
    fn a_repeating_gradient_is_bounded_by_its_two_values() {
        for tiling in [Tiling::Repeat, Tiling::MirroredRepeat] {
            let g = gradient(tiling);
            assert_eq!(over(&g, 8, 15), Interval::of(-64.0, 320.0));
        }
    }
}
