use super::*;

/// The cell a router falls back on when it interpolates nothing, so there is no
/// wrapper to read a lattice from.
const DEFAULT_CELL: IVec3 = IVec3::new(4, 8, 4);

fn constant_value(holder: &DensityFunctionHolder) -> Option<f32> {
    match holder {
        DensityFunctionHolder::Value(v) => Some(v.value.0 as f32),
        DensityFunctionHolder::Owned(f) => match &**f {
            ProtoDensityFunction::Constant(v) => Some(v.value.0 as f32),
            _ => None,
        },
        DensityFunctionHolder::Reference(_) => None,
    }
}

fn lower_constant_exponent(
    base: &DensityFunctionHolder,
    exponent: f32,
) -> Option<DensityFunctionHolder> {
    let owned = |function| DensityFunctionHolder::Owned(Box::new(function));
    let argument = SingleArgumentFunction {
        input: base.clone(),
    };
    let magnitude = exponent.abs();
    let lowered = if magnitude == 0.5 {
        owned(ProtoDensityFunction::Sqrt(argument))
    } else if magnitude == 1.0 {
        base.clone()
    } else if magnitude == 2.0 {
        owned(ProtoDensityFunction::Square(argument))
    } else if magnitude == 3.0 {
        owned(ProtoDensityFunction::Cube(argument))
    } else {
        return None;
    };
    Some(if exponent < 0.0 {
        owned(ProtoDensityFunction::Reciprocal(SingleArgumentFunction {
            input: lowered,
        }))
    } else {
        lowered
    })
}

/// Check if a multiply has one ClampedYGradient input.
/// Returns (gradient, other_input_index) if found.
pub(super) fn extract_mul_y_grad(
    mul: &Mul,
    stack: &[DensityFunctionComponent],
) -> Option<(ClampedYGradient, usize)> {
    if let Sampler::Independent(IndependentDensityFunction::ClampedYGradient(g)) =
        &stack[mul.input1_index].sampler
    {
        return Some((g.clone(), mul.input2_index));
    }
    if let Sampler::Independent(IndependentDensityFunction::ClampedYGradient(g)) =
        &stack[mul.input2_index].sampler
    {
        return Some((g.clone(), mul.input1_index));
    }
    None
}

/// Try to detect and build a Slide from a 5-node pattern:
///   Affine(+c) at `idx` ← Mul(ygrad2, Affine(+b) ← Mul(ygrad1, Affine(+a, input)))
pub(super) fn try_build_slide(idx: usize, stack: &[DensityFunctionComponent]) -> Option<Slide> {
    let affine_with_unit_scale = |i: usize| match &stack[i].sampler {
        Sampler::Dependent(DependentDensityFunction::Affine(a)) if a.scale == 1.0 => Some(a),
        _ => None,
    };
    let multiply = |i: usize| match &stack[i].sampler {
        Sampler::Dependent(DependentDensityFunction::Mul(m)) => Some(m),
        _ => None,
    };

    // Node at idx must be Affine with scale=1.0 (the outermost "add offset_c")
    let aff_c = affine_with_unit_scale(idx)?;

    // Its input must be a multiply with a ClampedYGradient
    let mul2 = multiply(aff_c.input_index)?;
    let (grad2, aff_b_idx) = extract_mul_y_grad(mul2, stack)?;

    // The other Mul input must be Affine with scale=1.0
    let aff_b = affine_with_unit_scale(aff_b_idx)?;

    // Its input must be a multiply with a ClampedYGradient
    let mul1 = multiply(aff_b.input_index)?;
    let (grad1, aff_a_idx) = extract_mul_y_grad(mul1, stack)?;

    // The other Mul input must be Affine with scale=1.0
    let aff_a = affine_with_unit_scale(aff_a_idx)?;

    // Both gradients must have a Y range where they saturate to 1.0
    let (g1_min, g1_max) = Slide::saturate_one_range(&grad1)?;
    let (g2_min, g2_max) = Slide::saturate_one_range(&grad2)?;

    // Intersect the two ranges
    let fast_min = g1_min.max(g2_min);
    let fast_max = g1_max.min(g2_max);
    if fast_min >= fast_max {
        return None; // No overlapping fast-path range
    }

    let combined_offset = aff_a.offset + aff_b.offset + aff_c.offset;

    Some(Slide {
        input_index: aff_a.input_index,
        grad1,
        grad2,
        offset_a: aff_a.offset,
        offset_b: aff_b.offset,
        offset_c: aff_c.offset,
        combined_offset,
        fast_path_min_y: fast_min,
        fast_path_max_y: fast_max,
    })
}

/// Choosing a sampler for an operation either collapses it onto a node that
/// already exists or produces exactly one concrete node — never a node that
/// still has to decide, at fill time, which operation it is.
#[allow(clippy::large_enum_variant)]
enum Lowering {
    Redirect(usize),
    Node(DensityFunctionComponent),
}

impl Lowering {
    fn dependent(range: Interval, function: DependentDensityFunction) -> Self {
        Lowering::Node(DensityFunctionComponent::dependent(range, function))
    }

    fn constant(value: f32) -> Self {
        Lowering::Node(DensityFunctionComponent::constant(value))
    }
}

fn lower_affine(
    stack: &[DensityFunctionComponent],
    input_index: usize,
    scale: f32,
    offset: f32,
) -> Lowering {
    let input = &stack[input_index];
    if let Some(value) = input.as_constant() {
        return Lowering::constant(value.mul_add(scale, offset));
    }
    if scale == 0.0 {
        return Lowering::constant(offset);
    }
    if scale == 1.0 && offset == 0.0 {
        return Lowering::Redirect(input_index);
    }
    Lowering::dependent(
        Affine::compute_range(input.range, scale, offset),
        DependentDensityFunction::Affine(Affine {
            input_index,
            scale,
            offset,
        }),
    )
}

/// The visitor for each one-input operation: lower it, then register it under
/// the proto of the same name.
macro_rules! unary_visitors {
    ($($visit:ident, $node:ident;)*) => {$(
        fn $visit(&mut self, arg: &SingleArgumentFunction) {
            let input_index = self.component(&arg.input);
            let lowering = lower_unary!(self.stack, $node, input_index);
            self.register_lowering(ProtoDensityFunction::$node(arg.clone()), lowering);
        }
    )*};
}

/// Fold a constant input through the operation, otherwise build the node and
/// take its bounds from the input's.
macro_rules! lower_unary {
    ($stack:expr, $node:ident, $input_index:expr) => {{
        let index = $input_index;
        let input = &$stack[index];
        match input.as_constant() {
            Some(value) => Lowering::constant($node::apply(value)),
            None => Lowering::dependent(
                $node::range(input.range),
                DependentDensityFunction::$node($node { input_index: index }),
            ),
        }
    }};
}

fn lower_leaky_relu(
    stack: &[DensityFunctionComponent],
    input_index: usize,
    negative_factor: f32,
) -> Lowering {
    let input = &stack[input_index];
    let leaky = LeakyReLU {
        input_index,
        negative_factor,
    };
    if let Some(value) = input.as_constant() {
        return Lowering::constant(leaky.apply(value));
    }
    Lowering::dependent(
        input.range.map_monotonic(|value| leaky.apply(value)),
        DependentDensityFunction::LeakyReLU(leaky),
    )
}

/// The reference picks a constant-operand sampler wherever one operand compiled
/// to a constant, so the constant is baked in rather than read from a row.
fn lower_add(
    stack: &[DensityFunctionComponent],
    left_index: usize,
    right_index: usize,
) -> Lowering {
    let (left, right) = (&stack[left_index], &stack[right_index]);
    match (left.as_constant(), right.as_constant()) {
        (Some(a), Some(b)) => Lowering::constant(a + b),
        (Some(c), None) => lower_affine(stack, right_index, 1.0, c),
        (None, Some(c)) => lower_affine(stack, left_index, 1.0, c),
        (None, None) if left_index == right_index => lower_affine(stack, left_index, 2.0, 0.0),
        (None, None) => Lowering::dependent(
            left.range + right.range,
            DependentDensityFunction::Add(Add {
                input1_index: left_index,
                input2_index: right_index,
            }),
        ),
    }
}

fn lower_sub(
    stack: &[DensityFunctionComponent],
    left_index: usize,
    right_index: usize,
) -> Lowering {
    let (left, right) = (&stack[left_index], &stack[right_index]);
    match (left.as_constant(), right.as_constant()) {
        (Some(a), Some(b)) => Lowering::constant(a - b),
        // `c - x` keeps the constant on the left, so it is its own sampler
        // rather than a scaled input.
        (Some(c), None) => Lowering::dependent(
            left.range - right.range,
            DependentDensityFunction::ConstSub(ConstSub {
                input_index: right_index,
                argument: c,
            }),
        ),
        // `x - c` is exactly `x + (-c)`: negation is exact in binary floating point.
        (None, Some(c)) => lower_affine(stack, left_index, 1.0, -c),
        (None, None) => Lowering::dependent(
            left.range - right.range,
            DependentDensityFunction::Sub(Sub {
                input1_index: left_index,
                input2_index: right_index,
            }),
        ),
    }
}

fn lower_mul(
    stack: &[DensityFunctionComponent],
    left_index: usize,
    right_index: usize,
) -> Lowering {
    let (left, right) = (&stack[left_index], &stack[right_index]);
    match (left.as_constant(), right.as_constant()) {
        (Some(a), Some(b)) => Lowering::constant(a * b),
        (Some(c), None) => lower_affine(stack, right_index, c, 0.0),
        (None, Some(c)) => lower_affine(stack, left_index, c, 0.0),
        (None, None) if left_index == right_index => Lowering::dependent(
            left.range.square(),
            DependentDensityFunction::Square(Square {
                input_index: left_index,
            }),
        ),
        (None, None) => Lowering::dependent(
            left.range * right.range,
            DependentDensityFunction::Mul(Mul {
                input1_index: left_index,
                input2_index: right_index,
            }),
        ),
    }
}

fn lower_div(
    stack: &[DensityFunctionComponent],
    left_index: usize,
    right_index: usize,
) -> Lowering {
    let (left, right) = (&stack[left_index], &stack[right_index]);
    match (left.as_constant(), right.as_constant()) {
        (Some(a), Some(b)) => Lowering::constant(a / b),
        (Some(c), None) => Lowering::dependent(
            left.range / right.range,
            DependentDensityFunction::ConstDiv(ConstDiv {
                input_index: right_index,
                argument: c,
            }),
        ),
        (None, Some(c)) => lower_affine(stack, left_index, 1.0 / c, 0.0),
        (None, None) => Lowering::dependent(
            left.range / right.range,
            DependentDensityFunction::Div(Div {
                input1_index: left_index,
                input2_index: right_index,
            }),
        ),
    }
}

/// `min` and `max` differ only in which side wins, so the redirect that drops
/// an operand whose range can never beat the other is written once.
macro_rules! extremum_lowering {
    ($name:ident, $fold:ident, $pointwise:ident, $const_variant:ident, $variant:ident, $dominates:expr) => {
        fn $name(
            stack: &[DensityFunctionComponent],
            left_index: usize,
            right_index: usize,
        ) -> Lowering {
            let (left, right) = (&stack[left_index], &stack[right_index]);
            let constants = (left.as_constant(), right.as_constant());
            if let (Some(a), Some(b)) = constants {
                return Lowering::constant(a.$fold(b));
            }
            let dominates: fn(Interval, Interval) -> bool = $dominates;
            if left_index == right_index || dominates(left.range, right.range) {
                return Lowering::Redirect(left_index);
            }
            if dominates(right.range, left.range) {
                return Lowering::Redirect(right_index);
            }
            let range = left.range.$pointwise(right.range);
            match constants {
                (Some(argument), None) => Lowering::dependent(
                    range,
                    DependentDensityFunction::$const_variant($const_variant {
                        input_index: right_index,
                        argument,
                    }),
                ),
                (None, Some(argument)) => Lowering::dependent(
                    range,
                    DependentDensityFunction::$const_variant($const_variant {
                        input_index: left_index,
                        argument,
                    }),
                ),
                _ => Lowering::dependent(
                    range,
                    DependentDensityFunction::$variant($variant {
                        input1_index: left_index,
                        input2_index: right_index,
                    }),
                ),
            }
        }
    };
}

extremum_lowering!(
    lower_min,
    min,
    pointwise_min,
    ConstMin,
    Min,
    |left, right| left.max() <= right.min()
);
extremum_lowering!(
    lower_max,
    max,
    pointwise_max,
    ConstMax,
    Max,
    |left, right| left.min() >= right.max()
);

fn lower_pow(
    stack: &[DensityFunctionComponent],
    base_index: usize,
    exponent_index: usize,
) -> Lowering {
    let (base, exponent) = (&stack[base_index], &stack[exponent_index]);
    let constants = (base.as_constant(), exponent.as_constant());
    if let (Some(a), Some(b)) = constants {
        return Lowering::constant(a.powf(b));
    }
    let range = base.range.pow(exponent.range);
    match constants {
        (Some(base), _) => Lowering::dependent(
            range,
            DependentDensityFunction::ConstBasePow(ConstBasePow {
                input_index: exponent_index,
                base,
            }),
        ),
        (None, Some(exponent)) => Lowering::dependent(
            range,
            DependentDensityFunction::ConstExponentPow(ConstExponentPow {
                input_index: base_index,
                exponent,
            }),
        ),
        (None, None) => Lowering::dependent(
            range,
            DependentDensityFunction::Pow(Pow {
                input1_index: base_index,
                input2_index: exponent_index,
            }),
        ),
    }
}

fn lower_round(
    stack: &[DensityFunctionComponent],
    mode: RoundingMode,
    input_index: usize,
    multiple_index: usize,
) -> Lowering {
    let (input, multiple) = (&stack[input_index], &stack[multiple_index]);
    let constants = (input.as_constant(), multiple.as_constant());
    if let (Some(a), Some(b)) = constants {
        return Lowering::constant(round_to_multiple(a, b, mode));
    }
    let range = round_range(input.range, multiple.range, mode);
    match constants.1 {
        Some(multiple) => Lowering::dependent(
            range,
            DependentDensityFunction::IntegerMultipleRound(IntegerMultipleRound {
                input_index,
                multiple,
                mode,
            }),
        ),
        None => Lowering::dependent(
            range,
            DependentDensityFunction::Round(Round {
                input1_index: input_index,
                input2_index: multiple_index,
                mode,
            }),
        ),
    }
}

/// A node whose only input became constant after the graph was built.
fn fold_constant_input(
    stack: &[DensityFunctionComponent],
    function: &DependentDensityFunction,
) -> Option<f32> {
    let input = |index: usize| stack[index].as_constant();
    Some(match function {
        DependentDensityFunction::PiecewiseAffine(x) => {
            let value = input(x.input_index)?;
            let scale = if value < 0.0 {
                x.neg_scale
            } else {
                x.pos_scale
            };
            value.mul_add(scale, x.offset)
        }
        DependentDensityFunction::LeakyReLU(x) => x.apply(input(x.input_index)?),
        DependentDensityFunction::ConstMin(x) => input(x.input_index)?.min(x.argument),
        DependentDensityFunction::ConstMax(x) => input(x.input_index)?.max(x.argument),
        DependentDensityFunction::ConstSub(x) => x.argument - input(x.input_index)?,
        DependentDensityFunction::ConstDiv(x) => x.argument / input(x.input_index)?,
        DependentDensityFunction::ConstExponentPow(x) => input(x.input_index)?.powf(x.exponent),
        DependentDensityFunction::ConstBasePow(x) => x.base.powf(input(x.input_index)?),
        DependentDensityFunction::IntegerMultipleRound(x) => {
            round_to_multiple(input(x.input_index)?, x.multiple, x.mode)
        }
        _ => return None,
    })
}

/// Re-run the lowering for one entry, now that everything below it is final.
/// `None` when the entry is already the node the lowering would produce.
fn relower(stack: &[DensityFunctionComponent], index: usize) -> Option<Lowering> {
    let Sampler::Dependent(function) = &stack[index].sampler else {
        return None;
    };
    let lowered = match function {
        DependentDensityFunction::Affine(x) => {
            lower_affine(stack, x.input_index, x.scale, x.offset)
        }
        DependentDensityFunction::Abs(x) => lower_unary!(stack, Abs, x.input_index),
        DependentDensityFunction::Square(x) => lower_unary!(stack, Square, x.input_index),
        DependentDensityFunction::Cube(x) => lower_unary!(stack, Cube, x.input_index),
        DependentDensityFunction::Reciprocal(x) => lower_unary!(stack, Reciprocal, x.input_index),
        DependentDensityFunction::Squeeze(x) => lower_unary!(stack, Squeeze, x.input_index),
        DependentDensityFunction::Sqrt(x) => lower_unary!(stack, Sqrt, x.input_index),
        DependentDensityFunction::Log(x) => lower_unary!(stack, Log, x.input_index),
        DependentDensityFunction::Sign(x) => lower_unary!(stack, Sign, x.input_index),
        DependentDensityFunction::Add(x) => lower_add(stack, x.input1_index, x.input2_index),
        DependentDensityFunction::Sub(x) => lower_sub(stack, x.input1_index, x.input2_index),
        DependentDensityFunction::Mul(x) => lower_mul(stack, x.input1_index, x.input2_index),
        DependentDensityFunction::Div(x) => lower_div(stack, x.input1_index, x.input2_index),
        DependentDensityFunction::Min(x) => lower_min(stack, x.input1_index, x.input2_index),
        DependentDensityFunction::Max(x) => lower_max(stack, x.input1_index, x.input2_index),
        DependentDensityFunction::Pow(x) => lower_pow(stack, x.input1_index, x.input2_index),
        DependentDensityFunction::Round(x) => {
            lower_round(stack, x.mode, x.input1_index, x.input2_index)
        }
        DependentDensityFunction::Clamp(x) => {
            let input = &stack[x.input_index];
            if let Some(value) = input.as_constant() {
                Lowering::constant(value.clamp(x.min, x.max))
            } else if input.range.min() >= x.min && input.range.max() <= x.max {
                Lowering::Redirect(x.input_index)
            } else {
                return None;
            }
        }
        DependentDensityFunction::RangeChoice(x) => {
            let input = stack[x.input_index].range;
            if input.max() < x.min_inclusion_value || input.min() >= x.max_exclusion_value {
                Lowering::Redirect(x.when_out_index)
            } else if input.min() >= x.min_inclusion_value && input.max() < x.max_exclusion_value {
                Lowering::Redirect(x.when_in_index)
            } else {
                return None;
            }
        }
        other => Lowering::constant(fold_constant_input(stack, other)?),
    };
    match &lowered {
        Lowering::Node(node) if node == &stack[index] => None,
        _ => Some(lowered),
    }
}

pub(super) fn optimize_stack(stack: &mut Vec<DensityFunctionComponent>, roots: &mut [usize]) {
    let n = stack.len();
    if n == 0 {
        return;
    }

    let mut redirect: Vec<usize> = (0..n).collect();
    let mut piecewise_affine_fusions = 0usize;
    let mut constants_folded = 0usize;
    let mut identities_eliminated = 0usize;
    let mut demotions = 0usize;
    let mut slide_fusions = 0usize;

    for i in 0..n {
        stack[i].rewrite_indices(&redirect);

        // Everything below this entry is final, so an input that only became
        // constant or only became provably in range gets its lowering now.
        match relower(stack, i) {
            Some(Lowering::Redirect(target)) => {
                redirect[i] = target;
                identities_eliminated += 1;
                continue;
            }
            Some(Lowering::Node(node)) => {
                if node.as_constant().is_some() {
                    constants_folded += 1;
                } else {
                    demotions += 1;
                }
                stack[i] = node;
            }
            None => {}
        }

        // The leaky rectifier is two half-lines through the origin, so scaling
        // and offsetting it is one node rather than two.
        if let Sampler::Dependent(DependentDensityFunction::Affine(aff)) = &stack[i].sampler {
            let (scale, offset, relu_index) = (aff.scale, aff.offset, aff.input_index);
            if let Sampler::Dependent(DependentDensityFunction::LeakyReLU(relu)) =
                &stack[relu_index].sampler
            {
                let (input_index, neg_scale) = (relu.input_index, scale * relu.negative_factor);
                let range = PiecewiseAffine::compute_range(
                    stack[input_index].range,
                    neg_scale,
                    scale,
                    offset,
                );
                stack[i] = DensityFunctionComponent::dependent(
                    range,
                    DependentDensityFunction::PiecewiseAffine(PiecewiseAffine {
                        input_index,
                        neg_scale,
                        pos_scale: scale,
                        offset,
                    }),
                );
                piecewise_affine_fusions += 1;
            }
        }

        // Fuse the world-boundary chain
        // `Affine(+c) ← Mul(ygrad2, Affine(+b) ← Mul(ygrad1, Affine(+a, input)))`.
        if let Some(slide) = try_build_slide(i, stack) {
            stack[i].sampler = Sampler::Dependent(DependentDensityFunction::Slide(slide));
            slide_fusions += 1;
        }
    }

    // Transitively resolve the redirect table
    for i in 0..n {
        let mut target = redirect[i];
        while redirect[target] != target {
            target = redirect[target];
        }
        redirect[i] = target;
    }

    for entry in stack.iter_mut() {
        entry.rewrite_indices(&redirect);
    }
    for root in roots.iter_mut() {
        *root = redirect[*root];
    }

    info!(
        stack_size = n,
        piecewise_affine_fusions,
        constants_folded,
        identities_eliminated,
        demotions,
        slide_fusions,
        "Density function stack optimized"
    );
}

/// Whether the column pass can evaluate an entry: its own value holds for the
/// whole column, so does every value it reads, and it resamples no subgraph of
/// its own.
///
/// That last clause is what lets the column cache lend its rows out across a
/// fill: an entry the column pass evaluates cannot re-enter the cache, because
/// `slice`, `find_top_surface` and `interpolated` are all excluded here.
pub(super) fn compute_column_ready(stack: &[DensityFunctionComponent]) -> Vec<bool> {
    let axes = compute_domain_axes(stack);
    let mut ready = vec![false; stack.len()];
    for i in 0..stack.len() {
        let mut i_ready = axes[i] & AXIS_Y == 0 && substituted_input(&stack[i]).is_none();
        stack[i].visit_input_indices(&mut |j| i_ready &= ready[j]);
        ready[i] = i_ready;
    }
    ready
}

/// The coordinate axes a stack entry's value can vary along. Every axis outside
/// the mask may be replaced by any value without changing the result.
pub(super) fn compute_domain_axes(stack: &[DensityFunctionComponent]) -> Vec<u8> {
    let mut axes = vec![0u8; stack.len()];

    for i in 0..stack.len() {
        let mut inputs = 0u8;
        stack[i].visit_input_indices(&mut |j| {
            debug_assert!(j < i, "stack entry {i} reads later entry {j}");
            inputs |= axes[j];
        });

        axes[i] = match &stack[i].sampler {
            Sampler::Independent(f) => match f {
                IndependentDensityFunction::Constant(_) => 0,
                IndependentDensityFunction::OldBlendedNoise(_)
                | IndependentDensityFunction::DistanceToPoint(_) => ALL_AXES,
                IndependentDensityFunction::Noise(n) => noise_scale_axes(n.xz_scale, n.y_scale),
                IndependentDensityFunction::ShiftB(_)
                | IndependentDensityFunction::EndOuterIslands(_) => AXIS_X | AXIS_Z,
                IndependentDensityFunction::ClampedYGradient(_) => AXIS_Y,
                IndependentDensityFunction::Gradient(g) => g.axis.bit(),
            },
            Sampler::Dependent(f) => match f {
                DependentDensityFunction::ShiftedNoise(n) => {
                    inputs | noise_scale_axes(n.xz_scale, n.y_scale)
                }
                DependentDensityFunction::Slide(_) => inputs | AXIS_Y,
                DependentDensityFunction::Slice(s) => inputs & !s.axes,
                // The upper bound is read at the sampled Y, so this is only sound
                // while that bound is itself Y-free; `domain_axes_are_sound` proves it.
                DependentDensityFunction::FindTopSurface(_) => inputs & !AXIS_Y,
                _ => inputs,
            },
            Sampler::Interpolated(_) => inputs,
        };
    }

    axes
}

/// Split the sub-graph feeding `root` at every `interpolated` node: the wrappers
/// themselves and everything above them evaluate per block, while their inputs
/// are only ever read off the cell lattice.
pub(super) fn compute_outer_terms(
    stack: &[DensityFunctionComponent],
    root: usize,
) -> (Vec<usize>, Vec<usize>) {
    let is_interpolated = |i: usize| matches!(&stack[i].sampler, Sampler::Interpolated(_));

    let mut reached = vec![false; stack.len()];
    let mut pending = vec![root];
    reached[root] = true;
    while let Some(i) = pending.pop() {
        if is_interpolated(i) {
            continue;
        }
        stack[i].visit_input_indices(&mut |j| {
            if !reached[j] {
                reached[j] = true;
                pending.push(j);
            }
        });
    }

    let mut terms = Vec::new();
    let mut wrappers = Vec::new();
    for i in 0..stack.len() {
        if !reached[i] {
            continue;
        }
        if is_interpolated(i) {
            wrappers.push(i);
        } else {
            terms.push(i);
        }
    }
    (terms, wrappers)
}

fn substituted_input(component: &DensityFunctionComponent) -> Option<usize> {
    match &component.sampler {
        Sampler::Dependent(DependentDensityFunction::Slice(x)) => Some(x.input_index),
        Sampler::Dependent(DependentDensityFunction::FindTopSurface(x)) => Some(x.density_index),
        Sampler::Interpolated(x) => Some(x.input_index),
        _ => None,
    }
}

/// Give every opcode that evaluates at a substituted position the ascending
/// member list of its own subgraph.
///
/// Must run after every pass that renumbers the stack — the member lists are
/// final indices and nothing rewrites them.
pub(super) fn resolve_substituted_subgraphs(
    stack: &mut [DensityFunctionComponent],
    column_ready: &[bool],
) {
    let mut subgraphs: Vec<Option<Subgraph>> = vec![None; stack.len()];
    for i in 0..stack.len() {
        if let Some(input) = substituted_input(&stack[i]) {
            let reached = branch_schedule::reachable_backwards(input, stack, input + 1);
            subgraphs[i] = Some(Subgraph::new(
                reached
                    .iter()
                    .enumerate()
                    .filter(|&(_, &hit)| hit)
                    .map(|(index, _)| index as u32)
                    .collect(),
                column_ready,
            ));
        }
    }
    for (i, subgraph) in subgraphs.into_iter().enumerate() {
        let Some(subgraph) = subgraph else { continue };
        match &mut stack[i].sampler {
            Sampler::Dependent(DependentDensityFunction::Slice(x)) => x.input = subgraph,
            Sampler::Dependent(DependentDensityFunction::FindTopSurface(x)) => x.density = subgraph,
            Sampler::Interpolated(x) => x.input = subgraph,
            _ => unreachable!(),
        }
    }
}

/// Reorder the stack into three zones the volume fill dispatches on:
///
///   Zone A `[0..column_boundary)`:  column-only entries reachable from final_density
///   Zone B `[column_boundary..fd_boundary)`: per-Y entries reachable from final_density
///   Zone C `[fd_boundary..n)`:               entries not reachable from final_density
///
/// Within each zone, topological order is maintained (children before parents).
/// Returns `(column_boundary, fd_boundary)`.
pub(super) fn reorder_stack_for_evaluation(
    stack: &mut Vec<DensityFunctionComponent>,
    column_ready: &mut Vec<bool>,
    node_labels: &mut Vec<String>,
    roots: &mut [usize],
    final_density_root_idx: usize,
) -> (usize, usize) {
    let n = stack.len();
    let fd_index = roots[final_density_root_idx];

    let mut fd_reachable = vec![false; n];
    {
        let mut worklist = vec![fd_index];
        fd_reachable[fd_index] = true;
        while let Some(idx) = worklist.pop() {
            stack[idx].visit_input_indices(&mut |input| {
                if !fd_reachable[input] {
                    fd_reachable[input] = true;
                    worklist.push(input);
                }
            });
        }
    }

    // Classify each entry into a zone
    //   Zone A (0): fd_reachable AND the column pass can evaluate it
    //   Zone B (1): fd_reachable AND it has to be re-evaluated as Y moves
    //   Zone C (2): NOT fd_reachable
    let mut zone = vec![2u8; n];
    for i in 0..n {
        if fd_reachable[i] {
            zone[i] = !column_ready[i] as u8;
        }
    }

    // The stack arrives topologically ordered, so sorting by original index
    // within a zone keeps children ahead of parents inside that zone.
    let mut sorted_indices: Vec<usize> = (0..n).collect();
    sorted_indices.sort_by_key(|&i| (zone[i], i));

    let mut old_to_new = vec![0usize; n];
    for (new_idx, &old_idx) in sorted_indices.iter().enumerate() {
        old_to_new[old_idx] = new_idx;
    }

    let old_stack: Vec<DensityFunctionComponent> = stack.drain(..).collect();
    let old_column_ready: Vec<bool> = column_ready.drain(..).collect();
    let old_labels: Vec<String> = node_labels.drain(..).collect();

    for &old_idx in &sorted_indices {
        stack.push(old_stack[old_idx].clone());
        column_ready.push(old_column_ready[old_idx]);
        node_labels.push(old_labels[old_idx].clone());
    }

    for entry in stack.iter_mut() {
        entry.rewrite_indices(&old_to_new);
    }

    for root in roots.iter_mut() {
        *root = old_to_new[*root];
    }

    let zone_a_count = zone.iter().filter(|&&z| z == 0).count();
    let zone_b_count = zone.iter().filter(|&&z| z == 1).count();
    let zone_c_count = zone.iter().filter(|&&z| z == 2).count();
    let column_boundary = zone_a_count;
    let fd_boundary = zone_a_count + zone_b_count;

    info!(
        zone_a_count,
        zone_b_count,
        zone_c_count,
        column_boundary,
        fd_boundary,
        "Stack reordered for evaluation zones"
    );

    let count_noises = |range: std::ops::Range<usize>| -> usize {
        range
            .filter(|&i| {
                matches!(
                    &stack[i].sampler,
                    Sampler::Independent(
                        IndependentDensityFunction::OldBlendedNoise(_)
                            | IndependentDensityFunction::Noise(_)
                            | IndependentDensityFunction::ShiftB(_)
                    ) | Sampler::Dependent(DependentDensityFunction::ShiftedNoise(_))
                )
            })
            .count()
    };
    info!(
        zone_a_noises = count_noises(0..column_boundary),
        zone_b_noises = count_noises(column_boundary..fd_boundary),
        zone_c_noises = count_noises(fd_boundary..n),
        "Noise evaluations per zone"
    );

    (column_boundary, fd_boundary)
}

pub fn build_functions(
    functions: &BTreeMap<ResourceLocation, ProtoDensityFunction>,
    noises: &BTreeMap<ResourceLocation, NoiseParam>,
    noise_settings: &NoiseGeneratorSettings,
    seed: u64,
    default_block_state: VoxelId,
    default_fluid_state: VoxelId,
) -> NoiseRouter {
    let random = RandomSource::new(seed, noise_settings.legacy_random_source);
    let inline = InlineReference(functions);
    let inlined: BTreeMap<ResourceLocation, ProtoDensityFunction> = functions
        .iter()
        .map(|(id, function)| {
            let function = function.rewrite_children(&inline);
            let axes = function.domain_axes();
            (
                id.clone(),
                function.rewrite_children(&SliceUniformAxes::new(axes)),
            )
        })
        .collect();
    let mut builder = FunctionStackBuilder::new(random, seed, &inlined, noises);
    let nr = &noise_settings.noise_router;
    let slice = SliceUniformAxes::new(ALL_AXES);
    let mut root = |builder: &mut FunctionStackBuilder<'_>, holder: &DensityFunctionHolder| {
        builder.component(&slice.rewrite(&inline.rewrite(holder)))
    };
    let temperature_index = root(&mut builder, &nr.temperature);
    let vegetation_index = root(&mut builder, &nr.vegetation);
    let continents_index = root(&mut builder, &nr.continents);
    let erosion_index = root(&mut builder, &nr.erosion);
    let depth_index = root(&mut builder, &nr.depth);
    let ridges_index = root(&mut builder, &nr.ridges);
    let chunk_surface_level_index = root(&mut builder, &nr.chunk_surface_level);
    let final_density_index = root(&mut builder, &nr.final_density);

    let mut roots = [
        temperature_index,
        vegetation_index,
        continents_index,
        erosion_index,
        depth_index,
        ridges_index,
        chunk_surface_level_index,
        final_density_index,
    ];

    optimize_stack(&mut builder.stack, &mut roots);

    let mut column_ready = compute_column_ready(&builder.stack);

    // Build node labels: start with type labels, then overlay reference names
    let mut node_labels: Vec<String> = vec![String::new(); builder.stack.len()];
    for (ident, proto) in builder.functions.iter() {
        if let Some(&idx) = builder.built.get(proto) {
            if idx < node_labels.len() {
                node_labels[idx] = ident.to_string();
            }
        }
    }

    // Reorder the stack into evaluation zones for optimal forward evaluation:
    //   Zone A [0..column_boundary): column-only entries reachable from final_density
    //   Zone B [column_boundary..fd_boundary): per-Y entries for final_density
    //   Zone C [fd_boundary..): entries not reachable from final_density
    let (column_boundary, fd_boundary) = reorder_stack_for_evaluation(
        &mut builder.stack,
        &mut column_ready,
        &mut node_labels,
        &mut roots,
        7, // final_density is roots[7]
    );

    let final_density_index = roots[7];
    resolve_substituted_subgraphs(&mut builder.stack, &column_ready);

    // Expose beach and surface octave noises for the Beta surface pass.
    // Only populated when using the Beta (legacy) random source; modern router gets None.
    let (beta_beach_noise, beta_surface_noise, beta_terrain_f64_opt) =
        if noise_settings.legacy_random_source {
            let (_, _, _, beach, surface, _, _) = beta_seed::seed_beta_terrain_f64(seed);
            let f64_noises = beta_terrain_f64::BetaTerrainF64::new(seed);
            (
                Some(Box::new(beach)),
                Some(Box::new(surface)),
                Some(Box::new(f64_noises)),
            )
        } else {
            (None, None, None)
        };

    let (outer_terms, outer_wrappers) = compute_outer_terms(&builder.stack, final_density_index);
    let outer_wrapper_inputs: Vec<usize> = outer_wrappers
        .iter()
        .map(|&i| match &builder.stack[i].sampler {
            Sampler::Interpolated(x) => x.input_index,
            _ => unreachable!(),
        })
        .collect();

    // Every value a caller reads back out of Zone B. The schedule may skip any
    // node outside this set, so it is also exactly what the debug verify checks.
    let zone_b_roots: Vec<usize> = std::iter::once(final_density_index)
        .chain(outer_wrapper_inputs.iter().copied())
        .collect();
    let zone_b_schedule = {
        let members: Vec<usize> = (column_boundary..=final_density_index).collect();
        branch_schedule::build(&builder.stack, &members, &zone_b_roots)
    };

    let mut geometries = outer_wrappers
        .iter()
        .map(|&i| match &builder.stack[i].sampler {
            Sampler::Interpolated(x) => IVec3::new(
                x.cell_size_xz as i32,
                x.cell_size_y as i32,
                x.cell_size_xz as i32,
            ),
            _ => unreachable!(),
        });
    let first = geometries.next();
    // Mixed geometries have no common lattice, so no whole-cell shortcut either.
    let cell_size = geometries
        .all(|g| Some(g) == first)
        .then(|| first.unwrap_or(DEFAULT_CELL));

    let router = NoiseRouter {
        temperature_index: roots[0],
        vegetation_index: roots[1],
        continents_index: roots[2],
        erosion_index: roots[3],
        depth_index: roots[4],
        ridges_index: roots[5],
        chunk_surface_level_index: roots[6],
        final_density_index,
        noise_min_y: noise_settings.noise.min_y,
        noise_height: noise_settings.noise.height,
        sea_level: noise_settings.sea_level,
        default_block_state,
        default_fluid_state,
        world_seed: seed,
        beta_beach_noise,
        beta_surface_noise,
        beta_terrain_f64: beta_terrain_f64_opt,
        column_ready: column_ready.into_boxed_slice(),
        outer_terms: outer_terms.into_boxed_slice(),
        outer_wrappers: outer_wrappers.into_boxed_slice(),
        outer_wrapper_inputs: outer_wrapper_inputs.into_boxed_slice(),
        column_boundary,
        fd_boundary,
        cell_size,
        stack: Box::from(builder.stack),
        node_labels: node_labels.into_boxed_slice(),
        zone_b_schedule,
        zone_b_roots: zone_b_roots.into_boxed_slice(),
    };

    router
}

pub(super) struct FunctionStackBuilder<'a> {
    random: RandomSource,
    world_seed: u64,
    functions: &'a BTreeMap<ResourceLocation, ProtoDensityFunction>,
    noises: &'a BTreeMap<ResourceLocation, NoiseParam>,
    stack: Vec<DensityFunctionComponent>,
    built: HashMap<ProtoDensityFunction, usize>,
}

impl<'a> FunctionStackBuilder<'a> {
    fn new(
        random: RandomSource,
        world_seed: u64,
        functions: &'a BTreeMap<ResourceLocation, ProtoDensityFunction>,
        noises: &'a BTreeMap<ResourceLocation, NoiseParam>,
    ) -> Self {
        Self {
            random,
            world_seed,
            functions,
            noises,
            stack: Vec::new(),
            built: HashMap::new(),
        }
    }
}

impl<'a> FunctionStackBuilder<'a> {
    fn get_index(&mut self, holder: &DensityFunctionHolder) -> Option<usize> {
        match holder {
            DensityFunctionHolder::Value(x) => self
                .built
                .get(&ProtoDensityFunction::Constant(x.clone()))
                .copied(),
            DensityFunctionHolder::Reference(x) => self.built.get(&self.functions[x]).copied(),
            DensityFunctionHolder::Owned(x) => self.built.get(x).copied(),
        }
    }

    fn component(&mut self, holder: &DensityFunctionHolder) -> (usize) {
        self.visit_density_function_holder(holder);
        let idx = self.get_index(holder);
        if idx.is_none() {
            panic!("Component not found after visiting: {:?}", holder);
        }
        let idx = idx.unwrap();
        idx
    }

    /// `shift` and `shift_a` are the plain noise scaled by four, so they lower
    /// into nodes that already exist instead of carrying a sampler each.
    fn lower_shift(&mut self, proto: ProtoDensityFunction, noise: &NoiseHolder, y_scale: f64) {
        let scaled = DensityFunctionHolder::Owned(Box::new(ProtoDensityFunction::Noise {
            noise: noise.clone(),
            xz_scale: 0.25.into(),
            y_scale: y_scale.into(),
            shift_x: None,
            shift_y: None,
            shift_z: None,
        }));
        let lowered = DensityFunctionHolder::Owned(Box::new(ProtoDensityFunction::Mul(
            TwoArgumentFunction {
                left: scaled,
                right: DensityFunctionHolder::Owned(Box::new(ProtoDensityFunction::Constant(
                    ConstantValue::from(4.0),
                ))),
            },
        )));
        let index = self.component(&lowered);
        self.built.insert(proto, index);
    }

    fn register_component(
        &mut self,
        proto_density_function: ProtoDensityFunction,
        component: DensityFunctionComponent,
    ) -> usize {
        let idx = self.get_index(&DensityFunctionHolder::Owned(
            proto_density_function.clone().into(),
        ));
        if let Some(index) = idx {
            return index;
        }

        idx.unwrap_or_else(|| {
            let pos = self.stack.iter().position(|c| c == &component);
            if let Some(index) = pos {
                self.built.insert(proto_density_function, index);
                return index;
            }
            let index = self.stack.len();
            self.built.insert(proto_density_function, index);
            self.stack.push(component);
            index
        })
    }
}

impl<'a> Visitor for FunctionStackBuilder<'a> {
    fn visit_constant(&mut self, value: f64) {
        self.register_component(
            ProtoDensityFunction::Constant(value.into()),
            DensityFunctionComponent::constant(value as f32),
        );
    }

    fn visit_blend_alpha(&mut self) {
        self.register_component(
            ProtoDensityFunction::BlendAlpha,
            DensityFunctionComponent::constant(1.0),
        );
    }

    fn visit_blend_offset(&mut self) {
        self.register_component(
            ProtoDensityFunction::BlendOffset,
            DensityFunctionComponent::constant(0.0),
        );
    }

    fn visit_beardifier(&mut self) {
        self.register_component(
            ProtoDensityFunction::Beardifier,
            DensityFunctionComponent::constant(0.0),
        );
    }

    fn visit_blend_density(&mut self, function: &SingleArgumentFunction) {
        let (input_index) = self.component(&function.input);
        let comp = &self.stack[input_index];
        self.register_component(
            ProtoDensityFunction::BlendDensity(SingleArgumentFunction {
                input: function.input.clone(),
            }),
            comp.clone(),
        );
    }

    /// `cache` memoizes for an engine that re-walks the graph once per position.
    /// The arena evaluates every node exactly once per fill, so the memo is
    /// already implied and the node is its input.
    fn visit_cache(&mut self, function: &SingleArgumentFunction) {
        let input_index = self.component(&function.input);
        let component = self.stack[input_index].clone();
        self.register_component(
            ProtoDensityFunction::Cache(SingleArgumentFunction {
                input: function.input.clone(),
            }),
            component,
        );
    }

    unary_visitors! {
        visit_abs, Abs;
        visit_square, Square;
        visit_cube, Cube;
        visit_reciprocal, Reciprocal;
        visit_squeeze, Squeeze;
        visit_sqrt, Sqrt;
        visit_log, Log;
        visit_sign, Sign;
    }

    fn visit_half_negative(&mut self, function: &SingleArgumentFunction) {
        self.leaky_relu(function, 0.5, ProtoDensityFunction::HalfNegative);
    }

    fn visit_quarter_negative(&mut self, function: &SingleArgumentFunction) {
        self.leaky_relu(function, 0.25, ProtoDensityFunction::QuarterNegative);
    }

    fn visit_negate(&mut self, function: &SingleArgumentFunction) {
        let input_index = self.component(&function.input);
        let lowering = lower_affine(&self.stack, input_index, -1.0, 0.0);
        self.register_lowering(
            ProtoDensityFunction::Negate(SingleArgumentFunction {
                input: function.input.clone(),
            }),
            lowering,
        );
    }

    fn visit_pow(&mut self, function: &PowFunctionArguments) {
        let proto = ProtoDensityFunction::Pow(function.clone());
        // A constant base keeps the transcendental even when the exponent is also
        // constant, so the exponent lowering only applies to a moving base.
        if constant_value(&function.base).is_none()
            && let Some(lowered) = constant_value(&function.exponent)
                .and_then(|exponent| lower_constant_exponent(&function.base, exponent))
        {
            let index = self.component(&lowered);
            self.built.insert(proto, index);
            return;
        }
        let (base, exponent) = self.operands(&function.base, &function.exponent);
        let lowering = lower_pow(&self.stack, base, exponent);
        self.register_lowering(proto, lowering);
    }

    fn visit_round(&mut self, mode: RoundingMode, function: &RoundFunctionArguments) {
        let proto = match mode {
            RoundingMode::Floor => ProtoDensityFunction::Floor(function.clone()),
            RoundingMode::Round => ProtoDensityFunction::Round(function.clone()),
            RoundingMode::Ceil => ProtoDensityFunction::Ceil(function.clone()),
            RoundingMode::Truncate => ProtoDensityFunction::Truncate(function.clone()),
        };
        let (input, multiple) = self.operands(&function.input, &function.multiple);
        let lowering = lower_round(&self.stack, mode, input, multiple);
        self.register_lowering(proto, lowering);
    }

    fn visit_add(&mut self, arg: &TwoArgumentFunction) {
        let (left, right) = self.operands(&arg.left, &arg.right);
        let lowering = lower_add(&self.stack, left, right);
        self.register_lowering(ProtoDensityFunction::Add(arg.clone()), lowering);
    }

    fn visit_sub(&mut self, function: &TwoArgumentFunction) {
        let (left, right) = self.operands(&function.left, &function.right);
        let lowering = lower_sub(&self.stack, left, right);
        self.register_lowering(ProtoDensityFunction::Sub(function.clone()), lowering);
    }

    fn visit_div(&mut self, function: &TwoArgumentFunction) {
        let (left, right) = self.operands(&function.left, &function.right);
        let lowering = lower_div(&self.stack, left, right);
        self.register_lowering(ProtoDensityFunction::Div(function.clone()), lowering);
    }

    fn visit_mul(&mut self, function: &TwoArgumentFunction) {
        let (left, right) = self.operands(&function.left, &function.right);
        let lowering = lower_mul(&self.stack, left, right);
        self.register_lowering(ProtoDensityFunction::Mul(function.clone()), lowering);
    }

    fn visit_min(&mut self, function: &TwoArgumentFunction) {
        let (left, right) = self.operands(&function.left, &function.right);
        let lowering = lower_min(&self.stack, left, right);
        self.register_lowering(ProtoDensityFunction::Min(function.clone()), lowering);
    }

    fn visit_max(&mut self, function: &TwoArgumentFunction) {
        let (left, right) = self.operands(&function.left, &function.right);
        let lowering = lower_max(&self.stack, left, right);
        self.register_lowering(ProtoDensityFunction::Max(function.clone()), lowering);
    }

    fn visit_old_blended_noise(
        &mut self,
        xz_scale: f64,
        y_scale: f64,
        xz_factor: f64,
        y_factor: f64,
        smear_scale_multiplier: f64,
    ) {
        // Legacy (Beta) mode seeds low/high/selector from LegacyRandom(world_seed)
        // exactly like ChunkProviderGenerate.java:33-35 and omits the trailing /128
        // (ChunkProviderGenerate.java:280-297 vs BlendedNoise.java:159).
        let (mut random, final_divisor) = if let RandomSource::Legacy(_) = self.random {
            (RandomSource::new(self.world_seed, true), 1.0)
        } else {
            (self.random.clone().fork_hash("minecraft:terrain"), 128.0)
        };
        let blended = BlendedNoise::new(
            &mut random,
            xz_scale as f32,
            y_scale as f32,
            xz_factor as f64,
            y_factor as f64,
            smear_scale_multiplier as f32,
            final_divisor,
        );
        self.register_component(
            ProtoDensityFunction::OldBlendedNoise {
                xz_scale: xz_scale.into(),
                y_scale: y_scale.into(),
                xz_factor: xz_factor.into(),
                y_factor: y_factor.into(),
                smear_scale_multiplier: smear_scale_multiplier.into(),
            },
            DensityFunctionComponent::independent(IndependentDensityFunction::OldBlendedNoise(
                blended,
            )),
        );
    }

    fn visit_noise(
        &mut self,
        noise_holder: &NoiseHolder,
        xz_scale: f64,
        y_scale: f64,
        shift_x: Option<&DensityFunctionHolder>,
        shift_y: Option<&DensityFunctionHolder>,
        shift_z: Option<&DensityFunctionHolder>,
    ) {
        let noise_name = Self::noise_name(noise_holder);
        let sampler = self.noise_sampler(noise_holder);
        let proto = ProtoDensityFunction::Noise {
            noise: noise_holder.clone(),
            xz_scale: xz_scale.into(),
            y_scale: y_scale.into(),
            shift_x: shift_x.cloned(),
            shift_y: shift_y.cloned(),
            shift_z: shift_z.cloned(),
        };

        if shift_x.is_none() && shift_y.is_none() && shift_z.is_none() {
            self.register_component(
                proto,
                DensityFunctionComponent::independent(IndependentDensityFunction::Noise(Noise {
                    noise_name,
                    sampler,
                    xz_scale: xz_scale as f64,
                    y_scale: y_scale as f64,
                })),
            );
            return;
        }

        let zero = DensityFunctionHolder::Value(0.0.into());
        let input_x_index = self.component(shift_x.unwrap_or(&zero));
        let input_y_index = self.component(shift_y.unwrap_or(&zero));
        let input_z_index = self.component(shift_z.unwrap_or(&zero));
        let shifted = ShiftedNoise {
            noise_name,
            input_x_index,
            input_y_index,
            input_z_index,
            xz_scale: xz_scale as f64,
            y_scale: y_scale as f64,
            sampler,
        };
        self.register_component(
            proto,
            DensityFunctionComponent::dependent(
                shifted.range(),
                DependentDensityFunction::ShiftedNoise(shifted),
            ),
        );
    }

    fn visit_range_choice(
        &mut self,
        input: &DensityFunctionHolder,
        min_inclusive: f64,
        max_exclusive: f64,
        when_in_range: &DensityFunctionHolder,
        when_out_of_range: &DensityFunctionHolder,
    ) {
        let (input_index) = self.component(input);
        let (when_in_index) = self.component(when_in_range);
        let (when_out_index) = self.component(when_out_of_range);
        let range = self.stack[when_in_index]
            .range
            .union(self.stack[when_out_index].range);
        let proto = ProtoDensityFunction::RangeChoice {
            input: input.clone(),
            min_inclusive: NoiseValue(min_inclusive),
            max_exclusive: NoiseValue(max_exclusive),
            when_in_range: when_in_range.clone(),
            when_out_of_range: when_out_of_range.clone(),
        };
        self.register_component(
            proto,
            DensityFunctionComponent::dependent(
                range,
                DependentDensityFunction::RangeChoice(RangeChoice {
                    input_index,
                    when_in_index,
                    when_out_index,
                    min_inclusion_value: min_inclusive as f32,
                    max_exclusion_value: max_exclusive as f32,
                }),
            ),
        );
    }

    fn visit_shift_a(&mut self, function: &NoiseHolder) {
        self.lower_shift(
            ProtoDensityFunction::ShiftA {
                noise: function.clone(),
            },
            function,
            0.0,
        );
    }

    fn visit_shift_b(&mut self, function: &NoiseHolder) {
        let noise_name = Self::noise_name(function);
        let sampler = self.noise_sampler(function);
        self.register_component(
            ProtoDensityFunction::ShiftB {
                noise: function.clone(),
            },
            DensityFunctionComponent::independent(IndependentDensityFunction::ShiftB(ShiftB {
                noise_name,
                sampler,
            })),
        );
    }

    fn visit_shift(&mut self, argument: &NoiseHolder) {
        self.lower_shift(
            ProtoDensityFunction::Shift {
                noise: argument.clone(),
            },
            argument,
            0.25,
        );
    }

    fn visit_end_outer_islands(&mut self) {
        self.register_component(
            ProtoDensityFunction::EndOuterIslands,
            DensityFunctionComponent::independent(IndependentDensityFunction::EndOuterIslands(
                EndIslands::new(self.world_seed),
            )),
        );
    }

    fn visit_clamp(&mut self, input: &DensityFunctionHolder, min: f64, max: f64) {
        let (input_index) = self.component(input);
        let range = self.stack[input_index]
            .range
            .clamped(min as f32, max as f32);
        let proto = ProtoDensityFunction::Clamp(ClampArguments {
            input: input.clone(),
            min: NoiseValue(min),
            max: NoiseValue(max),
        });
        self.register_component(
            proto,
            DensityFunctionComponent::dependent(
                range,
                DependentDensityFunction::Clamp(Clamp {
                    input_index,
                    min: range.min(),
                    max: range.max(),
                }),
            ),
        );
    }

    fn visit_spline(&mut self, spline: &SplineHolder) {
        let value = self.spline_value(spline);
        match value {
            SplineValue::Constant(x) => {
                self.register_component(
                    ProtoDensityFunction::Constant((x as f64).into()),
                    DensityFunctionComponent::constant(x),
                );
            }
            SplineValue::Spline(x) => {
                self.register_component(
                    ProtoDensityFunction::Spline {
                        spline: spline.clone(),
                    },
                    DensityFunctionComponent::dependent(
                        x.range,
                        DependentDensityFunction::Spline(x),
                    ),
                );
            }
        }
    }

    fn visit_gradient(
        &mut self,
        axis: Axis,
        tiling: TilingMode,
        from_coordinate: i32,
        to_coordinate: i32,
        from_value: f64,
        to_value: f64,
    ) {
        let proto = ProtoDensityFunction::Gradient(GradientArguments {
            axis,
            tiling,
            from_coordinate,
            to_coordinate,
            from_value: NoiseValue(from_value),
            to_value: NoiseValue(to_value),
        });
        let component = if axis == Axis::Y && tiling == TilingMode::ClampToEdge {
            IndependentDensityFunction::ClampedYGradient(ClampedYGradient {
                from_y: from_coordinate as f32,
                to_y: to_coordinate as f32,
                from_value: from_value as f32,
                to_value: to_value as f32,
            })
        } else {
            IndependentDensityFunction::Gradient(Gradient {
                axis,
                tiling,
                from_coordinate,
                to_coordinate,
                from_value: from_value as f32,
                to_value: to_value as f32,
            })
        };
        self.register_component(proto, DensityFunctionComponent::independent(component));
    }

    fn visit_lerp(
        &mut self,
        alpha: &DensityFunctionHolder,
        first: &DensityFunctionHolder,
        second: &DensityFunctionHolder,
    ) {
        let alpha_index = self.component(alpha);
        let first_index = self.component(first);
        let second_index = self.component(second);
        let range = Interval::lerp(
            self.stack[alpha_index].range,
            self.stack[first_index].range,
            self.stack[second_index].range,
        );
        self.register_component(
            ProtoDensityFunction::Lerp {
                alpha: alpha.clone(),
                first: first.clone(),
                second: second.clone(),
            },
            DensityFunctionComponent::dependent(
                range,
                DependentDensityFunction::Lerp(Lerp {
                    alpha_index,
                    first_index,
                    second_index,
                }),
            ),
        );
    }

    fn visit_slice(&mut self, axis: Axis, coordinate: i32, input: &DensityFunctionHolder) {
        let input_index = self.component(input);
        let proto = ProtoDensityFunction::Slice {
            axis,
            coordinate,
            input: input.clone(),
        };
        if input.domain_axes() & axis.bit() == 0 {
            let component = self.stack[input_index].clone();
            self.register_component(proto, component);
            return;
        }
        let range = self.stack[input_index].range;
        let mut slice = Slice {
            axes: axis.bit(),
            coordinate: IVec3::splat(coordinate),
            input_index,
            input: Subgraph::default(),
        };
        // A slice of a slice pins both axes at once, and the inner coordinate
        // wins wherever the two name the same one: the inner substitution is
        // the one applied last.
        if let Sampler::Dependent(DependentDensityFunction::Slice(inner)) =
            &self.stack[input_index].sampler
        {
            slice.coordinate = inner.pin(slice.coordinate);
            slice.axes |= inner.axes;
            slice.input_index = inner.input_index;
        }
        self.register_component(
            proto,
            DensityFunctionComponent::dependent(
                range,
                DependentDensityFunction::Slice(slice),
            ),
        );
    }

    fn visit_distance_to_point(&mut self, point: [i32; 3], metric: DistanceMetric) {
        self.register_component(
            ProtoDensityFunction::DistanceToPoint { point, metric },
            DensityFunctionComponent::independent(IndependentDensityFunction::DistanceToPoint(
                DistanceToPoint {
                    point: IVec3::new(point[0], point[1], point[2]),
                    metric,
                },
            )),
        );
    }

    fn visit_interval_select(
        &mut self,
        input: &DensityFunctionHolder,
        thresholds: &[NoiseValue],
        functions: &[DensityFunctionHolder],
    ) {
        let mut lowered = functions[functions.len() - 1].clone();
        for (threshold, function) in thresholds.iter().zip(functions).rev() {
            lowered = DensityFunctionHolder::Owned(Box::new(ProtoDensityFunction::RangeChoice {
                input: input.clone(),
                min_inclusive: NoiseValue(f64::NEG_INFINITY),
                max_exclusive: *threshold,
                when_in_range: function.clone(),
                when_out_of_range: lowered,
            }));
        }
        let index = self.component(&lowered);
        self.built.insert(
            ProtoDensityFunction::IntervalSelect(IntervalSelectArguments {
                input: input.clone(),
                thresholds: thresholds.to_vec(),
                functions: functions.to_vec(),
            }),
            index,
        );
    }

    fn visit_find_top_surface(
        &mut self,
        density: &DensityFunctionHolder,
        upper_bound: &DensityFunctionHolder,
        lower_bound: i32,
        cell_height: std::num::NonZeroU32,
    ) {
        let (density_index) = self.component(density);
        let (upper_bound_index) = self.component(upper_bound);
        let range = Interval::of(
            lower_bound as f32,
            self.stack[upper_bound_index]
                .range
                .max()
                .max(lower_bound as f32),
        );
        let proto = ProtoDensityFunction::FindTopSurface {
            density: density.clone(),
            upper_bound: upper_bound.clone(),
            lower_bound,
            cell_height,
        };
        self.register_component(
            proto,
            DensityFunctionComponent::dependent(
                range,
                DependentDensityFunction::FindTopSurface(FindTopSurface {
                    density_index,
                    density: Subgraph::default(),
                    upper_bound_index,
                    lower_bound: lower_bound as f32,
                    cell_height: cell_height.get() as f32,
                }),
            ),
        );
    }

    fn visit_interpolated(
        &mut self,
        input: &DensityFunctionHolder,
        cell_size_xz: std::num::NonZeroU32,
        cell_size_y: std::num::NonZeroU32,
    ) {
        let input_index = self.component(input);
        let range = self.stack[input_index].range;

        self.register_component(
            ProtoDensityFunction::Interpolated {
                input: input.clone(),
                cell_size_xz,
                cell_size_y,
            },
            DensityFunctionComponent::new(
                range,
                Sampler::Interpolated(Interpolated {
                    input_index,
                    input: Subgraph::default(),
                    cell_size_xz: cell_size_xz.get(),
                    cell_size_y: cell_size_y.get(),
                    cell_size_xz_inv: 1.0 / cell_size_xz.get() as f32,
                    cell_size_y_inv: 1.0 / cell_size_y.get() as f32,
                }),
            ),
        );
    }
}

impl<'a> FunctionStackBuilder<'a> {
    fn spline_value(&mut self, spline_holder: &SplineHolder) -> SplineValue {
        match spline_holder {
            SplineHolder::Constant(x) => SplineValue::Constant(x.0 as f32),
            SplineHolder::Spline(x) => SplineValue::Spline(self.spline(x)),
        }
    }

    fn spline(&mut self, proto_spline: &proto::Spline) -> Spline {
        let (cord_index) = self.component(&proto_spline.coordinate);
        let coordinate = self.stack[cord_index].range;
        let mut values = Vec::with_capacity(proto_spline.points.len());
        let mut derivatives = Vec::with_capacity(proto_spline.points.len());
        let mut locations = Vec::with_capacity(proto_spline.points.len());
        for p in &proto_spline.points {
            values.push(self.spline_value(&p.value));
            derivatives.push(p.derivative.0 as f32);
            locations.push(p.location.0 as f32);
        }
        Spline::new(cord_index, coordinate, locations, derivatives, values)
    }

    /// A lowering that collapsed onto an existing entry gives this function's
    /// proto that entry, rather than a node of its own.
    fn register_lowering(&mut self, proto: ProtoDensityFunction, lowering: Lowering) {
        match lowering {
            Lowering::Redirect(index) => {
                self.built.insert(proto, index);
            }
            Lowering::Node(component) => {
                self.register_component(proto, component);
            }
        }
    }

    fn operands(
        &mut self,
        left: &DensityFunctionHolder,
        right: &DensityFunctionHolder,
    ) -> (usize, usize) {
        (self.component(left), self.component(right))
    }

    fn leaky_relu(
        &mut self,
        arg: &SingleArgumentFunction,
        negative_factor: f32,
        proto: fn(SingleArgumentFunction) -> ProtoDensityFunction,
    ) {
        let input_index = self.component(&arg.input);
        let lowering = lower_leaky_relu(&self.stack, input_index, negative_factor);
        self.register_lowering(proto(arg.clone()), lowering);
    }

    fn noise_name(holder: &NoiseHolder) -> String {
        match holder {
            NoiseHolder::Reference(x) => x
                .as_str()
                .strip_prefix("minecraft:")
                .unwrap_or(x.as_str())
                .to_string(),
            NoiseHolder::Owned(_) => "inline".to_string(),
        }
    }

    fn noise_sampler(&mut self, holder: &NoiseHolder) -> NoiseSampler {
        match holder {
            NoiseHolder::Reference(x) => self.create_noise(x),
            NoiseHolder::Owned(x) => Self::from_noise_param(&mut self.random.clone(), x),
        }
    }

    fn from_noise_param<R: mcrs_minecraft_random::Random>(
        random: &mut R,
        param: &NoiseParam,
    ) -> NoiseSampler {
        NoiseSampler::from_params(
            random,
            param.base_octave,
            param.octave_amplitudes(),
            param.base_amplitude.0,
            param.normalize,
        )
    }

    fn create_noise(&mut self, id: &ResourceLocation) -> NoiseSampler {
        match id.as_str() {
            "minecraft:nether/temperature" => {
                return NoiseSampler::new(
                    &mut LegacyRandom::new(self.world_seed),
                    -7,
                    vec![1.0, 1.0],
                );
            }
            "minecraft:nether/vegetation" => {
                return NoiseSampler::new(
                    &mut LegacyRandom::new(self.world_seed.wrapping_add(1)),
                    -7,
                    vec![1.0, 1.0],
                );
            }
            _ => {}
        }

        if let RandomSource::Legacy(_) = &self.random {
            match id.as_str() {
                "minecraft:offset" => {
                    return NoiseSampler::new(
                        &mut self.random.clone().fork_hash("minecraft:offset"),
                        0,
                        vec![0.0],
                    );
                }
                // Beta terrain 2D noises: elements 3 and 4 of the sequential
                // seed_beta_terrain stream, sampled at noise-cell coords with their
                // Java frequency constants (1.121 scale, 200.0 depth). Bounds match
                // sample_xz: |acc| <= A * (2^octaves - 1) with per-octave |s| ~ 2.
                "mcrs:beta/scale" => {
                    let (_, _, _, _, _, scale_noise, _) =
                        beta_seed::seed_beta_terrain(self.world_seed);
                    return NoiseSampler::beta_octave_2d(scale_noise, 1.121, 2048.0);
                }
                "mcrs:beta/depth" => {
                    let (_, _, _, _, _, _, depth_noise) =
                        beta_seed::seed_beta_terrain(self.world_seed);
                    return NoiseSampler::beta_octave_2d(depth_noise, 200.0, 131072.0);
                }
                // Beta climate simplex noises: three independent LegacyRandom streams
                // (WorldChunkManager.java lines 18-20). Frequency constants are
                // id-intrinsic (JSON samples with xz_scale=1.0). Post-processing
                // lives in minecraft:beta/{temperature,vegetation,climate_detail}.
                "mcrs:beta/temperature" => {
                    let (temp_noise, _, _) = beta_seed::seed_beta_climate(self.world_seed);
                    return NoiseSampler::beta_simplex_2d(temp_noise, 0.025, 0.25, 16.0);
                }
                "mcrs:beta/vegetation" => {
                    let (_, rain_noise, _) = beta_seed::seed_beta_climate(self.world_seed);
                    return NoiseSampler::beta_simplex_2d(rain_noise, 0.05, 1.0 / 3.0, 16.0);
                }
                "mcrs:beta/climate_detail" => {
                    let (_, _, detail_noise) = beta_seed::seed_beta_climate(self.world_seed);
                    return NoiseSampler::beta_simplex_2d(detail_noise, 0.25, 1.0 / 1.7, 4.0);
                }
                _ => {}
            }
        }

        let mut random = self.random.clone().fork_hash(id.as_str());
        let noise_param = self.noises.get(id);
        if noise_param.is_none() {
            panic!("Noise not loaded: {}", id);
        }
        Self::from_noise_param(&mut random, noise_param.unwrap())
    }
}

#[cfg(all(test, feature = "serde"))]
mod arithmetic_node_tests {
    use super::{
        DensityFunctionComponent, DependentDensityFunction, FunctionStackBuilder,
    };
    use crate::density_function::Interval;
    use crate::density_function::node::Sampler;
    use crate::density_function::proto::{
        AXIS_X, AXIS_Z, DensityFunctionHolder, HashableF64, NoiseParam, Normalization,
        ProtoDensityFunction,
    };
    use crate::noise::normal_noise::NoiseSampler;
    use bevy_math::IVec3;
    use mcrs_minecraft_core::ResourceLocation;
    use mcrs_minecraft_random::legacy::LegacyRandom;
    use mcrs_minecraft_random::{Random, RandomSource};
    use std::collections::BTreeMap;

    fn build_at(
        json: &str,
        at: IVec3,
    ) -> (Vec<DensityFunctionComponent>, usize, f32, Interval) {
        let proto: ProtoDensityFunction =
            serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"));
        let functions = BTreeMap::new();
        let noises = BTreeMap::new();
        let mut builder =
            FunctionStackBuilder::new(RandomSource::new(0, false), 0, &functions, &noises);
        let index = builder.component(&DensityFunctionHolder::Owned(Box::new(proto)));
        let column_ready = super::compute_column_ready(&builder.stack);
        super::resolve_substituted_subgraphs(&mut builder.stack, &column_ready);
        let subgraph =
            crate::density_function::Subgraph::new((0..=index as u32).collect(), &column_ready);
        let scratch = crate::density_function::FillScratch::new();
        let mut value = [0.0f32];
        crate::density_function::node::Arena::new(&builder.stack, &scratch).fill_subgraph(
            &subgraph,
            &crate::density_function::Volume::point(at),
            &mut value,
        );
        let range = builder.stack[index].range;
        (builder.stack, index, value[0], range)
    }

    fn build(json: &str) -> (f32, f32, f32) {
        let (_, _, value, range) = build_at(json, IVec3::ZERO);
        (value, range.min(), range.max())
    }

    fn sample(json: &str) -> f32 {
        build(json).0
    }

    /// A gradient whose value is its own coordinate, so a pinned axis shows up
    /// in the result as the coordinate it was pinned to.
    fn coordinate_gradient(axis: &str) -> String {
        format!(
            r#"{{"type":"gradient","axis":"{axis}","from_coordinate":0,"to_coordinate":16,"from_value":0.0,"to_value":16.0}}"#
        )
    }

    fn plane() -> String {
        format!(
            r#"{{"type":"add","left":{},"right":{}}}"#,
            coordinate_gradient("x"),
            coordinate_gradient("z")
        )
    }

    fn sliced(axis: &str, coordinate: i32, input: &str) -> String {
        format!(r#"{{"type":"slice","axis":"{axis}","coordinate":{coordinate},"input":{input}}}"#)
    }

    /// A datapack may nest `slice` directly. The reference folds an X slice over
    /// a Z one into a sampler of its own; a mask folds a chain of any length.
    #[test]
    fn nested_slices_over_distinct_axes_fuse_into_one_node() {
        let json = sliced("x", 3, &sliced("z", 5, &plane()));
        let (stack, root, value, _) = build_at(&json, IVec3::new(11, 0, 13));

        let Sampler::Dependent(DependentDensityFunction::Slice(slice)) = &stack[root].sampler
        else {
            panic!("the outer slice compiled to {:?}", stack[root].sampler);
        };
        assert_eq!(slice.axes, AXIS_X | AXIS_Z);
        assert!(
            !matches!(
                stack[slice.input_index].sampler,
                Sampler::Dependent(DependentDensityFunction::Slice(_))
            ),
            "the fused slice still reads another slice"
        );
        assert_eq!(value, 8.0, "x pinned to 3 plus z pinned to 5");
    }

    /// Two slices naming the same axis: the inner substitution is the one
    /// applied last, so its coordinate is the one that survives.
    #[test]
    fn the_inner_coordinate_wins_when_two_slices_share_an_axis() {
        let json = sliced("x", 3, &sliced("x", 7, &plane()));
        let (_, _, value, _) = build_at(&json, IVec3::new(11, 0, 13));
        assert_eq!(value, 20.0, "x pinned to 7 plus z left at 13");
    }

    /// A y-gradient standing in for any input with a genuinely moving range;
    /// at y = 0 it evaluates to `from`.
    fn moving(from: f32, to: f32) -> String {
        format!(
            r#"{{"type":"gradient","axis":"y","from_coordinate":0,"to_coordinate":16,"from_value":{from},"to_value":{to}}}"#
        )
    }

    #[test]
    fn abs_and_square_cover_both_sides_of_a_zero_crossing_input() {
        let (_, min, max) = build(&format!(
            r#"{{"type":"abs","input":{}}}"#,
            moving(-5.0, 3.0)
        ));
        assert_eq!((min, max), (0.0, 5.0));
        let (_, min, max) = build(&format!(
            r#"{{"type":"square","input":{}}}"#,
            moving(-5.0, 3.0)
        ));
        assert_eq!((min, max), (0.0, 25.0));
        let (_, min, max) = build(&format!(
            r#"{{"type":"square","input":{}}}"#,
            moving(-4.0, 4.0)
        ));
        assert_eq!((min, max), (0.0, 16.0));
        let (_, min, max) = build(&format!(r#"{{"type":"abs","input":{}}}"#, moving(2.0, 6.0)));
        assert_eq!((min, max), (2.0, 6.0));
        let (_, min, max) = build(&format!(
            r#"{{"type":"square","input":{}}}"#,
            moving(-6.0, -2.0)
        ));
        assert_eq!((min, max), (4.0, 36.0));
    }

    #[test]
    fn clamp_range_is_the_image_of_its_input() {
        let (_, min, max) = build(&format!(
            r#"{{"type":"clamp","input":{},"min":-1.0,"max":1.0}}"#,
            moving(-0.5, 0.5)
        ));
        assert_eq!((min, max), (-0.5, 0.5));
        let (_, min, max) = build(&format!(
            r#"{{"type":"clamp","input":{},"min":-1.0,"max":1.0}}"#,
            moving(-8.0, 0.25)
        ));
        assert_eq!((min, max), (-1.0, 0.25));
    }

    #[test]
    fn pow_raises_base_to_exponent() {
        assert_eq!(
            sample(r#"{"type":"pow","base":2.0,"exponent":10.0}"#),
            1024.0
        );
        assert_eq!(
            sample(r#"{"type":"minecraft:pow","base":9.0,"exponent":0.5}"#),
            3.0
        );
        let (_, min, max) = build(r#"{"type":"pow","base":2.0,"exponent":10.0}"#);
        assert_eq!((min, max), (1024.0, 1024.0));
    }

    #[test]
    fn pow_range_covers_a_moving_base_and_exponent() {
        let (_, min, max) = build(&format!(
            r#"{{"type":"pow","base":{},"exponent":{}}}"#,
            moving(0.5, 2.0),
            moving(-2.0, 2.0)
        ));
        assert_eq!((min, max), (0.25, 4.0));
    }

    /// No shipped 26.3 asset uses `pow` or a rounding function, so nothing else
    /// proves these samplers are reachable rather than dead variants.
    #[test]
    fn pow_and_round_compile_their_constant_operand_samplers() {
        let kind = |json: &str| {
            let proto: ProtoDensityFunction =
                serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"));
            let functions = BTreeMap::new();
            let noises = BTreeMap::new();
            let mut builder =
                FunctionStackBuilder::new(RandomSource::new(0, false), 0, &functions, &noises);
            let index = builder.component(&DensityFunctionHolder::Owned(Box::new(proto)));
            let ready = super::compute_column_ready(&builder.stack);
        super::resolve_substituted_subgraphs(&mut builder.stack, &ready);
            crate::density_function::interval_prune::kind_name(&builder.stack[index])
        };
        let base = moving(0.5, 2.0);

        // The reference tests the base first, so a constant base wins outright.
        assert_eq!(
            kind(&format!(r#"{{"type":"pow","base":2.0,"exponent":{base}}}"#)),
            "ConstBasePow"
        );
        // An exponent the special cases do not cover keeps the transcendental,
        // but with the exponent baked in rather than read from a row.
        assert_eq!(
            kind(&format!(r#"{{"type":"pow","base":{base},"exponent":4.0}}"#)),
            "ConstExponentPow"
        );
        assert_eq!(
            kind(&format!(
                r#"{{"type":"pow","base":{base},"exponent":{base}}}"#
            )),
            "Pow"
        );
        assert_eq!(
            kind(&format!(
                r#"{{"type":"floor","input":{base},"multiple":4.0}}"#
            )),
            "IntegerMultipleRound"
        );

        assert_eq!(sample(r#"{"type":"pow","base":2.0,"exponent":3.0}"#), 8.0);
        assert_eq!(sample(r#"{"type":"pow","base":3.0,"exponent":4.0}"#), 81.0);
    }

    #[test]
    fn sqrt_log_and_sign() {
        assert_eq!(sample(r#"{"type":"sqrt","input":16.0}"#), 4.0);
        assert_eq!(sample(r#"{"type":"log","input":1.0}"#), 0.0);
        assert_eq!(sample(r#"{"type":"sign","input":-3.0}"#), -1.0);
        assert_eq!(sample(r#"{"type":"minecraft:sign","input":0.0}"#), 0.0);
        assert_eq!(
            build(&format!(
                r#"{{"type":"sign","input":{}}}"#,
                moving(-4.0, 4.0)
            ))
            .1,
            -1.0
        );
    }

    #[test]
    fn rounding_defaults_to_whole_numbers() {
        assert_eq!(sample(r#"{"type":"floor","input":2.7}"#), 2.0);
        assert_eq!(sample(r#"{"type":"ceil","input":2.1}"#), 3.0);
        assert_eq!(sample(r#"{"type":"truncate","input":-2.7}"#), -2.0);
        // Java's Math.round is half-up, so -2.5 rounds towards zero.
        assert_eq!(sample(r#"{"type":"round","input":-2.5}"#), -2.0);
        assert_eq!(sample(r#"{"type":"round","input":2.5}"#), 3.0);
    }

    #[test]
    fn rounding_snaps_to_a_multiple() {
        assert_eq!(
            sample(r#"{"type":"floor","input":7.0,"multiple":3.0}"#),
            6.0
        );
        let (value, min, max) = build(&format!(
            r#"{{"type":"ceil","input":{},"multiple":4.0}}"#,
            moving(0.0, 10.0)
        ));
        assert_eq!(value, 0.0);
        assert_eq!((min, max), (0.0, 12.0));
    }

    fn nether_climate_params() -> NoiseParam {
        NoiseParam {
            base_octave: -7,
            base_amplitude: HashableF64(0.9494731054427981),
            octave_count: 2,
            normalize: Normalization::Enabled,
            amplitude_modifiers: Vec::new(),
        }
    }

    #[test]
    fn the_nether_climate_noises_are_seeded_the_legacy_way() {
        const SEED: u64 = 845;
        let functions = BTreeMap::new();
        let mut noises = BTreeMap::new();
        for name in [
            "minecraft:nether/temperature",
            "minecraft:nether/vegetation",
        ] {
            noises.insert(
                ResourceLocation::parse(name).unwrap().into(),
                nether_climate_params(),
            );
        }
        let mut builder =
            FunctionStackBuilder::new(RandomSource::new(SEED, true), SEED, &functions, &noises);

        for (name, seed_offset) in [
            ("minecraft:nether/temperature", 0),
            ("minecraft:nether/vegetation", 1),
        ] {
            let id = ResourceLocation::parse(name).unwrap().into();
            let built = builder.create_noise(&id);
            assert_eq!(
                built,
                NoiseSampler::new(
                    &mut LegacyRandom::new(SEED + seed_offset),
                    -7,
                    vec![1.0, 1.0]
                ),
                "{name} must come from a raw legacy stream"
            );
            let param = nether_climate_params();
            assert_ne!(
                built,
                NoiseSampler::from_params(
                    &mut RandomSource::new(SEED, true).fork_hash(name),
                    param.base_octave,
                    param.octave_amplitudes(),
                    param.base_amplitude.0,
                    param.normalize,
                ),
                "{name} must not fall through to the hashed fork"
            );
        }
    }

    fn unary_node(json: &str) -> Option<&'static str> {
        let proto: ProtoDensityFunction = serde_json::from_str(json).unwrap();
        let functions = BTreeMap::new();
        let noises = BTreeMap::new();
        let mut builder =
            FunctionStackBuilder::new(RandomSource::new(0, false), 0, &functions, &noises);
        let index = builder.component(&DensityFunctionHolder::Owned(Box::new(proto)));
        match &builder.stack[index].sampler {
            Sampler::Dependent(f) => match f {
                DependentDensityFunction::Abs(_) => Some("abs"),
                DependentDensityFunction::Square(_) => Some("square"),
                DependentDensityFunction::Cube(_) => Some("cube"),
                DependentDensityFunction::Reciprocal(_) => Some("reciprocal"),
                DependentDensityFunction::Squeeze(_) => Some("squeeze"),
                DependentDensityFunction::Sqrt(_) => Some("sqrt"),
                DependentDensityFunction::Log(_) => Some("log"),
                DependentDensityFunction::Sign(_) => Some("sign"),
                _ => None,
            },
            _ => None,
        }
    }

    #[test]
    fn a_constant_exponent_avoids_the_transcendental() {
        let pow = |exponent: &str| {
            format!(
                r#"{{"type":"pow","base":{},"exponent":{exponent}}}"#,
                moving(0.5, 2.0)
            )
        };
        assert_eq!(unary_node(&pow("0.5")), Some("sqrt"));
        assert_eq!(unary_node(&pow("2.0")), Some("square"));
        assert_eq!(unary_node(&pow("3.0")), Some("cube"));
        assert_eq!(unary_node(&pow("-2.0")), Some("reciprocal"));
        assert_eq!(unary_node(&pow("4.0")), None);

        assert_eq!(sample(&pow("1.0")), sample(&moving(0.5, 2.0)));
        assert_eq!(
            sample(&pow("3.0")).to_bits(),
            sample(&format!(
                r#"{{"type":"cube","input":{}}}"#,
                moving(0.5, 2.0)
            ))
            .to_bits()
        );
    }

    #[test]
    fn lerp_follows_alpha_outside_the_unit_interval() {
        let lerp = |alpha: String| {
            format!(r#"{{"type":"lerp","alpha":{alpha},"first":0.0,"second":1.0}}"#)
        };
        let (_, min, max) = build(&lerp(moving(0.0, 1.0).to_string()));
        assert_eq!((min, max), (0.0, 1.0));
        let (_, min, max) = build(&lerp(moving(-2.0, 2.0).to_string()));
        assert_eq!((min, max), (-2.0, 2.0));
    }
}
